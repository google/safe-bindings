//! Utility functions: lzma_cputhreads, lzma_version_string.

use std::ffi::{c_char, CStr};
use std::thread;

/// Return a string describing the liblzma version.
/// This matches the version of lzma-rust2 but disguised under liblzma ABI.
#[unsafe(export_name = crate::prefix!(lzma_version_string))]
pub extern "C" fn lzma_version_string() -> *const c_char {
    const VERSION: &CStr =
        match CStr::from_bytes_with_nul(concat!(env!("LZMA_VERSION"), "\0").as_bytes()) {
            Ok(cstr) => cstr,
            Err(_) => panic!("Invalid LZMA_VERSION"),
        };
    VERSION.as_ptr()
}

/// Return the number of available CPU threads.
///
/// Returns 0 if the count cannot be determined (matching liblzma behavior).
#[unsafe(export_name = crate::prefix!(lzma_cputhreads))]
pub extern "C" fn lzma_cputhreads() -> u32 {
    thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(0)
}
