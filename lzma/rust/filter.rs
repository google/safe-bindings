//! Filter property functions: lzma_lzma_preset, lzma_properties_size,
//! lzma_properties_encode, lzma_properties_decode.

use lzma_rust2::{EncodeMode, LzmaOptions, MfType};
use std::ffi::c_void;

use crate::helpers::{mf_to_c, mode_to_c};
use crate::types::{
    self, lzma_bool, lzma_filter, lzma_options_delta, lzma_options_lzma, lzma_ret,
    LZMA_DELTA_TYPE_BYTE, LZMA_MEM_ERROR, LZMA_MF_BT4, LZMA_MODE_NORMAL, LZMA_OK,
    LZMA_OPTIONS_ERROR, LZMA_PROG_ERROR,
};

const PRESET_EXTREME: u32 = 0x80000000;

const FILTER_LZMA1: u64 = types::LZMA_FILTER_LZMA1 as u64;
const FILTER_LZMA2: u64 = types::LZMA_FILTER_LZMA2 as u64;
const FILTER_X86: u64 = types::LZMA_FILTER_X86 as u64;
const FILTER_POWERPC: u64 = types::LZMA_FILTER_POWERPC as u64;
const FILTER_IA64: u64 = types::LZMA_FILTER_IA64 as u64;
const FILTER_ARM: u64 = types::LZMA_FILTER_ARM as u64;
const FILTER_ARMTHUMB: u64 = types::LZMA_FILTER_ARMTHUMB as u64;
const FILTER_SPARC: u64 = types::LZMA_FILTER_SPARC as u64;
const FILTER_ARM64: u64 = types::LZMA_FILTER_ARM64 as u64;
const FILTER_RISCV: u64 = types::LZMA_FILTER_RISCV as u64;
const FILTER_DELTA: u64 = types::LZMA_FILTER_DELTA as u64;
const VLI_MAX: u64 = types::LZMA_VLI_MAX as u64;
const LZMA_DICT_SIZE_MIN: u32 = 4096;

/// The number of possible values for the LZMA1 `lc` (literal context) property.
/// Valid values are 0 through 8.
const LZMA1_NUM_LC_VALUES: u32 = 9;

/// The number of possible values for the LZMA1 `lp` (literal position) property.
/// Valid values are 0 through 4.
const LZMA1_NUM_LP_VALUES: u32 = 5;

/// The maximum allowed value for an LZMA2 property byte, mapping to the
/// maximum dictionary size (UINT32_MAX).
const LZMA2_PROP_MAX: u8 = 40;

