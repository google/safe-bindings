// Crubit does not support generic type parameters, so each concrete reader that
// a zip archive can be built on needs its own set of wrapper types. This module
// provides the reader that delegates to caller-supplied C callbacks, which is
// what lets C++ open an archive over storage this crate knows nothing about
// (for example a remote object read in ranges). `CallbackZipArchive` and
// `CallbackZipFile` are the archive and file types built on it.

use crate::read::CloneSlot;
use crate::ZipError;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::io::{Error, ErrorKind, Read, Result as IoResult, Seek, SeekFrom};
use std::sync::{Arc, OnceLock};

// NOTE(b/553659636): Evaluate replacing these callbacks, the `usize` context
// token and the thread-local staging buffer with a Crubit callable (`rs::Fn`),
// once that is confirmed to work with a `&mut [u8]` argument and with the OSS
// Corrosion build in `safe_bindings`.
pub(crate) type ReadAtCallback = extern "C" fn(usize, u64, usize) -> i64;
pub(crate) type SizeCallback = extern "C" fn(usize) -> i64;
pub(crate) type ReleaseCallback = extern "C" fn(usize);

/// The most bytes a single `read_at` call is asked for.
const MAX_READ_CHUNK_SIZE: usize = 1 << 20; // 1 MiB

/// The most staging capacity a thread keeps between reads.
///
/// Deliberately much smaller than `MAX_READ_CHUNK_SIZE`. The staging buffer is
/// thread-local, so whatever it keeps stays allocated for as long as the thread
/// lives, on every thread that ever read through a callback, even once every
/// reader is gone. This is enough for small reads to reuse the buffer; a larger
/// read costs one allocation, which is negligible next to calling back for
/// that many bytes.
const MAX_RETAINED_STAGED_CAPACITY: usize = 64 << 10; // 64 KiB

thread_local! {
    static STAGED_READ_BYTES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn clear_staged_vec(staged: &mut Vec<u8>) {
    staged.clear();
    if staged.capacity() > MAX_RETAINED_STAGED_CAPACITY {
        staged.shrink_to(MAX_RETAINED_STAGED_CAPACITY);
    }
}

fn clear_staged_read_bytes() {
    STAGED_READ_BYTES.with(|cell| {
        clear_staged_vec(&mut cell.borrow_mut());
    });
}

/// Stages the bytes read by the active `read_at` callback on the current thread
/// so `CallbackReader::read` can copy them into its destination slice without
/// passing raw pointers across FFI.
pub fn stage_callback_chunk(data: &[u8]) {
    STAGED_READ_BYTES.with(|cell| {
        let mut staged = cell.borrow_mut();
        staged.clear();
        staged.extend_from_slice(data);
    });
}

/// What can go wrong with one `read_at` call.
#[derive(Clone, Copy, Debug)]
enum ReadFault {
    /// `read_at` reported a failure.
    Failed,
    /// `read_at` returned no bytes. It is only asked for bytes that `size` said
    /// are there, so this is not the end of the input. Passing it on as the
    /// end would make the input look shorter than it is, which not everything
    /// that reads through the reader would notice: an entry read raw has no
    /// checksum that could fail.
    NoBytes,
    /// `read_at` returned more bytes than it was asked for. Trusting that
    /// would mean reporting bytes it never wrote.
    TooManyBytes,
    /// `read_at` staged a number of bytes other than the one it returned.
    StagedCountMismatch,
}

/// The text of the error that a `read_at` call which returned no bytes is
/// reported with.
const NO_BYTES_MESSAGE: &str =
    "The zip archive read callback returned no bytes before the end of the input";

impl From<ReadFault> for Error {
    fn from(fault: ReadFault) -> Self {
        match fault {
            ReadFault::Failed => Error::other(CallbackFailed),
            ReadFault::NoBytes => Error::new(ErrorKind::UnexpectedEof, NO_BYTES_MESSAGE),
            ReadFault::TooManyBytes => Error::new(
                ErrorKind::InvalidData,
                "The zip archive read callback reported reading more bytes than requested",
            ),
            ReadFault::StagedCountMismatch => Error::new(
                ErrorKind::InvalidData,
                "The zip archive read callback staged a byte count that does not match its return value",
            ),
        }
    }
}

/// Keeps the opening of an archive from reading on after a read has gone
/// wrong.
///
/// The `zip` crate does not propagate every error of a read: while it locates
/// the central directory it treats some of them like data that is not what it
/// expected and carries on, reading more. The open therefore watches the
/// reader that it opens the archive through (`CallbackReader::watch`). From
/// the first read of that reader that goes wrong (see `ReadFault`), every read
/// of it that would call `read_at` fails the same way instead, so `zip` gives
/// up at no further cost and that first fault stays the last thing the
/// callbacks did. The watch then tells the open what went wrong, if anything
/// did; see `CallbackZipArchive::new_from_callbacks`.
///
/// A read that goes wrong on a reader that is not watched has no effect on
/// later ones. That is every reader but the one an archive is being opened
/// through: a clone is never watched, and the archive that the open returns
/// reads through a clone. A read of an open archive can therefore be retried.
///
/// The fault is written once and never cleared. It sits behind an `Arc`
/// because the open still asks for it after `zip` has dropped the reader of an
/// open that failed.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenWatch(Arc<OnceLock<ReadFault>>);

impl OpenWatch {
    /// If a read of the watched reader has gone wrong, returns the error that
    /// the open fails with.
    pub(crate) fn check(&self) -> Result<(), ZipError> {
        match self.fault() {
            None => Ok(()),
            // Input that ends before its size says it does is not a valid
            // archive, which is also what the open reports where `zip` itself
            // finds the archive cut short.
            Some(ReadFault::NoBytes) => Err(ZipError::invalid_argument(NO_BYTES_MESSAGE)),
            Some(fault) => Err(ZipError::from_io_error(Error::from(fault))),
        }
    }

    /// Records that a read went wrong. Does nothing once the watch holds an
    /// earlier fault.
    fn record(&self, fault: ReadFault) {
        // Setting fails exactly when an earlier fault is held, which stays.
        let _ = self.0.set(fault);
    }

    /// Returns what went wrong first, if anything has.
    #[must_use]
    fn fault(&self) -> Option<ReadFault> {
        self.0.get().copied()
    }
}

/// The callbacks and the opaque caller token they operate on.
///
/// Held behind an `Arc` so that an archive and all of its clones share one
/// context and release it exactly once, when the last of them is dropped.
struct CallbackContext {
    read_at: ReadAtCallback,
    release: ReleaseCallback,
    context: usize,
}

impl Debug for CallbackContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackContext").finish_non_exhaustive()
    }
}

