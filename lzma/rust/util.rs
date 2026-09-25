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

/// Gets the combined memory usage limit of the state components.
///
/// Implements `lzma_memusage` by interrogating the rust state struct.
/// Returns memory usage for encoders and raw decoders where options are known.
/// For streaming decoders (XZ, Alone, Lzip, Auto), returns 0 because
/// the underlying decoders do not expose dynamically parsed dictionary size.
///
/// # Safety
/// - `strm` must be null or point to a valid, properly aligned `lzma_stream`.
/// - If `strm` is non-null and `strm.internal` is non-null, it must point to a valid
///   `InternalState` allocated by this crate.
/// - The caller must guarantee that `*strm` and `*strm.internal` are not concurrently modified
///   or deallocated during this call.
#[unsafe(export_name = crate::prefix!(lzma_memusage))]
pub unsafe extern "C" fn lzma_memusage(strm: Option<&crate::types::lzma_stream>) -> u64 {
    let Some(strm) = strm else {
        return 0;
    };
    if strm.internal.is_null() {
        return 0;
    }
    // SAFETY: `internal` is null-checked above, points to a valid `InternalState` allocated by
    // this crate per safety precondition, is properly aligned, and is not concurrently mutated
    // or freed.
    let state = unsafe { &*(strm.internal as *const crate::state::InternalState) };
    state.memusage()
}
