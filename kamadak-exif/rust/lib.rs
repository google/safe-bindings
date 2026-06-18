//! This crate provides Crubit wrapper for the Rust crate "kamadak-exif".

mod crubit_util;
mod crubit_vec_util;

// Reexport some original struct that does not need bridging.
pub use exif::Context;

pub mod error;
pub mod mnote;
pub mod reader;
pub mod reexport;
pub mod tag;
pub mod tiff;
pub mod types;
pub mod value;
pub mod writer;
