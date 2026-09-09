//! C++ bindings for the jxl-rs JPEG XL decoder crate.
//!
//! This crate provides Crubit-compatible types that wrap the jxl crate's decoder API,
//! enabling C++ code to decode JPEG XL images through auto-generated Crubit bindings.
//!
//! WARNING: This crate should never be used from Rust. Use the jxl crate directly instead.

pub mod decoder;
pub mod types;
