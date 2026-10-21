// Crubit does not support generic type parameters, so each concrete reader that
// a zip archive can be built on needs its own set of wrapper types. This module
// provides the reader that delegates to caller-supplied C callbacks, which is
// what lets C++ open an archive over storage this crate knows nothing about
// (for example a remote object read in ranges). The archive and file types
// built on it are `CallbackZipArchive` in `read.rs` and `CallbackZipFile` in
// `file.rs`.

use crate::ZipError;
use std::ffi::c_void;
use std::fmt::{Debug, Formatter};
use std::io::{Error, ErrorKind, Read, Result as IoResult, Seek, SeekFrom};
use std::sync::Arc;

/// The callbacks and the opaque context they operate on.
///
/// Held behind an `Arc` so that an archive and all of its clones share one
/// context and release it exactly once, when the last of them is dropped.
pub(crate) struct CallbackContext {
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
        // of `CallbackReader::new`, whose contract requires that `release`
        // accepts `context`. `CallbackContext` is only ever reachable through an
        // `Arc`, so this runs exactly once.
        unsafe { (self.release)(self.context) }
    }
}

// SAFETY: `CallbackReader::new` is unsafe and requires the caller to guarantee
// that the callbacks may be invoked from any thread. Raw pointers are neither
// `Send` nor `Sync` by default, so that guarantee is asserted here on the
// caller's behalf; without it a `CallbackZipArchive` could not be moved between
// threads even though the underlying reader supports it.
unsafe impl Send for CallbackContext {}
// SAFETY: `CallbackContext` is immutable after construction and the caller of
// `CallbackReader::new` guarantees that the callbacks may be invoked from any
// thread, so sharing `&CallbackContext` across threads via `Arc` is sound.
unsafe impl Sync for CallbackContext {}

/// A seekable reader whose bytes come from caller-supplied callbacks.
///
/// Cloning is cheap and performs no I/O: clones share one context and one
/// cached length, and only the read position is per-clone. That is what makes
/// `CallbackZipArchive::try_clone` free, which in turn lets a caller hold one
/// archive handle per concurrently open entry.
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
    /// exactly once, after the last clone of the reader is dropped.
    ///
    /// # Safety
    ///
    /// The caller must guarantee all of the following.
    ///
    /// * `context` is valid until `release` is called with it.
    /// * All three callbacks accept `context` and may be invoked from any thread, though never
    ///   concurrently by this reader.
    /// * `read_at` writes no more than `len` bytes to `buf` and returns at most `len`.
    /// * None of the callbacks unwind. A C++ exception crossing into Rust is undefined behavior.
    pub(crate) unsafe fn new(
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
        // the pointer and length passed to `read_at`.
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
                "Cannot seek to a negative position in a zip archive",
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
    use googletest::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test context backed by an in-memory buffer, which records how many bytes
    /// were read and whether it was released.
    struct TestContext {
        data: Vec<u8>,
        bytes_read: AtomicUsize,
        released: Arc<AtomicUsize>,
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
        context.bytes_read.fetch_add(len, Ordering::Relaxed);
        len as i64
    }

    unsafe extern "C" fn test_size(context: *mut c_void) -> i64 {
        // SAFETY: tests only ever pass a pointer to a live `TestContext`.
        let context = unsafe { &*(context as *const TestContext) };
        context.data.len() as i64
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
            unsafe { CallbackReader::new(test_read_at, test_size, test_release, context) }.unwrap();
        (reader, released)
    }

    fn context_for(data: Vec<u8>) -> TestContext {
        TestContext {
            data,
            bytes_read: AtomicUsize::new(0),
            released: Arc::new(AtomicUsize::new(0)),
            max_read: usize::MAX,
            fail_reads: false,
            overreport: false,
        }
    }

    #[gtest]
    fn test_reads_the_whole_input() {
        let (mut reader, _) = make_reader(context_for(b"Hello, world!".to_vec()));
        let mut got = Vec::new();
        reader.read_to_end(&mut got).unwrap();
        expect_eq!(got.as_slice(), b"Hello, world!");
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
        expect_true!(reader.seek(SeekFrom::End(-11)).is_err());
        expect_true!(reader.seek(SeekFrom::Current(-1)).is_err());
    }

    #[gtest]
    fn test_seek_past_the_end_reports_end_of_input() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        // `zip` seeks around while looking for the end-of-central-directory
        // record, so this must behave like `File` and not fail until a read.
        expect_eq!(reader.seek(SeekFrom::Start(1000)).unwrap(), 1000);
        let mut buf = [0u8; 4];
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[gtest]
    fn test_reads_never_run_past_the_end() {
        let (mut reader, _) = make_reader(context_for(b"0123456789".to_vec()));
        reader.seek(SeekFrom::Start(8)).unwrap();
        let mut buf = [0u8; 100];
        expect_eq!(reader.read(&mut buf).unwrap(), 2);
        expect_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[gtest]
    fn test_failing_read_is_reported() {
        let mut context = context_for(b"0123456789".to_vec());
        context.fail_reads = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        expect_true!(reader.read(&mut buf).is_err());
    }

    #[gtest]
    fn test_callback_claiming_too_many_bytes_is_rejected() {
        let mut context = context_for(b"0123456789".to_vec());
        context.overreport = true;
        let (mut reader, _) = make_reader(context);
        let mut buf = [0u8; 4];
        // Believing the callback here would hand uninitialized bytes to the
        // caller, so it must be an error rather than a silent truncation.
        expect_true!(reader.read(&mut buf).is_err());
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
    fn test_failure_to_determine_size_releases_the_context() {
        let context = context_for(b"0123456789".to_vec());
        let released = context.released.clone();
        let context = Box::into_raw(Box::new(context)) as *mut c_void;
        // SAFETY: the callbacks accept the boxed `TestContext` and do not unwind.
        let reader =
            unsafe { CallbackReader::new(test_read_at, test_size_fails, test_release, context) };

        expect_true!(reader.is_err());
        // Ownership transfers on call, so a failed construction must not leak.
        expect_eq!(released.load(Ordering::Relaxed), 1);
    }
}
