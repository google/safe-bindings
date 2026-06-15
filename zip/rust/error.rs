use crate::VecU8;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipErrorCode {
    InvalidArgument,
    OutOfRange,
    FailedPrecondition,
    Internal,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipError {
    pub message: VecU8,
    pub code: ZipErrorCode,
}

impl ZipError {
    pub fn new(message: impl Into<VecU8>, code: ZipErrorCode) -> Self {
        Self { message: message.into(), code }
    }

    pub fn invalid_argument(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::InvalidArgument)
    }

    pub fn out_of_range(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::OutOfRange)
    }

    pub fn failed_precondition(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::FailedPrecondition)
    }

    pub fn internal(message: impl Into<VecU8>) -> Self {
        Self::new(message, ZipErrorCode::Internal)
    }
}
