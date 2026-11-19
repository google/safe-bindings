use crate::callback_reader::{CallbackFailed, CALLBACK_FAILED_MESSAGE};
use crate::VecU8;
use std::io::ErrorKind;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipErrorCode {
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    FailedPrecondition,
    OutOfRange,
    Unimplemented,
    Internal,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipError {
    pub message: VecU8,
    pub code: ZipErrorCode,
    /// Whether this error reports a `CallbackZipArchive` callback (`read_at`
    /// or `size`) that returned a failure. See `is_reader_callback_failure`.
    from_reader_callback: bool,
}

impl ZipError {
    #[must_use]
    pub fn new(message: impl Into<VecU8>, code: ZipErrorCode) -> Self {
        Self { message: message.into(), code, from_reader_callback: false }
    }

    /// Returns whether this error is the direct result of a
    /// `CallbackZipArchive` callback reporting a failure (a negative return
    /// from `read_at` or `size`), as opposed to an error the archive raised on
    /// its own, such as a corrupt header or a failed CRC check.
    ///
    /// The caller that supplied the callbacks knows why they failed and
    /// typically has a more useful error to report than this one; this tells
    /// it when that applies. It is false for an archive error even if a
    /// callback failed earlier during the same operation and the archive
    /// recovered from it, as `zip` does while locating the central directory.
    #[must_use]
    pub fn is_reader_callback_failure(&self) -> bool {
        self.from_reader_callback
    }

    #[must_use]
    pub fn invalid_argument(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::InvalidArgument)
    }

    #[must_use]
    pub fn deadline_exceeded(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::DeadlineExceeded)
    }

    #[must_use]
    pub fn not_found(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::NotFound)
    }

    #[must_use]
    pub fn already_exists(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::AlreadyExists)
    }

    #[must_use]
    pub fn permission_denied(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::PermissionDenied)
    }

    #[must_use]
    pub fn failed_precondition(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::FailedPrecondition)
    }

    #[must_use]
    pub fn out_of_range(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::OutOfRange)
    }

    #[must_use]
    pub fn unimplemented(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::Unimplemented)
    }

    #[must_use]
    pub fn internal(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::Internal)
    }
}

impl ZipError {
    /// Marks this error as the direct result of a failed reader callback; see
    /// `is_reader_callback_failure`.
    #[must_use]
    pub(crate) fn caused_by_reader_callback(mut self) -> Self {
        self.from_reader_callback = true;
        self
    }

    /// Prepends `context` to the error message while preserving `self.code`
    /// and `is_reader_callback_failure()`.
    #[must_use]
    pub(crate) fn with_context(mut self, context: &str) -> Self {
        self.message =
            format!("{}: {}", context, String::from_utf8_lossy(self.message.as_slice())).into();
        self
    }

