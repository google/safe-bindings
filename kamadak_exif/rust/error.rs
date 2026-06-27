use crubit_annotate::cpp_enum;
use exif::Error as KamadakError;
use std::fmt;
use std::io;

#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct ErrorStatus(i32);

impl ErrorStatus {
    /// Input data was malformed or truncated.
    pub const INVALID_FORMAT: ErrorStatus = ErrorStatus(0);
    /// Input data could not be read due to an I/O error.
    pub const IO: ErrorStatus = ErrorStatus(1);
    /// Exif attribute information was not found in an image file
    /// such as JPEG.
    pub const NOT_FOUND: ErrorStatus = ErrorStatus(2);
    /// The value of the field is blank.  Some fields have blank values
    /// whose meanings are defined as "unknown".  Such a blank value
    /// should be treated the same as the absence of the field.
    pub const BLANK_VALUE: ErrorStatus = ErrorStatus(3);
    /// Field values or image data are too big to encode.
    pub const TOO_BIG: ErrorStatus = ErrorStatus(4);
    /// The field type is not supported and can not be encoded.
    pub const NOT_SUPPORTED: ErrorStatus = ErrorStatus(5);
    /// The field has an unexpected value.
    pub const UNEXPECTED_VALUE: ErrorStatus = ErrorStatus(6);
    /// Partially-parsed result and errors.
    pub const PARTIAL_RESULT: ErrorStatus = ErrorStatus(7);
    /// Unknown error.
    pub const UNKNOWN: ErrorStatus = ErrorStatus(8);
}

#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Error {
    status: ErrorStatus,
    message: crate::types::VecU8,
}

/// An error returned when parsing of Exif data fails.
impl Error {
    /// Returns the status of the error as an ErrorStatus enum.
    pub fn status(&self) -> ErrorStatus {
        self.status
    }

    /// Returns the message of the error.
    pub fn message(&self) -> crate::types::VecU8 {
        self.message.clone()
    }
}

impl From<KamadakError> for Error {
    fn from(error: KamadakError) -> Self {
        let status = match error {
            KamadakError::InvalidFormat(_) => ErrorStatus::INVALID_FORMAT,
            KamadakError::Io(_) => ErrorStatus::IO,
            KamadakError::NotFound(_) => ErrorStatus::NOT_FOUND,
            KamadakError::BlankValue(_) => ErrorStatus::BLANK_VALUE,
            KamadakError::TooBig(_) => ErrorStatus::TOO_BIG,
            KamadakError::NotSupported(_) => ErrorStatus::NOT_SUPPORTED,
            KamadakError::UnexpectedValue(_) => ErrorStatus::UNEXPECTED_VALUE,
            KamadakError::PartialResult(_) => ErrorStatus::PARTIAL_RESULT,
            _ => ErrorStatus::UNKNOWN,
        };
        Self { status, message: crate::types::VecU8(error.to_string().into_bytes()) }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::from(KamadakError::Io(err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = std::str::from_utf8(self.message.as_slice()).unwrap_or("Unknown error");
        write!(f, "{}", s)
    }
}
