// Crubit does not support generic type parameters, so each concrete reader that
// a zip archive can be built on needs its own set of wrapper types. This module
// provides the reader that delegates to caller-supplied C callbacks, which is
// what lets C++ open an archive over storage this crate knows nothing about
// (for example a remote object read in ranges). The archive and file types
// built on it are added in a follow-up change.

use crate::ZipError;
use std::ffi::c_void;
use std::fmt::{Debug, Formatter};
use std::io::{Error, ErrorKind, Read, Result as IoResult, Seek, SeekFrom};
use std::sync::Arc;

/// The callbacks and the opaque context they operate on.
///
/// Held behind an `Arc` so that an archive and all of its clones share one
/// context and release it exactly once, when the last of them is dropped.
struct CallbackContext {
    read_at: unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i64,
    size: unsafe extern "C" fn(*mut c_void) -> i64,
    release: unsafe extern "C" fn(*mut c_void),
    context: *mut c_void,
}

impl Debug for CallbackContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackContext").finish_non_exhaustive()
    }
}

impl Drop for CallbackContext {
    fn drop(&mut self) {
        // SAFETY: `context` and `release` were supplied together by the caller
        // of `CallbackReader::try_new`, whose contract requires that `release`
        // accepts `context`. `CallbackContext` is only ever reachable through an
        // `Arc`, so this runs exactly once.
        unsafe { (self.release)(self.context) }
    }
}

// SAFETY: `CallbackReader::try_new` is unsafe and requires the caller to
// guarantee that the callbacks may be invoked from any thread. Raw pointers are
// neither `Send` nor `Sync` by default, so that guarantee is asserted here on
// the caller's behalf; without it an archive built on this reader could not be
// moved between threads even though the underlying reader supports it.
unsafe impl Send for CallbackContext {}
// SAFETY: The fields of `CallbackContext` itself are never mutated after
// construction. The state behind the opaque `context` pointer is the caller's,
// and the contract of `CallbackReader::try_new` requires `read_at` to be safe
// to call concurrently from any thread on that state (`size` runs once before
// any clone exists and `release` after the last one is gone), so sharing
// `&CallbackContext` across threads via `Arc` is sound.
unsafe impl Sync for CallbackContext {}