    /// Validates that `path` is non-empty, contains no embedded NUL bytes, and
    /// is valid UTF-8, returning an `InvalidArgument` error prefixed with
    /// `context` otherwise.
    pub(crate) fn validate_non_empty_utf8_path<'a>(
        path: &'a [u8],
        context: &str,
    ) -> Result<&'a str, Self> {
        if path.is_empty() {
            return Err(Self::invalid_argument(format!("{}: path must not be empty", context)));
        }
        if path.contains(&0) {
            return Err(Self::invalid_argument(format!(
                "{}: path must not contain NUL bytes",
                context
            )));
        }
        std::str::from_utf8(path).map_err(|e| Self::invalid_argument(format!("{}: {}", context, e)))
    }

    /// Converts a `std::io::Error` to `ZipError`, preserving canonical OS error
    /// kinds (`NotFound`, `PermissionDenied`, `AlreadyExists`, OS
    /// `InvalidInput`, `TimedOut`, `Unsupported`) and mapping other
    /// stream/decoder I/O errors (such as `InvalidData`, `UnexpectedEof`, and
    /// synthetic `InvalidInput` from decompression libraries like `flate2` and
    /// `bzip2`) to `Internal`. An error carrying `CallbackFailed`, which is how
    /// `CallbackReader` reports a failed `read_at`, is marked as a reader
    /// callback failure.
    ///
    /// `UnexpectedEof` and `InvalidData` deliberately stay `Internal` rather
    /// than becoming `InvalidArgument`: they are raised by the decoders and
    /// CRC-32 check while reading entry data that already passed header
    /// parsing (a truncated or corrupt payload, a CRC mismatch), existing
    /// callers and the C++ headers (`file.h`, `read.h`) document `kInternal`
    /// for exactly those conditions, and header-level corruption is already
    /// reported as `InvalidArgument` by `from_zip_error` via
    /// `ZipError::InvalidArchive`.
    ///
    /// The `CallbackFailed` payload does not always survive the trip from
    /// `CallbackReader` to here: `lzma_rust2`'s `StickyError` (which re-raises
    /// a decoder failure as `io::Error::new(kind, err.to_string())`, so any
    /// LZMA entry read past the decoder's first 8 KiB buffer fill) and
    /// `zip::result::ZipError::clone()` (`io::Error::new(kind, to_string())`)
    /// both keep only the kind and the `Display` text. Those rebuilt errors
    /// are recognised by their text instead: `ErrorKind::Other`, no OS error
    /// code, and exactly `CALLBACK_FAILED_MESSAGE`. Without that, a reader
    /// failure on such an entry would surface as a generic `Internal` error
    /// instead of the reader's own status.
    #[must_use]
    pub(crate) fn from_io_error(err: std::io::Error) -> Self {
        let message = err.to_string();
        let from_reader_callback = err.get_ref().is_some_and(|inner| inner.is::<CallbackFailed>())
            || (err.kind() == ErrorKind::Other
                && err.raw_os_error().is_none()
                && message == CALLBACK_FAILED_MESSAGE);
        let code = match err.kind() {
            ErrorKind::NotFound => ZipErrorCode::NotFound,
            ErrorKind::PermissionDenied => ZipErrorCode::PermissionDenied,
            ErrorKind::AlreadyExists => ZipErrorCode::AlreadyExists,
            ErrorKind::InvalidInput if err.raw_os_error().is_some() => {
                ZipErrorCode::InvalidArgument
            }
            ErrorKind::TimedOut => ZipErrorCode::DeadlineExceeded,
            ErrorKind::Unsupported => ZipErrorCode::Unimplemented,
            _ => ZipErrorCode::Internal,
        };
        Self { message: message.into(), code, from_reader_callback }
    }

    /// Converts a `zip` crate error. Deliberately not a `From` impl: a public
    /// `From<zip::result::ZipError>` would make Crubit generate C++ bindings for
    /// the `zip` crate itself (and include them from zip_wrapper.h), which
    /// clients neither need nor want.
    #[must_use]
    pub(crate) fn from_zip_error(err: zip::result::ZipError) -> Self {
        match err {
            zip::result::ZipError::Io(e) => Self::from_io_error(e),
            zip::result::ZipError::InvalidArchive(e) => Self::invalid_argument(e.as_ref()),
            zip::result::ZipError::UnsupportedArchive(e) => Self::failed_precondition(e),
            zip::result::ZipError::CompressionMethodNotSupported(_) => {
                Self::unimplemented(err.to_string())
            }
            zip::result::ZipError::FileNotFound => Self::out_of_range("File not found"),
            zip::result::ZipError::InvalidPassword => Self::invalid_argument(err.to_string()),
            _ => Self::internal(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;
    use std::io;

    #[gtest]
    fn test_with_context_prepends_context_and_keeps_code() {
        let err = ZipError::not_found("No such file").with_context("Failed to open zip archive");
        expect_eq!(err.code, ZipErrorCode::NotFound);
        expect_eq!(err.message.as_slice(), b"Failed to open zip archive: No such file");
    }

    #[gtest]
    fn test_with_context_replaces_invalid_utf8_in_the_original_message() {
        let err = ZipError::internal(b"bad \xff byte".to_vec()).with_context("ctx");
        expect_eq!(err.code, ZipErrorCode::Internal);
        expect_eq!(err.message.as_slice(), "ctx: bad \u{FFFD} byte".as_bytes());
    }

    #[gtest]
    fn test_with_context_keeps_the_reader_callback_marker() {
        let err = ZipError::from_io_error(io::Error::other(CallbackFailed)).with_context("ctx");
        expect_true!(err.is_reader_callback_failure());
        expect_eq!(err.code, ZipErrorCode::Internal);
    }

    #[gtest]
    fn test_from_io_error_marks_a_callback_failed_payload() {
        let err = ZipError::from_io_error(io::Error::other(CallbackFailed));
        expect_true!(err.is_reader_callback_failure());
        expect_eq!(err.code, ZipErrorCode::Internal);
    }

    // `lzma_rust2::StickyError` and `zip::result::ZipError::clone()` rebuild
    // the error from its `Display` text, dropping the payload.
    #[gtest]
    fn test_from_io_error_marks_a_rebuilt_callback_failed_message() {
        let err = ZipError::from_io_error(io::Error::other(CALLBACK_FAILED_MESSAGE));
        expect_true!(err.is_reader_callback_failure());
        expect_eq!(err.code, ZipErrorCode::Internal);
    }

    #[gtest]
    fn test_from_io_error_does_not_mark_other_errors() {
        let other = ZipError::from_io_error(io::Error::other("something else"));
        expect_false!(other.is_reader_callback_failure());
        expect_eq!(other.code, ZipErrorCode::Internal);

        // An OS error is never a callback failure, whatever its text (ENOENT).
        let os = ZipError::from_io_error(io::Error::from_raw_os_error(2));
        expect_false!(os.is_reader_callback_failure());
        expect_eq!(os.code, ZipErrorCode::NotFound);

        // The text alone is not enough when the kind is not `Other`.
        let wrong_kind = ZipError::from_io_error(io::Error::new(
            ErrorKind::InvalidData,
            CALLBACK_FAILED_MESSAGE,
        ));
        expect_false!(wrong_kind.is_reader_callback_failure());
    }
}
