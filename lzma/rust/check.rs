//! Check/CRC functions: lzma_check_is_supported, lzma_crc32, lzma_crc64, lzma_get_check.

use std::slice;

use crc::{Crc, CRC_32_ISO_HDLC, CRC_64_XZ};
use lzma_rust2::CheckType;

use crate::state::{CoderInner, InternalState};
use crate::types::{
    lzma_check, lzma_stream, LZMA_CHECK_CRC32, LZMA_CHECK_CRC64, LZMA_CHECK_NONE, LZMA_CHECK_SHA256,
};

/// Returns 1 if the given check type is supported, 0 otherwise.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_check_is_supported))]`.
// - Required contract: The exported symbol name (`lzma_check_is_supported` or
//   `R_lzma_check_is_supported` depending on `LZMA_SYS_PREFIX`) must not collide with any other
//   symbol in the final linked artifact, and the `extern "C"` function signature must match the C
//   declaration `lzma_bool lzma_check_is_supported(lzma_check check)` in `lzma_rust_api.h`.
// - Evidence: Build target configuration (`LZMA_SYS_PREFIX` and `select()` rules in `BUILD`)
//   ensures symbol uniqueness when linked alongside or in place of C `liblzma`, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_check_is_supported))]
pub extern "C" fn lzma_check_is_supported(check: u32) -> u8 {
    match check {
        LZMA_CHECK_NONE | LZMA_CHECK_CRC32 | LZMA_CHECK_CRC64 | LZMA_CHECK_SHA256 => 1,
        _ => 0,
    }
}

/// Calculate CRC-32 with the polynomial used by liblzma/gzip/zlib.
///
/// This is compatible with liblzma's `lzma_crc32(buf, size, crc)`:
/// pass 0 as `crc` for the first call, then feed the return value back
/// as `crc` for incremental computation.
///
/// # Safety
///
/// The caller must ensure that:
/// - If `size > 0`:
///   1. `buf` is non-null, properly aligned for `u8` (alignment 1), and points to `size`
///      consecutive initialized `u8` values within a single live allocated object.
///   2. `size <= isize::MAX as usize`.
///   3. The byte range `[buf, buf + size)` is valid for reads and is not mutated by any thread,
///      alias, or reentrant code for the duration of this call.
/// - If `size == 0`, `buf` may be null or dangling and is not dereferenced.
///
/// If these preconditions hold, this function reads `size` bytes from `buf` without mutating
/// or retaining the pointer and returns the updated CRC-32 value.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_crc32))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `uint32_t lzma_crc32(const
//   uint8_t* buf, size_t size, uint32_t crc)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_crc32))]
pub unsafe extern "C" fn lzma_crc32(buf: *const u8, size: usize, crc: u32) -> u32 {
    if size == 0 {
        return crc;
    }
    debug_assert!(
        size <= isize::MAX as usize,
        "lzma_crc32: size must not be larger than isize::MAX"
    );
    debug_assert!(!buf.is_null(), "lzma_crc32: null buf with non-zero size");
    // SAFETY:
    // - Operation: `slice::from_raw_parts(buf, size)`.
    // - Required contract: `buf` must be non-null, aligned for `u8`, and point to `size`
    //   consecutive initialized `u8` values within a single allocated object of total size `<=
    //   isize::MAX as usize`, and the memory must not be mutated for the lifetime of `data`.
    // - Evidence:
    //   - `size > 0` holds after the early return `if size == 0` above, and no intervening code
    //     modifies `buf` or `size`.
    //   - Alignment for `u8` is 1 by the Rust Reference layout rules, which is satisfied by every
    //     address.
    //   - By this function's `# Safety` precondition, when `size > 0`, `buf` is non-null, points to
    //     `size` consecutive initialized readable bytes in a single live allocation with `size <=
    //     isize::MAX as usize`, and is not mutated during this call.
    // - Postcondition: `data` is a shared slice `&[u8]` of length `size` borrowed only within this
    //   function and not retained after return.
    let data = unsafe { slice::from_raw_parts(buf, size) };
    let crc_alg = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let mut digest = crc_alg.digest_with_initial((crc ^ 0xFFFF_FFFF).reverse_bits());
    digest.update(data);
    digest.finalize()
}