/// A seekable reader whose bytes come from caller-supplied callbacks.
///
/// Cloning is cheap and performs no I/O: clones share one context and one
/// cached length, and only the read position is per-clone. That is what lets
/// the archive type built on it in a follow-up change offer a free
/// `try_clone`, which in turn lets a caller hold one archive handle per
/// concurrently open entry.
///
/// `pub(crate)` rather than `pub`: Crubit cannot bind the `Read`/`Seek` traits,
/// so C++ only ever sees the archive wrappers built on top of this reader.
#[derive(Clone, Debug)]
pub(crate) struct CallbackReader {
    context: Arc<CallbackContext>,
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

impl CallbackReader {
    /// Creates a reader that obtains its bytes by invoking `read_at` on
    /// `context`.
    ///
    /// `read_at` must read up to `len` bytes from `context` starting at
    /// `offset` into `buf` and return the number of bytes read; it must return
    /// 0 only at end of input, and a negative value if the read failed. It may
    /// return fewer bytes than requested. `size` must return the total number
    /// of bytes available, or a negative value on failure. `release` is called
    /// exactly once: after the last clone of the reader is dropped, or before
    /// this function returns an error.
    ///
    /// # Safety
    ///
    /// The caller must guarantee all of the following.
    ///
    /// * `context` is valid until `release` is called with it.
    /// * All three callbacks accept `context` and may be invoked from any thread. Because clones of
    ///   this reader share `context` and are meant to be used from different threads at once (one
    ///   archive handle per concurrently open entry), `read_at` must be safe to call concurrently
    ///   with itself. `size` is called once, from this function, before any clone exists, and
    ///   `release` runs after the last clone is dropped, so neither overlaps another callback.
    /// * `read_at` writes no more than `len` bytes to `buf` and returns at most `len`.
    /// * None of the callbacks unwind. A C++ exception crossing into Rust is undefined behavior.
    pub(crate) unsafe fn try_new(
        read_at: unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i64,
        size: unsafe extern "C" fn(*mut c_void) -> i64,
        release: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> Result<Self, ZipError> {
        // Take ownership before anything else can fail, so that `release` runs
        // even if determining the length does not succeed.
        let context = Arc::new(CallbackContext { read_at, size, release, context });
        // SAFETY: by this function's contract, `size` accepts `context`.
        let len = unsafe { (context.size)(context.context) };
        if len < 0 {
            return Err(ZipError::internal("Failed to determine the size of the zip archive"));
        }
        Ok(Self { context, len: len as u64, pos: 0 })
    }

    /// Returns the total number of bytes available, as reported by `size` when
    /// the reader was created.
    #[must_use]
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// Returns true if the reader has zero available bytes.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
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
        let remaining = self.len - self.pos;
        let len = std::cmp::min(buf.len() as u64, remaining) as usize;
        // SAFETY: `buf` is valid for writes of `buf.len()` bytes and `len` is at
        // most `buf.len()`, which is what this function's contract requires of
        // the pointer and length passed to `read_at`. `context` is still live:
        // the `Arc` keeps `CallbackContext`, and with it the caller's `context`
        // (valid until `release`, per `try_new`'s contract), alive for this
        // call.
        let read = unsafe {
            (self.context.read_at)(self.context.context, self.pos, buf.as_mut_ptr(), len)
        };
        if read < 0 {
            return Err(Error::other("The zip archive read callback failed"));
        }
        let read = read as u64;
        if read > len as u64 {
            // Trusting this would mean reporting bytes the callback never wrote.
            return Err(Error::new(
                ErrorKind::InvalidData,
                "The zip archive read callback reported reading more bytes than requested",
            ));
        }
        self.pos += read;
        Ok(read as usize)
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

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
    }

    unsafe extern "C" fn test_read_at(
        context: *mut c_void,
        offset: u64,
        buf: *mut u8,
        len: usize,
    ) -> i64 {
        // SAFETY: tests only ever pass a pointer to a live `TestContext`.
        let context = unsafe { &*(context as *const TestContext) };
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
        // SAFETY: the caller guarantees `buf` is valid for `len` bytes, and
        // `len` was clamped to what is available.
        unsafe { std::ptr::copy_nonoverlapping(context.data[offset..].as_ptr(), buf, len) };
        len as i64
    }

    unsafe extern "C" fn test_size(context: *mut c_void) -> i64 {
        // SAFETY: tests only ever pass a pointer to a live `TestContext`.
        let context = unsafe { &*(context as *const TestContext) };
        context.reported_size
    }

    unsafe extern "C" fn test_size_fails(_context: *mut c_void) -> i64 {
        -1
    }

    unsafe extern "C" fn test_release(context: *mut c_void) {
        // SAFETY: the pointer came from `Box::into_raw` in `make_reader`, and
        // `release` is called at most once.
        let context = unsafe { Box::from_raw(context as *mut TestContext) };
        context.released.fetch_add(1, Ordering::Relaxed);
    }

    /// Builds a reader over `data`. Returns the reader and a counter that the
    /// release callback increments, so tests can assert on ownership.
    fn make_reader(context: TestContext) -> (CallbackReader, Arc<AtomicUsize>) {
        let released = context.released.clone();
        let context = Box::into_raw(Box::new(context)) as *mut c_void;
        // SAFETY: the callbacks all accept the boxed `TestContext`, none of them
        // unwind, and `test_release` frees it exactly once.
        let reader =
            unsafe { CallbackReader::try_new(test_read_at, test_size, test_release, context) }
                .unwrap();
        (reader, released)
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
        }
    }

