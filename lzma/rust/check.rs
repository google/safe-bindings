//! Check/CRC functions: lzma_check_is_supported, lzma_crc32, lzma_crc64, lzma_get_check.

use std::slice;

use crc::{Crc, CRC_32_ISO_HDLC, CRC_64_XZ};
use lzma_rust2::CheckType;

use crate::state::{CoderInner, InternalState};
use crate::types::{
    lzma_check, lzma_stream, LZMA_CHECK_CRC32, LZMA_CHECK_CRC64, LZMA_CHECK_NONE, LZMA_CHECK_SHA256,
};

/// Returns 1 if the given check type is supported, 0 otherwise.
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
/// `buf` must point to `size` readable bytes, or be NULL when `size` is 0.
/// `size` must be less than or equal to `isize::MAX`.
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
    // SAFETY: `buf` is a pointer to a readable buffer of `size` bytes, according to safety contract
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
/// `buf` must point to `size` readable bytes, or be NULL when `size` is 0.
/// `size` must be less than or equal to `isize::MAX`.
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
    // SAFETY: `buf` is a pointer to a readable buffer of `size` bytes, according to safety contract
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
/// `strm` must be null or point to a valid `lzma_stream`. If `strm.internal` is non-null,
/// it must point to a valid, initialized `InternalState` allocated by this crate and not
/// yet freed. The caller must ensure that nothing else mutates `strm` or `strm.internal`
/// for the duration of this call.
#[unsafe(export_name = crate::prefix!(lzma_get_check))]
pub unsafe extern "C" fn lzma_get_check(strm: *const lzma_stream) -> lzma_check {
    if strm.is_null() {
        return LZMA_CHECK_NONE;
    }
    // SAFETY: `strm` is null-checked above, and caller promises it is valid.
    let internal = unsafe { (*strm).internal } as *const InternalState;
    if internal.is_null() {
        return LZMA_CHECK_NONE;
    }
    // SAFETY: `internal` is null-checked above, and the caller guarantees it points to an
    // `InternalState` allocated by this crate that is not concurrently mutated.
    let state = unsafe { &*internal };
    check_of(state)
}
