//! This crate provides C++ bindings for the image crate.
//! This crate defines types and methods that expose the image API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use image directly.

mod crubit_util;
pub mod image;
pub mod reader;
pub mod vec_u8;

make_option_type!(u8, OptionU8);
