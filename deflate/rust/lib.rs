//! This crate provides C++ bindings for the flate2 crate.
//! This crate defines types and methods that expose the flate2 API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use flate2 directly.
#![feature(string_from_utf8_lossy_owned)]

mod crubit_util;
mod crubit_vec_util;
mod gz;

use rust_vec_u8::VecU8;

// Re-export original types from flate2 without wrapper.
pub use flate2::Compression;
pub use flate2::GzHeader;

// Utility types
make_result_type!(usize, ResultUsize);
make_result_type!(VecU8, ResultVecU8);
make_result_type!((), ResultUnit);

// Types wrapped by this crate.
pub use crate::gz::OptionGzHeader;

pub mod read {
    pub use crate::gz::read::GzDecoder;
    pub use crate::gz::read::GzEncoder;
    pub use crate::gz::read::MultiGzDecoder;
}

pub mod write {
    pub use crate::gz::write::GzDecoder;
    pub use crate::gz::write::GzEncoder;
    pub use crate::gz::write::MultiGzDecoder;
}
