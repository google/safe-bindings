//! C FFI wrapper around `lzma-rust2` providing a liblzma-compatible API.
//!
//! This crate exposes `extern "C"` functions that are ABI-compatible with
//! liblzma for XZ, LZMA2, and LZMA alone encoding and decoding.
//!
//! # Safety
//! This crate implements FFI wrappers for the C liblzma API. It inherently
//! requires extensive use of `unsafe` code to dereference raw pointers passed
//! from C, execute pointer arithmetic, and manage raw FFI boundaries.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
// NOTE: b/515650349 - Remove this once the wrapper is fully implemented.
#![allow(dead_code)]

mod check;
mod code;
mod decode;
mod encode;
mod filter;
mod helpers;
mod state;
mod types;
mod util;

macro_rules! prefix {
    ($name:ident) => {
        concat!(env!("LZMA_SYS_PREFIX"), stringify!($name))
    };
}
pub(crate) use prefix;

pub use check::{lzma_check_is_supported, lzma_crc32, lzma_crc64, lzma_get_check};
pub use code::{lzma_code, lzma_end};
#[cfg(feature = "lzip")]
pub use decode::lzma_lzip_decoder;
pub use decode::{lzma_alone_decoder, lzma_auto_decoder, lzma_raw_decoder, lzma_stream_decoder};
#[cfg(feature = "lzip")]
pub use encode::lzma_lzip_encoder;
pub use encode::{
    lzma_alone_encoder, lzma_easy_encoder, lzma_raw_encoder, lzma_stream_encoder,
    lzma_stream_encoder_mt,
};
pub use filter::{
    lzma_lzma_preset, lzma_properties_decode, lzma_properties_encode, lzma_properties_size,
};
pub use types::{
    lzma_action, lzma_allocator, lzma_bool, lzma_check, lzma_filter, lzma_internal,
    lzma_internal_s, lzma_mt, lzma_options_bcj, lzma_options_delta, lzma_options_lzma,
    lzma_reserved_enum, lzma_ret, lzma_stream,
};
pub use util::{lzma_cputhreads, lzma_memusage, lzma_version_string};
