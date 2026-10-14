//! Utility functions: lzma_cputhreads, lzma_version_string.

use std::ffi::{c_char, CStr};
use std::thread;

/// Return a string describing the liblzma version.
/// This matches the version of lzma-rust2 but disguised under liblzma ABI.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_version_string))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `const char*
//   lzma_version_string(void)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_cputhreads))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `uint32_t
//   lzma_cputhreads(void)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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
///
/// The caller must ensure that:
/// 1. `strm` is either `None` (null in C) or a valid shared reference to an initialized, live
///    `lzma_stream` not mutably aliased or modified for the duration of this call.
/// 2. If `strm` is `Some(strm_ref)` and `strm_ref.internal` is non-null, `strm_ref.internal` points
///    to a live, initialized `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, and `*strm_ref.internal` is not
///    mutably aliased or concurrently modified for the duration of this call.
///
/// If these preconditions hold, this function reads the memory usage from `*strm_ref.internal`
/// without mutating or deallocating the stream state.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_memusage))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `uint64_t
//   lzma_memusage(const lzma_stream* strm)` in `lzma_rust_api.h` (`Option<&T>` is guaranteed
//   ABI-compatible with nullable `const T*` by Rust's null-pointer optimization).
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_memusage))]
pub unsafe extern "C" fn lzma_memusage(strm: Option<&crate::types::lzma_stream>) -> u64 {
    let Some(strm) = strm else {
        return 0;
    };
    if strm.internal.is_null() {
        return 0;
    }
    // SAFETY:
    // - Operation: Creating a shared reference `&*(strm.internal as *const InternalState)`.
    // - Required contract: `strm.internal` must be non-null, properly aligned for `InternalState`,
    //   point to a live initialized `InternalState`, and obey shared-reference aliasing rules for
    //   the lifetime of `state`.
    // - Evidence: `strm.internal` is non-null after the `if strm.internal.is_null()` check above.
    //   By preconditions 1 and 2 of this function's `# Safety` contract, a non-null `strm.internal`
    //   was allocated by this crate via `Box::into_raw(Box::new(InternalState))`, has not been
    //   freed, and is not mutably aliased or concurrently modified during this call.
    // - Postcondition: `state` is a valid shared reference `&InternalState` confined to this call.
    let state = unsafe { &*(strm.internal as *const crate::state::InternalState) };
    state.memusage()
}
