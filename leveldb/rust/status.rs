// NOTE: b/351976355 - This file provides a workaround for passing structured error
// codes from Rust to C++ via Crubit. Once Crubit supports this natively, this
// should be refactored to use the native support.

use rusty_leveldb::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum LevelDBErrorCode {
    NotFound = 1,
    Corruption = 2,
    NotSupported = 3,
    InvalidArgument = 4,
    IOError = 5,
    AlreadyExists = 6,
    CompressionError = 7,
    InvalidData = 8,
    LockError = 9,
    AsyncError = 10,
    PermissionDenied = 11,
    Unknown = 12,
}

impl From<StatusCode> for LevelDBErrorCode {
    fn from(code: StatusCode) -> Self {
        match code {
            StatusCode::OK => LevelDBErrorCode::Unknown, // Should not happen in Err
            StatusCode::NotFound => LevelDBErrorCode::NotFound,
            StatusCode::Corruption => LevelDBErrorCode::Corruption,
            StatusCode::NotSupported => LevelDBErrorCode::NotSupported,
            StatusCode::InvalidArgument => LevelDBErrorCode::InvalidArgument,
            StatusCode::IOError => LevelDBErrorCode::IOError,
            StatusCode::AlreadyExists => LevelDBErrorCode::AlreadyExists,
            StatusCode::CompressionError => LevelDBErrorCode::CompressionError,
            StatusCode::InvalidData => LevelDBErrorCode::InvalidData,
            StatusCode::LockError => LevelDBErrorCode::LockError,
            StatusCode::AsyncError => LevelDBErrorCode::AsyncError,
            StatusCode::PermissionDenied => LevelDBErrorCode::PermissionDenied,
            StatusCode::Unknown => LevelDBErrorCode::Unknown,
            StatusCode::Errno(_) => LevelDBErrorCode::IOError,
        }
    }
}

use crate::VecU8;

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct LevelDBError {
    pub message: VecU8,
    pub code: LevelDBErrorCode,
}

impl From<&str> for LevelDBError {
    fn from(msg: &str) -> Self {
        Self { message: VecU8::from(msg), code: LevelDBErrorCode::Unknown }
    }
}

impl From<String> for LevelDBError {
    fn from(msg: String) -> Self {
        Self { message: VecU8::from(msg), code: LevelDBErrorCode::Unknown }
    }
}

impl From<rusty_leveldb::Status> for LevelDBError {
    fn from(status: rusty_leveldb::Status) -> Self {
        Self { message: VecU8::from(status.err), code: LevelDBErrorCode::from(status.code) }
    }
}