// NOTE(b/552362497): Change the raw pointers into references once MaybeUninit is supported in
// safer_cffi.
/// Fill `options` with values from `preset`.
///
/// Returns 0 (false) on success, 1 (true) on failure (matching liblzma's
/// inverted boolean convention).
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `options` is either null or a non-null, properly aligned pointer to a writable
///    `lzma_options_lzma` allocation (`size_of::<lzma_options_lzma>()` bytes).
/// 2. If `options` is non-null, no other pointer or reference reads or writes `*options` for the
///    duration of this call.
///
/// If these preconditions hold and this function returns `0`, `*options` is initialized with
/// the requested preset configuration.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_lzma_preset))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_bool
//   lzma_lzma_preset(lzma_options_lzma* options, uint32_t preset)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_lzma_preset))]
pub unsafe extern "C" fn lzma_lzma_preset(
    options: *mut lzma_options_lzma,
    preset: u32,
) -> lzma_bool {
    if options.is_null() {
        return 1;
    }

    // The LZMA preset in liblzma is passed as a bitmask where the base level (0-9)
    // is OR'd with modifier flags (e.g., PRESET_EXTREME).
    // The lzma-rust2 library's `LzmaOptions::with_preset()` expects just the numeric
    // base level, so we mask out the EXTREME flag to isolate the level constraint.
    let level = preset & !PRESET_EXTREME;
    if level > 9 {
        return 1;
    }

    let mut rust_opts = LzmaOptions::with_preset(level);
    // The extreme preset requires special handling in Rust since the underlying
    // lzma-rust2 library applies these parameters explicitly rather than having
    // an extreme preset API endpoint.
    if preset & PRESET_EXTREME != 0 {
        rust_opts.mode = EncodeMode::Normal;
        rust_opts.mf = MfType::Bt4;
        rust_opts.nice_len = LzmaOptions::NICE_LEN_MAX;
        rust_opts.depth_limit = 0;
    }

    let opts = lzma_options_lzma {
        dict_size: rust_opts.dict_size,
        preset_dict: core::ptr::null(),
        preset_dict_size: 0,
        lc: rust_opts.lc,
        lp: rust_opts.lp,
        pb: rust_opts.pb,
        mode: mode_to_c(rust_opts.mode),
        nice_len: rust_opts.nice_len,
        mf: mf_to_c(rust_opts.mf),
        depth: rust_opts.depth_limit as u32,
        // SAFETY:
        // - Operation: `core::mem::zeroed::<lzma_options_lzma>()`.
        // - Required contract: The all-zero byte pattern of length `size_of::<lzma_options_lzma>()`
        //   must represent a valid value of type `lzma_options_lzma`.
        // - Evidence: `lzma_options_lzma` is a bindgen-generated `#[repr(C)]` struct whose fields
        //   are exclusively primitive unsigned integers (`u32`, C enum typedefs represented as
        //   `u32`) and raw pointers (`*const u8`, `*mut c_void`). All-zero bit patterns are valid
        //   for all integer and raw-pointer types under the Rust Reference validity rules.
        // - Postcondition: Produces a valid zero-initialized `lzma_options_lzma` value for struct
        //   update syntax.
        ..unsafe { core::mem::zeroed() }
    };

    // SAFETY:
    // - Operation: `core::ptr::write(options, opts)`.
    // - Required contract: `options` must be non-null, properly aligned for `lzma_options_lzma`,
    //   and valid for writes of `size_of::<lzma_options_lzma>()` bytes without aliasing violation.
    // - Evidence: `options` is non-null after the `if options.is_null()` check above, and by
    //   preconditions 1 and 2 of this function's `# Safety` contract, `options` is properly
    //   aligned, points to a live writable `lzma_options_lzma` allocation, and is exclusively
    //   accessed during this call.
    // - Postcondition: `*options` is initialized with `opts` without reading or dropping any prior
    //   contents (`lzma_options_lzma` is `Copy` and has no drop glue).
    unsafe {
        core::ptr::write(options, opts);
    }

    0 // success
}

