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

/// Fill `options` with values from `preset`.
///
/// Returns 0 (false) on success, 1 (true) on failure (matching liblzma's
/// inverted boolean convention).
///
/// # Safety
/// `options` must point to a valid, writable `lzma_options_lzma`.
/// The memory pointed to by `options` must be initialized.
// NOTE(b/552362497): Change the raw pointers into references once MaybeUninit is supported in safer_cffi.
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
        ..unsafe { core::mem::zeroed() }
    };

    // SAFETY: `options` is a valid, writable pointer to an initialized memory region,
    // as required by the safety precondition.
    unsafe {
        core::ptr::write(options, opts);
    }

    0 // success
}

/// Get the size of the encoded properties for a filter.
///
/// For LZMA1/LZMA2, the properties are always 5 bytes (or 1 byte for LZMA2).
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
/// `props` must be a valid pointer to a writable memory region
/// with enough space for the property bytes.
/// Users could use `lzma_properties_size()` to get the required size.
///
/// `filter.options` must point to an initialized valid struct
/// for that filter type (e.g., `lzma_options_lzma` for LZMA1/2,
/// or `lzma_options_delta` for DELTA).
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
        // SAFETY: `props` has room for `props_size` bytes, as required by the safety precondition.
        unsafe {
            core::ptr::write_bytes(props, 0, props_size);
            core::slice::from_raw_parts_mut(props, props_size)
        }
    } else {
        &mut []
    };

    lzma_properties_encode_inner(f, props_slice)
}

fn lzma_properties_encode_inner(f: &lzma_filter, props_slice: &mut [u8]) -> lzma_ret {
    match f.id {
        FILTER_LZMA1 => {
            if f.options.is_null() {
                return LZMA_PROG_ERROR;
            }
            // SAFETY: As documented in the public safety invariant, the caller guarantees
            // `f.options` is a valid pointer to an initialized `lzma_options_lzma` state
            // for LZMA1 filters.
            let rust_opts_result =
                unsafe { crate::state::lzma_options_from_c(f.options.cast::<lzma_options_lzma>()) };

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
            // SAFETY: As documented in the public safety invariant, the caller guarantees
            // `f.options` is a valid pointer to an initialized `lzma_options_lzma` state
            // for LZMA2 filters.
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
            // SAFETY: As documented in the public safety invariant, the caller guarantees
            // `f.options` is a valid pointer to an initialized `lzma_options_delta` state
            // for DELTA filters.
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
/// `props` must point to `props_size` bytes.
/// The memory pointed to by `props` must be initialized.
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
    // SAFETY:
    // - If `props_size > 0`, `props` points to `props_size` bytes of initialized memory, as required by the safety precondition.
    let props = if props_size > 0 {
        unsafe { core::slice::from_raw_parts(props, props_size) }
    } else {
        &[]
    };

    lzma_properties_decode_inner(f, props)
}

/// We must allocate the options structs via the C allocator rather than
/// `Box::new`, because the caller expects to free them with C's `free()`.
fn alloc_options<T>(options: T) -> Result<*mut c_void, lzma_ret> {
    // SAFETY: Calling `malloc` is safe, and we check the returned pointer for null before using it.
    let ptr = unsafe { libc::malloc(core::mem::size_of::<T>()) }.cast::<T>();
    if ptr.is_null() {
        return Err(LZMA_MEM_ERROR);
    }
    // SAFETY: `ptr` is a valid pointer allocated just above.
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