impl Drop for CallbackContext {
    fn drop(&mut self) {
        (self.release)(self.context);
    }
}

/// The payload of the `io::Error` that `CallbackReader::read` returns when
/// `read_at` reports a failure, so that `ZipError::from_io_error` can tell a
/// failed callback apart from any other I/O error the `zip` crate reports.
///
/// The distinction matters to the C++ side: a failed callback has left the
/// reader's own status behind for the caller to pick up, whereas an error the
/// archive raised on its own must be reported as is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CallbackFailed;

/// The `Display` text of `CallbackFailed`. `ZipError::from_io_error` also
/// recognises it by this text, because some code between `CallbackReader` and
/// `from_io_error` rebuilds `io::Error`s from their `Display` output and so
/// drops the `CallbackFailed` payload: `lzma_rust2`'s `StickyError` (every
/// LZMA entry read past the decoder's first buffer fill) and
/// `zip::result::ZipError::clone()`.
pub(crate) const CALLBACK_FAILED_MESSAGE: &str = "The zip archive read callback failed";

impl Display for CallbackFailed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(CALLBACK_FAILED_MESSAGE)
    }
}

impl std::error::Error for CallbackFailed {}

/// A seekable reader whose bytes come from caller-supplied callbacks.
///
/// Cloning is cheap and performs no I/O: clones share one context and one
/// cached length, and only the read position is per-clone. That is what lets
/// `CallbackZipArchive::try_clone` be free, which in turn lets a caller hold
/// one archive handle per concurrently open entry. A clone adopts whatever
/// `CallbackReader` is staged in its `CloneSlot` instead, which is how
/// `CallbackZipArchive::try_clone_with_callbacks` moves the parsed central
/// directory onto a different set of callbacks.
///
/// `pub(crate)` rather than `pub`: Crubit cannot bind the `Read`/`Seek` traits,
/// so C++ only ever sees the archive wrappers built on top of this reader.
#[derive(Debug)]
pub(crate) struct CallbackReader {
    context: Arc<CallbackContext>,
    clone_slot: Arc<CloneSlot<CallbackReader>>,
    /// Total number of bytes available.
    ///
    /// Queried once at construction rather than per seek. The input is
    /// read-only for the lifetime of the reader, so the length cannot change,
    /// and `zip` seeks relative to the end repeatedly while hunting for the
    /// end-of-central-directory record; re-asking the caller each time would
    /// turn that search into a burst of round trips against remote storage.
    len: u64,
    /// Offset the next `read` starts at. May exceed `len`, in which case reads
    /// report end of input.
    pos: u64,
    /// The watch that the opening of an archive keeps over this reader, if it
    /// is the reader of an open in progress. See `OpenWatch`.
    open_watch: Option<OpenWatch>,
}

impl Clone for CallbackReader {
    fn clone(&self) -> Self {
        // A clone is never watched, whichever reader it comes from. Only the
        // open of an archive wants its reader watched, and once it is over it
        // keeps a clone of what it opened; that is how the reads of an open
        // archive come to be free of the watch. See `OpenWatch`.
        if let Some(replacement) = self.clone_slot.take_staged() {
            return Self { open_watch: None, ..replacement };
        }
        Self {
            context: Arc::clone(&self.context),
            clone_slot: Arc::clone(&self.clone_slot),
            len: self.len,
            pos: self.pos,
            open_watch: None,
        }
    }
}