/// Get the size of the encoded properties for a filter.
///
/// For LZMA1/LZMA2, the properties are always 5 bytes (or 1 byte for LZMA2).
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_properties_size))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_properties_size(uint32_t* size, const lzma_filter* filter)` in `lzma_rust_api.h`
//   (`Option<&mut u32>` and `Option<&lzma_filter>` are ABI-compatible with nullable pointers by
//   Rust's null-pointer optimization).
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_properties_size))]
pub extern "C" fn lzma_properties_size(
    size: Option<&mut u32>,
    filter: Option<&lzma_filter>,
) -> lzma_ret {
    let (Some(size), Some(f)) = (size, filter) else {
        return LZMA_PROG_ERROR;
    };
    let props_size = match f.id {
        FILTER_LZMA1 => 5,
        FILTER_LZMA2 => 1,
        FILTER_DELTA => 1,
        FILTER_X86 | FILTER_POWERPC | FILTER_IA64 | FILTER_ARM | FILTER_ARMTHUMB | FILTER_SPARC
        | FILTER_ARM64 | FILTER_RISCV => 0,
        ..=VLI_MAX => {
            // Following liblzma's behavior, if the filter ID is not a valid VLI,
            // we return `LZMA_PROG_ERROR`. Otherwise, we return `LZMA_OPTIONS_ERROR`.
            return LZMA_OPTIONS_ERROR;
        }
        _ => return LZMA_PROG_ERROR,
    };
    *size = props_size;
    LZMA_OK
}

/// Encode filter properties into `props`.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `filter` is either `None` (null in C) or a valid shared reference to an initialized
///    `lzma_filter` not mutated for the duration of this call.
/// 2. If `filter` is `Some(f)` and the encoded properties size `props_size` for `f.id` is non-zero
///    (`5` bytes for `LZMA_FILTER_LZMA1`, `1` byte for `LZMA_FILTER_LZMA2` or `LZMA_FILTER_DELTA`),
///    `props` is either null (which returns `LZMA_PROG_ERROR`) or a non-null, properly aligned
///    pointer to at least `props_size` writable bytes within a single live allocated object that
///    does not overlap `f` or `*f.options` and is not read or written through any other pointer or
///    reference for the duration of this call.
/// 3. If `filter` is `Some(f)` and `f.options` is non-null:
///    - When `f.id == u64::from(LZMA_FILTER_LZMA1)` or `f.id == u64::from(LZMA_FILTER_LZMA2)`,
///      `f.options` is properly aligned for `lzma_options_lzma` and points to a live, initialized
///      `lzma_options_lzma` valid for reads and not mutated for the duration of this call.
///    - When `f.id == u64::from(LZMA_FILTER_LZMA1)`, if `(*(f.options as *const
///      lzma_options_lzma)).preset_dict` is non-null and `preset_dict_size > 0`, `preset_dict`
///      points to at least `preset_dict_size as usize` consecutive initialized readable `u8` bytes
///      within a single live allocated object (`<= isize::MAX as usize`) not mutated for the
///      duration of this call.
///    - When `f.id == u64::from(LZMA_FILTER_DELTA)`, `f.options` is properly aligned for
///      `lzma_options_delta` and points to a live, initialized `lzma_options_delta` valid for reads
///      and not mutated for the duration of this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`, the encoded filter properties
/// are written into the first `props_size` bytes of `props`.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_properties_encode))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_properties_encode(const lzma_filter* filter, uint8_t* props)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_properties_encode))]
pub unsafe extern "C" fn lzma_properties_encode(
    filter: Option<&lzma_filter>,
    props: *mut u8,
) -> lzma_ret {
    let Some(f) = filter else {
        return LZMA_PROG_ERROR;
    };

    let props_size = match f.id {
        FILTER_LZMA1 => 5,
        FILTER_LZMA2 => 1,
        FILTER_DELTA => 1,
        _ => 0,
    };

    if props_size > 0 && props.is_null() {
        return LZMA_PROG_ERROR;
    }

    let props_slice = if props_size > 0 {
        // SAFETY:
        // - Operation: `core::ptr::write_bytes(props, 0, props_size)`.
        // - Required contract: `props` must be non-null, aligned for `u8` (alignment 1), and valid
        //   for writes of `props_size` bytes within a single live allocation without aliasing.
        // - Evidence: `props_size > 0` and `!props.is_null()` hold after the check above, alignment
        //   for `u8` is 1 by the Rust Reference layout rules, and precondition 2 of this function's
        //   `# Safety` contract guarantees `props` points to at least `props_size` writable
        //   unaliased bytes in a single live allocation.
        // - Postcondition: The `props_size` bytes starting at `props` are initialized to `0u8`.
        unsafe {
            core::ptr::write_bytes(props, 0, props_size);
        }
        // SAFETY:
        // - Operation: `core::slice::from_raw_parts_mut(props, props_size)`.
        // - Required contract: `props` must be non-null, aligned for `u8`, and point to
        //   `props_size` consecutive initialized `u8` values within a single allocated object of
        //   size `<= isize::MAX as usize` that is exclusively accessed for the slice's lifetime.
        // - Evidence: `!props.is_null()` and `props_size <= 5 <= isize::MAX as usize` hold, the
        //   preceding `write_bytes` call initialized all `props_size` bytes, and precondition 2 of
        //   this function's `# Safety` contract guarantees single-allocation bounds and exclusive
        //   access during this call.
        // - Postcondition: `props_slice` is a valid exclusive slice `&mut [u8]` of length
        //   `props_size`.
        unsafe { core::slice::from_raw_parts_mut(props, props_size) }
    } else {
        &mut []
    };

    // SAFETY:
    // - Operation: `lzma_properties_encode_inner(f, props_slice)`.
    // - Required contract: When `f.options` is non-null, it must satisfy the alignment,
    //   initialization, `preset_dict`, and non-aliasing requirements for `f.id`.
    // - Evidence: Guaranteed directly by preconditions 2 and 3 of `lzma_properties_encode`'s `#
    //   Safety` contract, with no intervening mutation of `f`.
    // - Postcondition: Encodes the filter properties into `props_slice` and returns the status
    //   code.
    unsafe { lzma_properties_encode_inner(f, props_slice) }
}

/// Encodes filter properties for `f` into `props_slice`.
///
/// # Panics
///
/// Panics if `props_slice` has length less than `5` when `f.id == FILTER_LZMA1` or less than `1`
/// when `f.id == FILTER_LZMA2` or `f.id == FILTER_DELTA`.
///
/// # Safety
///
/// If `f.options` is non-null, the caller must ensure that:
/// 1. When `f.id == FILTER_LZMA1` or `f.id == FILTER_LZMA2`, `f.options` is properly aligned for
///    `lzma_options_lzma` and points to a live, initialized `lzma_options_lzma` valid for reads and
///    not mutated or aliased by `props_slice` for the duration of this call.
/// 2. When `f.id == FILTER_LZMA1`, if `(*(f.options as *const lzma_options_lzma)).preset_dict` is
///    non-null and `preset_dict_size > 0`, `preset_dict` points to at least `preset_dict_size as
///    usize` consecutive initialized readable `u8` bytes within a single live allocated object (`<=
///    isize::MAX as usize`) not mutated for the duration of this call.
/// 3. When `f.id == FILTER_DELTA`, `f.options` is properly aligned for `lzma_options_delta` and
///    points to a live, initialized `lzma_options_delta` valid for reads and not mutated or aliased
///    by `props_slice` for the duration of this call.
unsafe fn lzma_properties_encode_inner(f: &lzma_filter, props_slice: &mut [u8]) -> lzma_ret {
    match f.id {
        FILTER_LZMA1 => {
            if f.options.is_null() {
                return LZMA_PROG_ERROR;
            }
            // SAFETY:
            // - Operation: Creating a shared reference `&*f.options.cast::<lzma_options_lzma>()`.
            // - Required contract: `f.options` must be non-null, properly aligned for
            //   `lzma_options_lzma`, and point to a live, initialized `lzma_options_lzma` not
            //   mutated or mutably aliased for the lifetime of `opts`.
            // - Evidence: `f.id == FILTER_LZMA1` and `!f.options.is_null()` hold after the check
            //   above, and precondition 1 of this function's `# Safety` contract guarantees
            //   alignment, initialization, and non-aliasing.
            // - Postcondition: `opts` is a valid shared reference `&lzma_options_lzma` confined to
            //   this match arm.
            let opts = unsafe { &*f.options.cast::<lzma_options_lzma>() };
            // SAFETY:
            // - Operation: `crate::state::lzma_options_from_c(opts)`.
            // - Required contract: When `opts.preset_dict` is non-null and `opts.preset_dict_size >
            //   0`, `opts.preset_dict` must point to at least `opts.preset_dict_size as usize`
            //   initialized readable bytes in a single live allocation (`<= isize::MAX as usize`)
            //   not mutated during this call.
            // - Evidence: Guaranteed by precondition 2 of this function's `# Safety` contract, with
            //   no intervening mutation of `opts`.
            // - Postcondition: Returns an owned `Result<LzmaOptions, lzma_ret>` without retaining
            //   any raw pointers.
            let rust_opts_result = unsafe { crate::state::lzma_options_from_c(opts) };

            let rust_opts = match rust_opts_result {
                Ok(opts) => opts,
                Err(e) => return e,
            };

            let props_byte = rust_opts.get_props();
            props_slice[0] = props_byte;
            let dict_bytes = rust_opts.dict_size.to_le_bytes();
            props_slice[1..5].copy_from_slice(&dict_bytes);
            LZMA_OK
        }
        FILTER_LZMA2 => {
            if f.options.is_null() {
                return LZMA_PROG_ERROR;
            }
            // SAFETY:
            // - Operation: Creating a shared reference `&*f.options.cast::<lzma_options_lzma>()`.
            // - Required contract: `f.options` must be non-null, properly aligned for
            //   `lzma_options_lzma`, and point to a live, initialized `lzma_options_lzma` not
            //   mutated or mutably aliased for the lifetime of `opts`.
            // - Evidence: `f.id == FILTER_LZMA2` and `!f.options.is_null()` hold after the check
            //   above, and precondition 1 of this function's `# Safety` contract guarantees
            //   alignment, initialization, and non-aliasing.
            // - Postcondition: `opts` is a valid shared reference `&lzma_options_lzma` confined to
            //   this match arm.
            let opts = unsafe { &*f.options.cast::<lzma_options_lzma>() };
            // LZMA2 property byte encodes dict_size
            let prop = lzma2_dict_size_to_prop(opts.dict_size);
            props_slice[0] = prop;
            LZMA_OK
        }
        FILTER_DELTA => {
            if f.options.is_null() {
                return LZMA_PROG_ERROR;
            }
            // SAFETY:
            // - Operation: Creating a shared reference `&*f.options.cast::<lzma_options_delta>()`.
            // - Required contract: `f.options` must be non-null, properly aligned for
            //   `lzma_options_delta`, and point to a live, initialized `lzma_options_delta` not
            //   mutated or mutably aliased for the lifetime of `opts`.
            // - Evidence: `f.id == FILTER_DELTA` and `!f.options.is_null()` hold after the check
            //   above, and precondition 3 of this function's `# Safety` contract guarantees
            //   alignment, initialization, and non-aliasing.
            // - Postcondition: `opts` is a valid shared reference `&lzma_options_delta` confined to
            //   this match arm.
            let opts = unsafe { &*f.options.cast::<lzma_options_delta>() };
            if opts.type_ != LZMA_DELTA_TYPE_BYTE || opts.dist < 1 || opts.dist > 256 {
                return LZMA_PROG_ERROR;
            }
            // Delta distance - 1 stored as single byte
            let dist = opts.dist.saturating_sub(1) as u8;
            props_slice[0] = dist;
            LZMA_OK
        }
        FILTER_X86 | FILTER_POWERPC | FILTER_IA64 | FILTER_ARM | FILTER_ARMTHUMB | FILTER_SPARC
        | FILTER_ARM64 | FILTER_RISCV => {
            // BCJ filters have no properties
            LZMA_OK
        }
        _ => LZMA_PROG_ERROR,
    }
}

/// Decode filter properties from `props` into `filter->options`.
///
/// This allocates an `lzma_options_lzma` struct for LZMA1/LZMA2 filters.
/// The caller must free it using `free()`. Note that a custom allocator
/// is not supported, and `libc::malloc` is always used. The `allocator`
/// argument must be NULL.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `filter` is either `None` (null in C) or a valid exclusive reference to an initialized,
///    writable `lzma_filter` not accessed through any other pointer or reference for the duration
///    of this call.
/// 2. If `props_size > 0` and `!props.is_null()`, `props` is properly aligned for `u8` (alignment
///    1), points to `props_size` consecutive initialized `u8` bytes within a single live allocated
///    object with `props_size <= isize::MAX as usize`, does not overlap `filter`, and is not
///    mutated for the duration of this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`:
/// - For `LZMA_FILTER_LZMA1`, `LZMA_FILTER_LZMA2`, and `LZMA_FILTER_DELTA`, `filter.options` is set
///   to a newly heap-allocated, initialized options struct allocated via `libc::malloc` that the
///   caller now owns and must eventually release with `libc::free`.
/// - For supported BCJ filters with `props_size == 0`, `filter.options` is set to null.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_properties_decode))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_properties_decode(lzma_filter* filter, const void* allocator, const uint8_t* props, size_t
//   props_size)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_properties_decode))]
pub unsafe extern "C" fn lzma_properties_decode(
    filter: Option<&mut lzma_filter>,
    allocator: *const c_void,
    props: *const u8,
    props_size: usize,
) -> lzma_ret {
    let Some(f) = filter else {
        return LZMA_PROG_ERROR;
    };
    // Non-null allocator is currently not supported.
    if !allocator.is_null() {
        return LZMA_PROG_ERROR;
    }
    if props_size > 0 && props.is_null() {
        return LZMA_OPTIONS_ERROR;
    }
    let props = if props_size > 0 {
        // SAFETY:
        // - Operation: `core::slice::from_raw_parts(props, props_size)`.
        // - Required contract: `props` must be non-null, aligned for `u8`, and point to
        //   `props_size` consecutive initialized `u8` values within a single allocated object of
        //   size `<= isize::MAX as usize` that is not mutated for the lifetime of `props`.
        // - Evidence: `props_size > 0` and `!props.is_null()` are checked above, alignment for `u8`
        //   is 1 by the Rust Reference layout rules, and precondition 2 of this function's `#
        //   Safety` contract guarantees that `props` points to `props_size` initialized readable
        //   bytes in a single live allocation (`<= isize::MAX as usize`) disjoint from `f` and not
        //   mutated during this call.
        // - Postcondition: `props` is a valid shared slice `&[u8]` confined to this call.
        unsafe { core::slice::from_raw_parts(props, props_size) }
    } else {
        &[]
    };

    lzma_properties_decode_inner(f, props)
}

/// We must allocate the options structs via the C allocator rather than
/// `Box::new`, because the caller expects to free them with C's `free()`.
fn alloc_options<T>(options: T) -> Result<*mut c_void, lzma_ret> {
    const {
        assert!(
            core::mem::size_of::<T>() > 0
                && core::mem::align_of::<T>() <= core::mem::align_of::<libc::max_align_t>()
        );
    }
    // SAFETY:
    // - Operation: `libc::malloc(core::mem::size_of::<T>())`.
    // - Required contract: `libc::malloc` is an FFI call to the C standard library allocator; it
    //   has no pointer preconditions and returns either a null pointer on allocation failure or a
    //   live pointer suitably aligned for any fundamental type up to `libc::max_align_t` with at
    //   least `size_of::<T>()` allocated bytes.
    // - Evidence: The compile-time assertion above guarantees `size_of::<T>() > 0` and
    //   `align_of::<T>() <= align_of::<libc::max_align_t>()`.
    // - Postcondition: `ptr` is either null or a non-null, `T`-aligned pointer to a fresh,
    //   unaliased heap allocation of `size_of::<T>()` uninitialized bytes.
    let ptr = unsafe { libc::malloc(core::mem::size_of::<T>()) }.cast::<T>();
    if ptr.is_null() {
        return Err(LZMA_MEM_ERROR);
    }
    // SAFETY:
    // - Operation: `ptr.write(options)`.
    // - Required contract: `ptr` must be non-null, properly aligned for `T`, and valid for writes
    //   of `size_of::<T>()` bytes without aliasing violation.
    // - Evidence: `ptr` is non-null after the `if ptr.is_null()` check above, is aligned for `T`
    //   because `libc::malloc` aligns to `max_align_t` and the compile-time assertion above
    //   established `align_of::<T>() <= align_of::<libc::max_align_t>()`, and points to a freshly
    //   allocated `size_of::<T>()`-byte region uniquely owned by this function.
    // - Postcondition: The heap allocation at `ptr` is initialized with `options` without reading
    //   or dropping the previously uninitialized bytes, and ownership is returned to the caller as
    //   `*mut c_void` to be freed with `libc::free`.
    unsafe {
        ptr.write(options);
    }
    Ok(ptr.cast::<c_void>())
}

fn lzma_properties_decode_inner(f: &mut lzma_filter, props: &[u8]) -> lzma_ret {
    match f.id {
        FILTER_LZMA1 => {
            if props.len() != 5 {
                return LZMA_OPTIONS_ERROR;
            }
            let props_byte = props[0];
            let raw_dict_size = u32::from_le_bytes(props[1..5].try_into().unwrap());
            let dict_size = raw_dict_size.max(LZMA_DICT_SIZE_MIN);

            // The properties byte is encoded as `(pb * 5 + lp) * 9 + lc`.
            let lc = (props_byte % LZMA1_NUM_LC_VALUES as u8) as u32;
            let remainder = props_byte / LZMA1_NUM_LC_VALUES as u8;
            let lp = (remainder % LZMA1_NUM_LP_VALUES as u8) as u32;
            let pb = (remainder / LZMA1_NUM_LP_VALUES as u8) as u32;

            if pb > 4 || lc + lp > 4 {
                return LZMA_OPTIONS_ERROR;
            }

            match alloc_options(lzma_options_lzma {
                dict_size,
                preset_dict: core::ptr::null(),
                preset_dict_size: 0,
                lc,
                lp,
                pb,
                mode: LZMA_MODE_NORMAL,
                nice_len: 64,
                mf: LZMA_MF_BT4,
                depth: 0,
                reserved_int1: 0,
                reserved_int2: 0,
                reserved_int3: 0,
                reserved_int4: 0,
                reserved_int5: 0,
                reserved_int6: 0,
                reserved_int7: 0,
                reserved_int8: 0,
                reserved_enum1: 0,
                reserved_enum2: 0,
                reserved_enum3: 0,
                reserved_enum4: 0,
                reserved_ptr1: core::ptr::null_mut(),
                reserved_ptr2: core::ptr::null_mut(),
            }) {
                Ok(ptr) => {
                    f.options = ptr;
                    LZMA_OK
                }
                Err(e) => e,
            }
        }
        FILTER_LZMA2 => {
            if props.len() != 1 {
                return LZMA_OPTIONS_ERROR;
            }
            let prop = props[0];
            if prop > LZMA2_PROP_MAX {
                return LZMA_OPTIONS_ERROR;
            }
            let dict_size = lzma2_prop_to_dict_size(prop);

            match alloc_options(lzma_options_lzma {
                dict_size,
                preset_dict: core::ptr::null(),
                preset_dict_size: 0,
                lc: 3,
                lp: 0,
                pb: 2,
                mode: LZMA_MODE_NORMAL,
                nice_len: 64,
                mf: LZMA_MF_BT4,
                depth: 0,
                reserved_int1: 0,
                reserved_int2: 0,
                reserved_int3: 0,
                reserved_int4: 0,
                reserved_int5: 0,
                reserved_int6: 0,
                reserved_int7: 0,
                reserved_int8: 0,
                reserved_enum1: 0,
                reserved_enum2: 0,
                reserved_enum3: 0,
                reserved_enum4: 0,
                reserved_ptr1: core::ptr::null_mut(),
                reserved_ptr2: core::ptr::null_mut(),
            }) {
                Ok(ptr) => {
                    f.options = ptr;
                    LZMA_OK
                }
                Err(e) => e,
            }
        }
        FILTER_DELTA => {
            if props.len() != 1 {
                return LZMA_OPTIONS_ERROR;
            }
            let dist = props[0] as u32 + 1;
            match alloc_options(lzma_options_delta {
                type_: LZMA_DELTA_TYPE_BYTE,
                dist,
                reserved_int1: 0,
                reserved_int2: 0,
                reserved_int3: 0,
                reserved_int4: 0,
                reserved_ptr1: core::ptr::null_mut(),
                reserved_ptr2: core::ptr::null_mut(),
            }) {
                Ok(ptr) => {
                    f.options = ptr;
                    LZMA_OK
                }
                Err(e) => e,
            }
        }
        FILTER_X86 | FILTER_POWERPC | FILTER_IA64 | FILTER_ARM | FILTER_ARMTHUMB | FILTER_SPARC
        | FILTER_ARM64 | FILTER_RISCV => {
            if !props.is_empty() {
                return LZMA_OPTIONS_ERROR;
            }
            f.options = core::ptr::null_mut();
            LZMA_OK
        }
        ..=VLI_MAX => LZMA_OPTIONS_ERROR,
        _ => LZMA_PROG_ERROR,
    }
}

/// Convert LZMA2 dict_size to property byte.
fn lzma2_dict_size_to_prop(dict_size: u32) -> u8 {
    let mut d = core::cmp::max(dict_size, LZMA_DICT_SIZE_MIN);

    // Round up to the next 2^n - 1 or 2^n + 2^(n - 1) - 1 depending
    // on which one is the next.
    d -= 1;
    d |= d >> 2;
    d |= d >> 3;
    d |= d >> 4;
    d |= d >> 8;
    d |= d >> 16;

    // Get the highest two bits using the proper encoding:
    if d == u32::MAX {
        return LZMA2_PROP_MAX;
    }

    let dist = d + 1;
    let i = 31 - dist.leading_zeros();
    let get_dist_slot = (i + i) + ((dist >> (i - 1)) & 1);

    (get_dist_slot - 24) as u8
}

/// Convert LZMA2 property byte to dict_size.
fn lzma2_prop_to_dict_size(prop: u8) -> u32 {
    if prop >= LZMA2_PROP_MAX {
        return u32::MAX;
    }
    if prop == 0 {
        return 4096;
    }
    (2 | (prop as u32 & 1)) << (prop as u32 / 2 + 11)
}
