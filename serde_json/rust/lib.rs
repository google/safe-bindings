//! This crate provides C++ bindings for the json_serde crate.
//! This crate defines types and methods that expose the image API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use image directly.

mod crubit_util;
mod crubit_vec_util;

pub mod raw_string;

pub mod json;