impl CallbackReader {
    /// Creates a reader that obtains its bytes by invoking `read_at` on
    /// `context`.
    ///
    /// `read_at` must read up to `len` bytes from `context` starting at
    /// `offset`, stage exactly the bytes it read with a single call to
    /// [`stage_callback_chunk`] (which replaces anything staged before), and
    /// return their count, or a negative value if the read failed. It may
    /// return fewer bytes than requested, but not 0: it is only asked for
    /// bytes that `size` said are there, so a count of 0 is reported as
    /// truncated input. `size` must return the total number of bytes
    /// available, or a negative value on failure. `release` is called with
    /// `context` exactly once: after the last clone of the reader is dropped,
    /// or before this function returns an error.
    ///
    /// Because clones of this reader share `context` and may be used from
    /// different threads at once (one archive handle per concurrently open
    /// entry), `read_at` must be safe to call concurrently with itself from any
    /// thread. `size` is called once, from this function, before any clone
    /// exists, and `release` runs after the last clone is dropped, so neither
    /// overlaps another callback. None of the callbacks may unwind across FFI.
    pub(crate) fn try_new(
        read_at: ReadAtCallback,
        size: SizeCallback,
        release: ReleaseCallback,
        context: usize,
    ) -> Result<Self, ZipError> {
        // Take ownership before anything else can fail, so that `release` runs
        // even if determining the length does not succeed.
        let context = Arc::new(CallbackContext { read_at, release, context });
        // `size` is only ever called here, so it is not kept.
        let len = size(context.context);
        if len < 0 {
            return Err(ZipError::internal("Failed to determine the size of the zip archive")
                .caused_by_reader_callback());
        }
        Ok(Self {
            context,
            clone_slot: Arc::new(CloneSlot::default()),
            len: len as u64,
            pos: 0,
            open_watch: None,
        })
    }

    /// The slot a replacement reader is staged in for `clone` to adopt.
    #[must_use]
    pub(crate) fn clone_slot(&self) -> Arc<CloneSlot<CallbackReader>> {
        Arc::clone(&self.clone_slot)
    }

    /// Returns the total number of bytes available, as reported by `size` when
    /// the reader was created.
    #[must_use]
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// Puts this reader, which an archive is about to be opened through, under
    /// a watch and returns that watch. See `OpenWatch`.
    #[must_use]
    pub(crate) fn watch(&mut self) -> OpenWatch {
        let open_watch = OpenWatch::default();
        self.open_watch = Some(open_watch.clone());
        open_watch
    }

    /// Calls `read_at` for the bytes at the current position, which must be
    /// before the end, and copies what it staged into `buf`, which must not be
    /// empty. Returns how many bytes that is. Does not move the position.
    fn call_read_at(&self, buf: &mut [u8]) -> Result<usize, ReadFault> {
        // Never ask for more than what remains. The caller's `read_at` is also
        // expected to stop at the end, but clamping here means a caller that
        // clamps differently still cannot make this reader advance past `len`.
        //
        // Also never ask for more than `MAX_READ_CHUNK_SIZE` at once.
        // The callback typically allocates a temporary buffer of the requested
        // length on the C++ side and the staged copy on this side is another
        // one, so this bounds both however large `buf` is. `Read` permits
        // short reads, and so does this reader's contract, so a caller wanting
        // more simply calls again.
        let remaining = self.len - self.pos;
        let len = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len())
            .min(MAX_READ_CHUNK_SIZE);
        clear_staged_read_bytes();
        let read = (self.context.read_at)(self.context.context, self.pos, len);
        STAGED_READ_BYTES.with(|cell| {
            let mut staged = cell.borrow_mut();
            let copied = copy_staged_bytes(read, &staged, &mut buf[..len]);
            // Whatever became of the read, its bytes are not kept around.
            clear_staged_vec(&mut staged);
            copied
        })
    }
}

/// Copies the bytes that a `read_at` call staged (`staged`) into `buf`, whose
/// length is what the call was asked for, and returns how many bytes that is,
/// which is never 0.
///
/// Returns the `ReadFault` instead, and copies nothing, if what the call
/// returned (`read`) is negative, exceeds what it was asked for, differs from
/// the number of bytes it staged, or is 0.
fn copy_staged_bytes(read: i64, staged: &[u8], buf: &mut [u8]) -> Result<usize, ReadFault> {
    if read < 0 {
        return Err(ReadFault::Failed);
    }
    let read = read as u64;
    if read > buf.len() as u64 {
        return Err(ReadFault::TooManyBytes);
    }
    let read = read as usize;
    // A count of 0 is checked like any other: nothing may have been staged for
    // it either.
    if staged.len() != read {
        return Err(ReadFault::StagedCountMismatch);
    }
    if read == 0 {
        return Err(ReadFault::NoBytes);
    }
    buf[..read].copy_from_slice(staged);
    Ok(read)
}

