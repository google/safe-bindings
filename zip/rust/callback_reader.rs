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
use std::sync::Arc;

pub(crate) type ReadAtCallback = extern "C" fn(usize, u64, usize) -> i64;
pub(crate) type SizeCallback = extern "C" fn(usize) -> i64;
pub(crate) type ReleaseCallback = extern "C" fn(usize);

const MAX_RETAINED_STAGED_CAPACITY: usize = 1 << 20; // 1 MiB

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

/// The callbacks and the opaque caller token they operate on.
///
/// Held behind an `Arc` so that an archive and all of its clones share one
/// context and release it exactly once, when the last of them is dropped.
struct CallbackContext {
    read_at: ReadAtCallback,
    size: SizeCallback,
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
/// archive raised on its own must be reported as is, even if a callback
/// failed earlier in the same operation and the archive recovered from it.
#[derive(Debug)]
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
}

impl Clone for CallbackReader {
    fn clone(&self) -> Self {
        if let Some(replacement) = self.clone_slot.take_staged() {
            return replacement;
        }
        Self {
            context: Arc::clone(&self.context),
            clone_slot: Arc::clone(&self.clone_slot),
            len: self.len,
            pos: self.pos,
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
    /// return their count; it must return 0 only at end of input, and a
    /// negative value if the read failed. It may return fewer bytes than
    /// requested. `size` must return the total number of bytes available, or a
    /// negative value on failure. `release` is called with `context` exactly
    /// once: after the last clone of the reader is dropped, or before this
    /// function returns an error.
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
        let context = Arc::new(CallbackContext { read_at, size, release, context });
        let len = (context.size)(context.context);
        if len < 0 {
            return Err(ZipError::internal("Failed to determine the size of the zip archive")
                .caused_by_reader_callback());
        }
        Ok(Self { context, clone_slot: Arc::new(CloneSlot::default()), len: len as u64, pos: 0 })
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
}

impl Read for CallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        // Never ask for more than what remains. The caller's `read_at` is also
        // expected to stop at the end, but clamping here means a caller that
        // clamps differently still cannot make this reader advance past `len`.
        //
        // Also never ask for more than `MAX_RETAINED_STAGED_CAPACITY` at once.
        // The callback typically allocates a temporary buffer of the requested
        // length on the C++ side and the staged copy on this side is another
        // one, so this bounds both however large `buf` is. `Read` permits
        // short reads, and so does this reader's contract, so a caller wanting
        // more simply calls again.
        let remaining = self.len - self.pos;
        let len = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len())
            .min(MAX_RETAINED_STAGED_CAPACITY);
        clear_staged_read_bytes();
        let read = (self.context.read_at)(self.context.context, self.pos, len);
        if read < 0 {
            clear_staged_read_bytes();
            return Err(Error::other(CallbackFailed));
        }
        let read = read as u64;
        if read > len as u64 {
            clear_staged_read_bytes();
            // Trusting this would mean reporting bytes the callback never wrote.
            return Err(Error::new(
                ErrorKind::InvalidData,
                "The zip archive read callback reported reading more bytes than requested",
            ));
        }
        let read = read as usize;
        if read == 0 {
            clear_staged_read_bytes();
            return Ok(0);
        }
        STAGED_READ_BYTES.with(|cell| {
            let mut staged = cell.borrow_mut();
            if staged.len() != read {
                clear_staged_vec(&mut staged);
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "The zip archive read callback staged a byte count that does not match its return value",
                ));
            }
            buf[..read].copy_from_slice(&staged[..read]);
            clear_staged_vec(&mut staged);
            self.pos += read as u64;
            Ok(read)
        })
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{LazyLock, Mutex};

    /// Test context backed by an in-memory buffer, which records the length of
    /// every `read_at` request and whether it was released.
    ///
    /// Shared between concurrently used clones of the reader, so all mutable
    /// state is behind atomics or a `Mutex`.
    struct TestContext {
        data: Vec<u8>,
        /// The `len` argument of every `read_at` call, in order.
        requested_lens: Mutex<Vec<usize>>,
        released: Arc<AtomicUsize>,
        /// What `size` reports. Lets tests under-report the available bytes.
        reported_size: i64,
        /// Largest number of bytes any single `read_at` will serve, so tests can
        /// exercise the short-read path.
        max_read: usize,
        /// When true, `read_at` fails instead of serving bytes.
        fail_reads: bool,
        /// When true, `read_at` claims to have read more than it was asked for.
        overreport: bool,
        /// When true, `read_at` returns a positive count without staging bytes.
        skip_staging: bool,
    }

    static NEXT_TEST_TOKEN: AtomicUsize = AtomicUsize::new(1);
    static TEST_CONTEXTS: LazyLock<Mutex<HashMap<usize, Arc<TestContext>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    fn register_context(context: TestContext) -> (usize, Arc<TestContext>) {
        let token = NEXT_TEST_TOKEN.fetch_add(1, Ordering::Relaxed);
        let context = Arc::new(context);
        TEST_CONTEXTS.lock().unwrap().insert(token, Arc::clone(&context));
        (token, context)
    }

    fn lookup_context(token: usize) -> Arc<TestContext> {
        TEST_CONTEXTS.lock().unwrap().get(&token).cloned().expect("live test context")
    }

    extern "C" fn test_read_at(token: usize, offset: u64, len: usize) -> i64 {
        let context = lookup_context(token);
        context.requested_lens.lock().unwrap().push(len);
        if context.fail_reads {
            return -1;
        }
        if context.overreport {
            return len as i64 + 1;
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
        let context = TEST_CONTEXTS.lock().unwrap().remove(&token).expect("context released once");
        context.released.fetch_add(1, Ordering::Relaxed);
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
            released: Arc::new(AtomicUsize::new(0)),
            max_read: usize::MAX,
            fail_reads: false,
            overreport: false,
            skip_staging: false,
        }
    }

    fn requested_lens(context: &TestContext) -> Vec<usize> {
        context.requested_lens.lock().unwrap().clone()
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
        // 0 while the reader still expects bytes. The reader passes the 0
        // through as end of input without advancing, and `read_exact` (what
        // `zip` uses for headers) then reports the input as truncated.
        context.reported_size = 10;
        let (mut reader, context) = make_reader(context);
        expect_eq!(reader.len(), 10);

        let mut buf = [0u8; 10];
        expect_eq!(reader.read(&mut buf).unwrap(), 5);
        expect_eq!(&buf[..5], b"01234");
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_eq!(reader.stream_position().unwrap(), 5, "a 0-byte read must not advance");
        expect_eq!(reader.read_exact(&mut buf[..1]).unwrap_err().kind(), ErrorKind::UnexpectedEof);
        // Each of the three reads consulted the callback: nothing is cached.
        expect_eq!(requested_lens(&context), vec![10, 5, 1]);
    }

    #[gtest]
    fn test_staged_buffer_capacity_is_given_back_after_a_large_read() {
        // A callback may stage more than `MAX_RETAINED_STAGED_CAPACITY` (the
        // reader rejects the excess, but the bytes have been copied by then).
        // The thread-local must not keep that much memory around afterwards.
        let oversized = vec![0u8; 3 * MAX_RETAINED_STAGED_CAPACITY];
        stage_callback_chunk(&oversized);
        let capacity_before = STAGED_READ_BYTES.with(|cell| cell.borrow().capacity());
        expect_ge!(capacity_before, oversized.len());

        clear_staged_read_bytes();
        let (len_after, capacity_after) =
            STAGED_READ_BYTES.with(|cell| (cell.borrow().len(), cell.borrow().capacity()));
        expect_eq!(len_after, 0);
        expect_le!(capacity_after, MAX_RETAINED_STAGED_CAPACITY);
    }

    #[gtest]
    fn test_large_reads_are_requested_in_bounded_chunks() {
        // 4 MiB of input, read with a 4 MiB buffer: no single `read_at` may be
        // asked for more than `MAX_RETAINED_STAGED_CAPACITY`, since the
        // request bounds both the callback's temporary buffer and the staged
        // copy, yet looping over the short reads must still yield everything.
        let data: Vec<u8> = (0..=255u8).cycle().take(4 << 20).collect();
        let (mut reader, context) = make_reader(context_for(data.clone()));
        let mut buf = vec![0u8; 4 << 20];

        let first = reader.read(&mut buf).unwrap();
        expect_eq!(first, MAX_RETAINED_STAGED_CAPACITY);
        let mut total = first;
        while total < data.len() {
            let n = reader.read(&mut buf[total..]).unwrap();
            expect_gt!(n, 0);
            total += n;
        }
        expect_eq!(total, data.len());
        expect_eq!(buf, data);
        expect_eq!(reader.read(&mut buf).unwrap(), 0);

        let lens = requested_lens(&context);
        expect_eq!(lens.len(), (4 << 20) / MAX_RETAINED_STAGED_CAPACITY);
        for len in lens {
            expect_le!(len, MAX_RETAINED_STAGED_CAPACITY);
        }

        // `read_to_end` goes through the same path.
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got, data);
        for len in requested_lens(&context) {
            expect_le!(len, MAX_RETAINED_STAGED_CAPACITY);
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
    fn test_context_is_released_once_after_the_last_clone() {
        let (reader, context) = make_reader(context_for(b"0123456789".to_vec()));
        let clone = reader.clone();
        let another = clone.clone();
        expect_eq!(context.released.load(Ordering::Relaxed), 0);

        drop(reader);
        drop(clone);
        expect_eq!(
            context.released.load(Ordering::Relaxed),
            0,
            "context released while a clone was live"
        );

        drop(another);
        expect_eq!(context.released.load(Ordering::Relaxed), 1);
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
        expect_eq!(
            context.released.load(Ordering::Relaxed),
            0,
            "the main-thread reader is still live"
        );
        drop(reader);
        expect_eq!(context.released.load(Ordering::Relaxed), 1);
    }

    #[gtest]
    fn test_failure_to_determine_size_releases_the_context() {
        let context = context_for(b"0123456789".to_vec());
        let released = Arc::clone(&context.released);
        let (token, _) = register_context(context);
        let reader = CallbackReader::try_new(test_read_at, test_size_fails, test_release, token);

        expect_eq!(reader.unwrap_err().code, ZipErrorCode::Internal);
        // Ownership transfers on call, so a failed construction must not leak.
        expect_eq!(released.load(Ordering::Relaxed), 1);
    }
}