/// Calculate CRC-64 with the polynomial used by liblzma/XZ.
///
/// This is compatible with liblzma's `lzma_crc64(buf, size, crc)`:
/// pass 0 as `crc` for the first call, then feed the return value back
/// as `crc` for incremental computation.
///
/// # Safety
///
/// The caller must ensure that:
/// - If `size > 0`:
///   1. `buf` is non-null, properly aligned for `u8` (alignment 1), and points to `size`
///      consecutive initialized `u8` values within a single live allocated object.
///   2. `size <= isize::MAX as usize`.
///   3. The byte range `[buf, buf + size)` is valid for reads and is not mutated by any thread,
///      alias, or reentrant code for the duration of this call.
/// - If `size == 0`, `buf` may be null or dangling and is not dereferenced.
///
/// If these preconditions hold, this function reads `size` bytes from `buf` without mutating
/// or retaining the pointer and returns the updated CRC-64 value.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_crc64))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `uint64_t lzma_crc64(const
//   uint8_t* buf, size_t size, uint64_t crc)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_crc64))]
pub unsafe extern "C" fn lzma_crc64(buf: *const u8, size: usize, crc: u64) -> u64 {
    if size == 0 {
        return crc;
    }
    debug_assert!(
        size <= isize::MAX as usize,
        "lzma_crc64: size must not be larger than isize::MAX"
    );
    debug_assert!(!buf.is_null(), "lzma_crc64: null buf with non-zero size");
    // SAFETY:
    // - Operation: `slice::from_raw_parts(buf, size)`.
    // - Required contract: `buf` must be non-null, aligned for `u8`, and point to `size`
    //   consecutive initialized `u8` values within a single allocated object of total size `<=
    //   isize::MAX as usize`, and the memory must not be mutated for the lifetime of `data`.
    // - Evidence:
    //   - `size > 0` holds after the early return `if size == 0` above, and no intervening code
    //     modifies `buf` or `size`.
    //   - Alignment for `u8` is 1 by the Rust Reference layout rules, which is satisfied by every
    //     address.
    //   - By this function's `# Safety` precondition, when `size > 0`, `buf` is non-null, points to
    //     `size` consecutive initialized readable bytes in a single live allocation with `size <=
    //     isize::MAX as usize`, and is not mutated during this call.
    // - Postcondition: `data` is a shared slice `&[u8]` of length `size` borrowed only within this
    //   function and not retained after return.
    let data = unsafe { slice::from_raw_parts(buf, size) };
    let crc_alg = Crc::<u64>::new(&CRC_64_XZ);
    let mut digest = crc_alg.digest_with_initial((crc ^ 0xFFFFFFFF_FFFFFFFF).reverse_bits());
    digest.update(data);
    digest.finalize()
}

/// Returns the check type recorded by `state`'s decoder.
///
/// This is the shared implementation behind [`lzma_get_check`]. Internal callers must use
/// this instead of re-entering through the FFI entry point: they already hold a reference to
/// the `InternalState`, and going back through the raw `lzma_stream` pointer would create a
/// second, aliasing reference to it.
pub(crate) fn check_of(state: &InternalState) -> lzma_check {
    // The auto decoder answers for the format it detected.
    let coder = match &state.coder {
        CoderInner::AutoDecoder(auto) => auto.inner().unwrap_or(&state.coder),
        coder => coder,
    };
    if let CoderInner::XzDecoder(xz) = coder {
        match xz.check_type() {
            Some(CheckType::None) | None => LZMA_CHECK_NONE,
            Some(CheckType::Crc32) => LZMA_CHECK_CRC32,
            Some(CheckType::Crc64) => LZMA_CHECK_CRC64,
            Some(CheckType::Sha256) => LZMA_CHECK_SHA256,
        }
    } else if matches!(coder, CoderInner::LzipDecoder(_)) {
        // The .lz format always uses CRC32.
        LZMA_CHECK_CRC32
    } else {
        // Raw decoders, lzma alone, etc., don't have a check type natively stored per stream.
        // Encoders are not expected to be queried for check type.
        LZMA_CHECK_NONE
    }
}

/// Get the check type of the current XZ stream.
///
/// Returns the `lzma_check` value for the stream being decoded.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to an initialized, live
///    `lzma_stream` allocation valid for reads for the duration of this call.
/// 2. If `strm` is non-null, `(*strm).internal` is either null or a live pointer previously
///    allocated by this crate via `Box::into_raw(Box::new(InternalState))` (in
///    `helpers::install_state`) that has not yet been freed by `lzma_end` or replaced by a
///    re-initialization call.
/// 3. For the duration of this call, neither `*strm` nor `*(*strm).internal` is mutably aliased or
///    concurrently modified by any thread or access path.
///
/// If these preconditions hold, this function reads the stream's check type without mutating
/// `*strm` or `*(*strm).internal`.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_get_check))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_check
//   lzma_get_check(const lzma_stream* strm)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_get_check))]
pub unsafe extern "C" fn lzma_get_check(strm: *const lzma_stream) -> lzma_check {
    if strm.is_null() {
        return LZMA_CHECK_NONE;
    }
    // SAFETY:
    // - Operation: Place projection and read of `(*strm).internal`.
    // - Required contract: `strm` must be non-null, properly aligned for `lzma_stream`, and point
    //   to an initialized `lzma_stream` valid for reads without concurrent mutation.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and by
    //   preconditions 1 and 3 of this function's `# Safety` contract, `strm` is properly aligned,
    //   points to an initialized live `lzma_stream`, and is not concurrently mutated during this
    //   call.
    // - Postcondition: Copies the raw pointer value `(*strm).internal` without taking ownership.
    let internal = unsafe { (*strm).internal } as *const InternalState;
    if internal.is_null() {
        return LZMA_CHECK_NONE;
    }
    // SAFETY:
    // - Operation: Creating a shared reference `&*internal` of type `&InternalState`.
    // - Required contract: `internal` must be non-null, properly aligned for `InternalState`, point
    //   to a live initialized `InternalState`, and obey shared-reference aliasing rules for the
    //   lifetime of `state`.
    // - Evidence: `internal` is non-null after the `if internal.is_null()` check above. By
    //   preconditions 2 and 3 of this function's `# Safety` contract, a non-null `(*strm).internal`
    //   was created by `Box::into_raw(Box::new(InternalState))` in this crate, has not been freed,
    //   and is not mutably aliased or concurrently modified for the duration of this call.
    // - Postcondition: `state` is a valid shared reference `&InternalState` confined to this call.
    let state = unsafe { &*internal };
    check_of(state)
}