    /// Returns the `len` of every `read_at` call made so far on the context
    /// behind `reader`.
    fn requested_lens(reader: &CallbackReader) -> Vec<usize> {
        // SAFETY: `make_reader` built the context from a `TestContext`, and the
        // `Arc` behind `reader` keeps it alive.
        let context = unsafe { &*(reader.context.context as *const TestContext) };
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
        expect_that!(reader.is_empty(), eq(false));
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got.as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_empty_reader_reports_empty_without_calling_back() {
        let (mut reader, _) = make_reader(context_for(Vec::new()));
        expect_eq!(reader.len(), 0);
        expect_that!(reader.is_empty(), eq(true));
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_that!(requested_lens(&reader), is_empty());
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
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        // `zip` seeks around while looking for the end-of-central-directory
        // record, so this must behave like `File` and not fail until a read.
        expect_eq!(reader.seek(SeekFrom::Start(1000)).unwrap(), 1000);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_that!(requested_lens(&reader), is_empty(), "read past the end must not call back");
    }

    #[gtest]
    fn test_reads_never_run_past_the_end() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        reader.seek(SeekFrom::Start(2)).unwrap();
        let mut buf = [0u8; 100];
        expect_eq!(reader.read(&mut buf).unwrap(), 8);
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        // The request is clamped to the 8 bytes that remain, and once the
        // position reaches the end the callback is not consulted at all.
        expect_eq!(requested_lens(&reader), vec![8]);
    }

    #[gtest]
    fn test_under_reported_size_bounds_the_reader() {
        let mut context = context_for(b"0123456789".to_vec());
        // `size` is the authority on the length: if it reports fewer bytes
        // than `read_at` could serve, the reader never asks for the rest.
        context.reported_size = 5;
        let (mut reader, _) = make_reader(context);
        expect_eq!(reader.len(), 5);

        let mut buf = [0u8; 100];
        expect_eq!(reader.read(&mut buf).unwrap(), 5);
        expect_eq!(&buf[..5], b"01234");
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
        expect_eq!(reader.stream_position().unwrap(), 5);
        expect_eq!(requested_lens(&reader), vec![5], "only one clamped request is expected");
    }

    #[gtest]
    fn test_failing_read_is_reported() {
        let mut context = context_for(b"0123456789".to_vec());
        context.fail_reads = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::Other);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_callback_claiming_too_many_bytes_is_rejected() {
        let mut context = context_for(b"0123456789".to_vec());
        context.overreport = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        // Believing the callback here would hand uninitialized bytes to the
        // caller, so it must be an error rather than a silent truncation.
        expect_eq!(reader.read(&mut buf).unwrap_err().kind(), ErrorKind::InvalidData);
        expect_eq!(reader.stream_position().unwrap(), 0, "a failed read must not advance");
    }

    #[gtest]
    fn test_context_is_released_once_after_the_last_clone() {
        let (reader, released) = make_reader(context_for(b"0123456789".to_vec()));
        let clone = reader.clone();
        let another = clone.clone();
        expect_eq!(released.load(Ordering::Relaxed), 0);

        drop(reader);
        drop(clone);
        expect_eq!(released.load(Ordering::Relaxed), 0, "context released while a clone was live");

        drop(another);
        expect_eq!(released.load(Ordering::Relaxed), 1);
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
        let (mut reader, released) = make_reader(context);
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
        expect_eq!(released.load(Ordering::Relaxed), 0, "the main-thread reader is still live");
        drop(reader);
        expect_eq!(released.load(Ordering::Relaxed), 1);
    }

    #[gtest]
    fn test_failure_to_determine_size_releases_the_context() {
        let context = context_for(b"0123456789".to_vec());
        let released = context.released.clone();
        let context = Box::into_raw(Box::new(context)) as *mut c_void;
        // SAFETY: the callbacks accept the boxed `TestContext` and do not unwind.
        let reader = unsafe {
            CallbackReader::try_new(test_read_at, test_size_fails, test_release, context)
        };

        expect_eq!(reader.unwrap_err().code, ZipErrorCode::Internal);
        // Ownership transfers on call, so a failed construction must not leak.
        expect_eq!(released.load(Ordering::Relaxed), 1);
    }
}