impl Read for CallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        if let Some(fault) = self.open_watch.as_ref().and_then(OpenWatch::fault) {
            // A read has gone wrong during the open in progress, which must
            // not read on; see `OpenWatch`.
            return Err(fault.into());
        }
        match self.call_read_at(buf) {
            Ok(read) => {
                self.pos += read as u64;
                Ok(read)
            }
            Err(fault) => {
                if let Some(open_watch) = &self.open_watch {
                    open_watch.record(fault);
                }
                Err(fault.into())
            }
        }
    }
}

impl Seek for CallbackReader {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let target = match pos {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(offset) => self.len.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.pos.checked_add_signed(offset),
        };
        let Some(target) = target else {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Zip archive seek target is negative or overflows u64",
            ));
        };
        // Seeking beyond the end is allowed and only reports end of input on the
        // next read, matching `File`. `zip` relies on that while searching
        // backwards for the end-of-central-directory record.
        self.pos = target;
        Ok(target)
    }

    fn stream_position(&mut self) -> IoResult<u64> {
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
        use super::*;
    use crate::ZipErrorCode;
    use googletest::prelude::*;
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    /// Test context backed by an in-memory buffer, which records the length of
    /// every `read_at` request and whether it was released.
    ///
    /// Shared between concurrently used clones of the reader, so all mutable
    /// state is behind a `Mutex`.
    struct TestContext {
        data: Vec<u8>,
        /// The `len` argument of every `read_at` call, in order.
        requested_lens: Mutex<Vec<usize>>,
        released: Arc<Mutex<usize>>,
        /// What `size` reports. Lets tests under-report the available bytes.
        reported_size: i64,
        /// Largest number of bytes any single `read_at` will serve, so tests can
        /// exercise the short-read path.
        max_read: usize,
        /// When true, `read_at` fails instead of serving bytes.
        fail_reads: bool,
        /// How many more `read_at` calls fail before bytes are served again, so
        /// tests can let a read fail and a later one succeed.
        failing_reads_left: Mutex<usize>,
        /// When true, `read_at` claims to have read more than it was asked for.
        overreport: bool,
        /// When true, `read_at` returns a positive count without staging bytes.
        skip_staging: bool,
        /// When true, `read_at` stages bytes but reports having read none.
        stage_but_report_zero: bool,
        /// When true, `read_at` first stages far more bytes than any read asks
        /// for, and then reports a count that chunk does not match unless
        /// `fail_reads` or `overreport` say otherwise.
        stage_oversized: bool,
    }

    /// The live test contexts by token, and the token that the next one gets.
    struct TestContexts {
        next_token: usize,
        contexts: HashMap<usize, Arc<TestContext>>,
    }

    static TEST_CONTEXTS: LazyLock<Mutex<TestContexts>> =
        LazyLock::new(|| Mutex::new(TestContexts { next_token: 1, contexts: HashMap::new() }));

    fn register_context(context: TestContext) -> (usize, Arc<TestContext>) {
        let context = Arc::new(context);
        let mut test_contexts = TEST_CONTEXTS.lock().unwrap();
        let token = test_contexts.next_token;
        test_contexts.next_token += 1;
        test_contexts.contexts.insert(token, Arc::clone(&context));
        (token, context)
    }

    fn lookup_context(token: usize) -> Arc<TestContext> {
        TEST_CONTEXTS.lock().unwrap().contexts.get(&token).cloned().expect("live test context")
    }

    extern "C" fn test_read_at(token: usize, offset: u64, len: usize) -> i64 {
        let context = lookup_context(token);
        context.requested_lens.lock().unwrap().push(len);
        if context.stage_oversized {
            stage_callback_chunk(&vec![0u8; 3 * MAX_READ_CHUNK_SIZE]);
        }
        if context.fail_reads {
            return -1;
        }
        {
            let mut failing_reads_left = context.failing_reads_left.lock().unwrap();
            if *failing_reads_left > 0 {
                *failing_reads_left -= 1;
                return -1;
            }
        }
        if context.overreport {
            return len as i64 + 1;
        }
        if context.stage_oversized {
            return len as i64;
        }
        if context.stage_but_report_zero {
            stage_callback_chunk(b"0123");
            return 0;
        }
        let offset = offset as usize;
        if offset >= context.data.len() {
            return 0;
        }
        let available = context.data.len() - offset;
        let len = len.min(available).min(context.max_read);
        if len > 0 && !context.skip_staging {
            stage_callback_chunk(&context.data[offset..offset + len]);
        }
        len as i64
    }

    extern "C" fn test_size(token: usize) -> i64 {
        lookup_context(token).reported_size
    }

    extern "C" fn test_size_fails(_token: usize) -> i64 {
        -1
    }

    extern "C" fn test_release(token: usize) {
        let context =
            TEST_CONTEXTS.lock().unwrap().contexts.remove(&token).expect("context released once");
        *context.released.lock().unwrap() += 1;
    }

    /// Builds a reader over `context`. Returns the reader and the shared test
    /// context so tests can inspect `released` and `requested_lens`.
    fn make_reader(context: TestContext) -> (CallbackReader, Arc<TestContext>) {
        let (token, shared_context) = register_context(context);
        let reader = CallbackReader::try_new(test_read_at, test_size, test_release, token).unwrap();
        (reader, shared_context)
    }

    fn context_for(data: Vec<u8>) -> TestContext {
        TestContext {
            reported_size: data.len() as i64,
            data,
            requested_lens: Mutex::new(Vec::new()),
            released: Arc::new(Mutex::new(0)),
            max_read: usize::MAX,
            fail_reads: false,
            failing_reads_left: Mutex::new(0),
            overreport: false,
            skip_staging: false,
            stage_but_report_zero: false,
            stage_oversized: false,
        }
    }

    fn requested_lens(context: &TestContext) -> Vec<usize> {
        context.requested_lens.lock().unwrap().clone()
    }

    /// Returns the length and the capacity of this thread's staging buffer.
    fn staged_len_and_capacity() -> (usize, usize) {
        STAGED_READ_BYTES.with(|cell| {
            let staged = cell.borrow();
            (staged.len(), staged.capacity())
        })
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[gtest]
    fn test_reader_is_send_and_sync() {
        // Compile-time check: the reader must be usable from any thread so that
        // the archive built on it can be moved to and cloned across threads.
        assert_send_sync::<CallbackReader>();
    }

    #[gtest]
    fn test_reads_the_whole_input() {
        let (mut reader, _) = make_reader(context_for(b"Hello, world!".to_vec()));
        expect_eq!(reader.len(), 13);
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got.as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_empty_reader_reports_empty_without_calling_back() {
        let (mut reader, context) = make_reader(context_for(Vec::new()));
        expect_eq!(reader.len(), 0);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_that!(requested_lens(&context), is_empty());
    }

    #[gtest]
    fn test_empty_buffer_mid_input_reads_nothing_without_calling_back() {
        let (mut reader, context) = make_reader(context_for(b"0123456789".to_vec()));
        reader.seek(SeekFrom::Start(3)).unwrap();
        // Bytes remain, but there is nowhere to put them: `Read` says this
        // returns 0, and a round trip to the caller for nothing is pointless.
        expect_eq!(reader.read(&mut []).unwrap(), 0);
        expect_that!(requested_lens(&context), is_empty());
        expect_eq!(reader.stream_position().unwrap(), 3);
    }

    #[gtest]
    fn test_premature_end_of_input_is_reported_as_truncation() {
        let mut context = context_for(b"01234".to_vec());
        // `size` promised more than `read_at` can serve, so `read_at` returns
        // 0 while the reader still expects bytes. Taken for the end of the
        // input, that would silently shorten whatever is being read, so the
        // reader reports the input as truncated instead, without advancing.
        context.reported_size = 10;
        let (mut reader, context) = make_reader(context);
        expect_eq!(reader.len(), 10);

        let mut buf = [0u8; 10];
        expect_eq!(reader.read(&mut buf).unwrap(), 5);
        expect_eq!(&buf[..5], b"01234");
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::UnexpectedEof);
        expect_eq!(reader.stream_position().unwrap(), 5, "a 0-byte read must not advance");
        expect_eq!(reader.read_exact(&mut buf[..1]).unwrap_err().kind(), ErrorKind::UnexpectedEof);
        // Each of the three reads consulted the callback: nothing is cached.
        expect_eq!(requested_lens(&context), vec![10, 5, 1]);

        // This is the archive's own error, not a failure of the callback.
        let zip_error = ZipError::from_io_error(reader.read(&mut buf).unwrap_err());
        expect_eq!(zip_error.code, ZipErrorCode::Internal);
        expect_false!(zip_error.is_reader_callback_failure());
    }

    #[gtest]
    fn test_staged_buffer_capacity_is_given_back_after_a_large_read() {
        // A full-size read stages `MAX_READ_CHUNK_SIZE` bytes. Once they have
        // been copied out, the thread-local must not keep that much memory
        // around: it would stay allocated for the rest of the thread's life.
        let data: Vec<u8> = (0..=255u8).cycle().take(MAX_READ_CHUNK_SIZE).collect();
        let (mut reader, _) = make_reader(context_for(data.clone()));
        let mut buf = vec![0u8; MAX_READ_CHUNK_SIZE];
        expect_eq!(reader.read(&mut buf).unwrap(), MAX_READ_CHUNK_SIZE);
        expect_eq!(buf, data);

        let (len_after, capacity_after) = staged_len_and_capacity();
        expect_eq!(len_after, 0);
        expect_le!(capacity_after, MAX_RETAINED_STAGED_CAPACITY);
    }

    #[gtest]
    fn test_staged_buffer_capacity_is_given_back_after_a_rejected_read() {
        // A callback may stage far more than it was asked for; the bytes have
        // been copied by the time the reader can object. Whichever way the
        // reader then rejects the read, it must give that memory back.
        let cases = [
            ("callback failed", true, false, ErrorKind::Other),
            ("callback over-reported", false, true, ErrorKind::InvalidData),
            ("staged length mismatched", false, false, ErrorKind::InvalidData),
        ];
        for (name, fail_reads, overreport, expected_kind) in cases {
            let mut context = context_for(b"0123456789".to_vec());
            context.stage_oversized = true;
            context.fail_reads = fail_reads;
            context.overreport = overreport;
            let (mut reader, _) = make_reader(context);
            let mut buf = [0u8; 4];
            expect_eq!(reader.read(&mut buf).unwrap_err().kind(), expected_kind, "{}", name);

            let (len_after, capacity_after) = staged_len_and_capacity();
            expect_eq!(len_after, 0, "{}", name);
            expect_le!(capacity_after, MAX_RETAINED_STAGED_CAPACITY, "{}", name);
        }
    }

    #[gtest]
    fn test_large_reads_are_requested_in_bounded_chunks() {
        // 4 MiB of input, read with a 4 MiB buffer: no single `read_at` may be
        // asked for more than `MAX_READ_CHUNK_SIZE`, since the request bounds
        // both the callback's temporary buffer and the staged copy, yet looping
        // over the short reads must still yield everything.
        let data: Vec<u8> = (0..=255u8).cycle().take(4 << 20).collect();
        let (mut reader, context) = make_reader(context_for(data.clone()));
        let mut buf = vec![0u8; 4 << 20];

        let first = reader.read(&mut buf).unwrap();
        expect_eq!(first, MAX_READ_CHUNK_SIZE);
        // `read_exact` keeps calling `read` until the buffer is full, and
        // fails instead of spinning if a read comes back empty before then.
        reader.read_exact(&mut buf[first..]).unwrap();
        expect_eq!(buf, data);
        expect_eq!(reader.read(&mut buf).unwrap(), 0);

        let lens = requested_lens(&context);
        expect_eq!(lens.len(), (4 << 20) / MAX_READ_CHUNK_SIZE);
        for len in lens {
            expect_le!(len, MAX_READ_CHUNK_SIZE);
        }

        // `read_to_end` goes through the same path.
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got, data);
        for len in requested_lens(&context) {
            expect_le!(len, MAX_READ_CHUNK_SIZE);
        }
    }

    #[gtest]
    fn test_short_reads_are_resumed() {
        let mut context = context_for(b"Hello, world!".to_vec());
        // Force the callback to serve at most 3 bytes per call, which a correct
        // `Read` implementation must handle by being called again.
        context.max_read = 3;
        let (mut reader, _) = make_reader(context);
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got.as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_seek_from_start_current_and_end() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        let mut byte = [0u8; 1];

        expect_eq!(reader.seek(SeekFrom::Start(4)).unwrap(), 4);
        reader.read_exact(&mut byte).unwrap();
        expect_eq!(byte[0], b'4');

        // The read above advanced the position to 5.
        expect_eq!(reader.seek(SeekFrom::Current(2)).unwrap(), 7);
        reader.read_exact(&mut byte).unwrap();
        expect_eq!(byte[0], b'7');

        expect_eq!(reader.seek(SeekFrom::End(-1)).unwrap(), 9);
        reader.read_exact(&mut byte).unwrap();
        expect_eq!(byte[0], b'9');
    }

    #[gtest]
    fn test_seek_before_the_start_fails() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        expect_eq!(reader.seek(SeekFrom::End(-11)).unwrap_err().kind(), ErrorKind::InvalidInput);
        expect_eq!(reader.seek(SeekFrom::Current(-1)).unwrap_err().kind(), ErrorKind::InvalidInput);
        // The most negative offsets, which cannot be negated.
        expect_eq!(
            reader.seek(SeekFrom::End(i64::MIN)).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        expect_eq!(
            reader.seek(SeekFrom::Current(i64::MIN)).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        expect_eq!(
            reader.stream_position().unwrap(),
            0,
            "a failed seek must not move the position"
        );
    }

    #[gtest]
    fn test_seek_overflowing_u64_fails() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        expect_eq!(reader.seek(SeekFrom::Start(u64::MAX)).unwrap(), u64::MAX);
        expect_eq!(reader.seek(SeekFrom::Current(1)).unwrap_err().kind(), ErrorKind::InvalidInput);
        expect_eq!(reader.stream_position().unwrap(), u64::MAX);
    }

    #[gtest]
    fn test_seek_past_the_end_reports_end_of_input() {
        let (mut reader, context) = make_reader(context_for(b"0123456789".to_vec()));
        // `zip` seeks around while looking for the end-of-central-directory
        // record, so this must behave like `File` and not fail until a read.
        expect_eq!(reader.seek(SeekFrom::Start(1000)).unwrap(), 1000);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_that!(requested_lens(&context), is_empty(), "read past the end must not call back");
    }

    #[gtest]
    fn test_reads_never_run_past_the_end() {
        let (mut reader, context) = make_reader(context_for(b"0123456789".to_vec()));
        reader.seek(SeekFrom::Start(2)).unwrap();
        let mut buf = [0u8; 100];
        expect_eq!(reader.read(&mut buf).unwrap(), 8);
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        // The request is clamped to the 8 bytes that remain, and once the
        // position reaches the end the callback is not consulted at all.
        expect_eq!(requested_lens(&context), vec![8]);
    }

    #[gtest]
    fn test_under_reported_size_bounds_the_reader() {
        let mut context = context_for(b"0123456789".to_vec());
        // `size` is the authority on the length: if it reports fewer bytes
        // than `read_at` could serve, the reader never asks for the rest.
        context.reported_size = 5;
        let (mut reader, context) = make_reader(context);
        expect_eq!(reader.len(), 5);

        let mut buf = [0u8; 100];
        expect_eq!(reader.read(&mut buf).unwrap(), 5);
        expect_eq!(&buf[..5], b"01234");
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_eq!(reader.stream_position().unwrap(), 5);
        expect_eq!(requested_lens(&context), vec![5], "only one clamped request is expected");
    }

    #[gtest]
    fn test_failing_read_is_reported() {
        let mut context = context_for(b"0123456789".to_vec());
        context.fail_reads = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        let err = reader.read(&mut buf).unwrap_err();
        expect_eq!(err.kind(), ErrorKind::Other);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");

        // The error identifies itself as a failed callback all the way into a
        // `ZipError`, which is what lets the C++ side substitute the reader's
        // own status for it and only for it.
        expect_true!(err.get_ref().is_some_and(|inner| inner.is::<CallbackFailed>()));
        let zip_error = ZipError::from_io_error(err);
        expect_eq!(zip_error.code, ZipErrorCode::Internal);
        expect_true!(zip_error.is_reader_callback_failure());
        expect_true!(zip_error.with_context("context").is_reader_callback_failure());
    }

    #[gtest]
    fn test_watched_reader_is_not_called_again_after_a_failure() {
        let mut context = context_for(b"0123456789".to_vec());
        context.failing_reads_left = Mutex::new(1);
        let (mut reader, context) = make_reader(context);
        let watch = reader.watch();

        // Only the first `read_at` fails, but while the reader is watched the
        // next read fails the same way, and without calling back.
        let mut buf = [0u8; 4];
        let failed = reader.read(&mut buf).unwrap_err();
        expect_true!(failed.get_ref().is_some_and(|inner| inner.is::<CallbackFailed>()));
        let refused = reader.read(&mut buf).unwrap_err();
        expect_true!(refused.get_ref().is_some_and(|inner| inner.is::<CallbackFailed>()));
        expect_eq!(requested_lens(&context), vec![4], "the second read must not call back");
        // Where no callback would be made anyway, nothing fails.
        expect_eq!(reader.read(&mut []).unwrap(), 0);

        // The watch reports the failure. A clone of the reader is not watched,
        // so its reads go through again.
        let open_error = watch.check().unwrap_err();
        expect_eq!(open_error.code, ZipErrorCode::Internal);
        expect_true!(open_error.is_reader_callback_failure());
        let mut clone = reader.clone();
        expect_eq!(clone.read(&mut buf).unwrap(), 4);
        expect_eq!(&buf, b"0123");
        expect_eq!(requested_lens(&context), vec![4, 4]);
    }

    #[gtest]
    fn test_watched_reader_is_not_called_again_after_a_breach_of_the_read_at_contract() {
        // `size` promises bytes that `read_at` then does not have.
        let mut no_bytes = context_for(Vec::new());
        no_bytes.reported_size = 10;
        let mut too_many_bytes = context_for(b"0123456789".to_vec());
        too_many_bytes.overreport = true;
        let mut nothing_staged = context_for(b"0123456789".to_vec());
        nothing_staged.skip_staging = true;
        let cases = [
            (
                "returned no bytes",
                no_bytes,
                ErrorKind::UnexpectedEof,
                ZipErrorCode::InvalidArgument,
            ),
            ("over-reported", too_many_bytes, ErrorKind::InvalidData, ZipErrorCode::Internal),
            ("staged nothing", nothing_staged, ErrorKind::InvalidData, ZipErrorCode::Internal),
        ];
        for (name, context, expected_kind, expected_code) in cases {
            let (mut reader, context) = make_reader(context);
            let watch = reader.watch();

            // The second read fails like the first, and without calling back.
            let mut buf = [0u8; 4];
            expect_eq!(reader.read(&mut buf).unwrap_err().kind(), expected_kind, "{}", name);
            expect_eq!(reader.read(&mut buf).unwrap_err().kind(), expected_kind, "{}", name);
            expect_eq!(requested_lens(&context), vec![4], "{}", name);

            // The open fails with the archive's own error: no callback
            // reported a failure.
            let open_error = watch.check().unwrap_err();
            expect_eq!(open_error.code, expected_code, "{}", name);
            expect_eq!(open_error.is_reader_callback_failure(), false, "{}", name);
        }
    }

    #[gtest]
    fn test_unwatched_reader_is_called_again_after_a_failure() {
        let mut context = context_for(b"0123456789".to_vec());
        context.failing_reads_left = Mutex::new(1);
        let (mut reader, context) = make_reader(context);

        // On a reader that is not watched a failed read is over once it has
        // been reported, so that a caller may retry it.
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::Other);
        expect_eq!(reader.read(&mut buf).unwrap(), 4);
        expect_eq!(&buf, b"0123");
        expect_eq!(requested_lens(&context), vec![4, 4]);

        // Nor is it held against a watch that begins later.
        let watch = reader.watch();
        expect_eq!(reader.read(&mut buf).unwrap(), 4);
        expect_eq!(watch.check(), Ok(()));
    }

    #[gtest]
    fn test_other_io_errors_are_not_reader_callback_failures() {
        let eof = ZipError::from_io_error(Error::new(ErrorKind::UnexpectedEof, "truncated"));
        expect_eq!(eof.code, ZipErrorCode::Internal);
        expect_false!(eof.is_reader_callback_failure());
        let other = ZipError::from_io_error(Error::other("some other failure"));
        expect_eq!(other.code, ZipErrorCode::Internal);
        expect_false!(other.is_reader_callback_failure());
        expect_false!(ZipError::internal("archive error").is_reader_callback_failure());
    }

    #[gtest]
    fn test_failing_size_is_a_reader_callback_failure() {
        let context = context_for(b"0123456789".to_vec());
        let (token, _) = register_context(context);
        let err = CallbackReader::try_new(test_read_at, test_size_fails, test_release, token)
            .unwrap_err();
        expect_eq!(err.code, ZipErrorCode::Internal);
        expect_true!(err.is_reader_callback_failure());
    }

    #[gtest]
    fn test_callback_claiming_too_many_bytes_is_rejected() {
        let mut context = context_for(b"0123456789".to_vec());
        context.overreport = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        // Believing the callback here would report bytes the callback never
        // staged, so it must be an error rather than a silent truncation.
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::InvalidData);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_callback_staging_mismatched_length_is_rejected() {
        let mut context = context_for(b"0123456789".to_vec());
        context.skip_staging = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::InvalidData);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_callback_staging_bytes_but_reporting_none_is_rejected() {
        let mut context = context_for(b"0123456789".to_vec());
        context.stage_but_report_zero = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        // Staged bytes contradict a count of 0, so this must not pass for a
        // clean end of input.
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::InvalidData);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_bytes_staged_before_a_read_are_not_served() {
        let mut context = context_for(b"0123456789".to_vec());
        context.skip_staging = true;
        let (mut reader, _) = make_reader(context);
        // Bytes left staged on this thread by anything other than the read's
        // own callback must not be handed out as that read's data.
        stage_callback_chunk(b"STAL");
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::InvalidData);
        expect_eq!(buf, [0u8; 4], "stale staged bytes must not reach the caller");
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_context_is_released_once_after_the_last_clone() {
        let (reader, context) = make_reader(context_for(b"0123456789".to_vec()));
        let clone = reader.clone();
        let another = clone.clone();
        expect_eq!(*context.released.lock().unwrap(), 0);

        drop(reader);
        drop(clone);
        expect_eq!(*context.released.lock().unwrap(), 0, "context released while a clone was live");

        drop(another);
        expect_eq!(*context.released.lock().unwrap(), 1);
    }

    #[gtest]
    fn test_clones_have_independent_positions() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        reader.seek(SeekFrom::Start(5)).unwrap();
        let mut clone = reader.clone();
        clone.seek(SeekFrom::Start(0)).unwrap();

        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).unwrap();
        expect_eq!(byte[0], b'5');
        clone.read_exact(&mut byte).unwrap();
        expect_eq!(byte[0], b'0');
    }

    #[gtest]
    fn test_clones_can_be_read_from_different_threads() {
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let mut context = context_for(data.clone());
        // Small reads keep both threads calling back for a while, so their
        // `read_at` calls actually interleave.
        context.max_read = 7;
        let (mut reader, context) = make_reader(context);
        let mut clone = reader.clone();

        let worker = std::thread::spawn(move || {
            let mut got = Vec::new();
            clone.read_to_end(&mut got).unwrap();
            got
        });
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        let got_on_worker = worker.join().unwrap();

        expect_eq!(got, data);
        expect_eq!(got_on_worker, data);
        expect_eq!(*context.released.lock().unwrap(), 0, "the main-thread reader is still live");
        drop(reader);
        expect_eq!(*context.released.lock().unwrap(), 1);
    }

    #[gtest]
    fn test_failure_to_determine_size_releases_the_context() {
        let context = context_for(b"0123456789".to_vec());
        let released = Arc::clone(&context.released);
        let (token, _) = register_context(context);
        let reader = CallbackReader::try_new(test_read_at, test_size_fails, test_release, token);

        expect_eq!(reader.unwrap_err().code, ZipErrorCode::Internal);
        // Ownership transfers on call, so a failed construction must not leak.
        expect_eq!(*released.lock().unwrap(), 1);
    }
}
