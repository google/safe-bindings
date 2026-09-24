//! Decoder initialization functions: lzma_stream_decoder,
//! lzma_alone_decoder.

#[cfg(feature = "lzip")]
use lzma_rust2::LzipStream;
use lzma_rust2::{
    EncodeMode, Lzma2Stream, LzmaOptions, LzmaStream, MfType, XzStream, DICT_SIZE_MIN,
};

use crate::helpers;
use crate::state::{ActionState, AutoDecoder, CoderInner, InternalState, RawProps};
use crate::types::{
    lzma_filter, lzma_options_lzma, lzma_ret, lzma_stream, LZMA_CONCATENATED, LZMA_FILTER_LZMA1,
    LZMA_FILTER_LZMA2, LZMA_LC_DEFAULT, LZMA_LP_DEFAULT, LZMA_OK, LZMA_OPTIONS_ERROR,
    LZMA_PB_DEFAULT, LZMA_PROG_ERROR, LZMA_SUPPORTED_FLAGS, LZMA_TELL_ANY_CHECK,
    LZMA_TELL_NO_CHECK,
};

/// Initialize `.xz` stream decoder.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
///
/// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
#[unsafe(export_name = crate::prefix!(lzma_stream_decoder))]
pub unsafe extern "C" fn lzma_stream_decoder(
    strm: *mut lzma_stream,
    memlimit: u64,
    flags: u32,
) -> lzma_ret {
    if strm.is_null() {
        return LZMA_PROG_ERROR;
    }

    if flags & !LZMA_SUPPORTED_FLAGS != 0 {
        return LZMA_OPTIONS_ERROR;
    }

    let allow_multiple = (flags & LZMA_CONCATENATED) != 0;
    let memlimit_kb = if memlimit == u64::MAX {
        u32::MAX
    } else {
        (memlimit / 1024).clamp(0, u32::MAX as u64) as u32
    };

    let decoder = XzStream::new_mem_limit(allow_multiple, memlimit_kb);
    let mut state = InternalState::new(CoderInner::XzDecoder(Box::new(decoder)));
    state.tell_flags = flags & (LZMA_TELL_NO_CHECK | LZMA_TELL_ANY_CHECK);

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate `internal`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}

/// Initialize `.lzma` decoder.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
///
/// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
#[unsafe(export_name = crate::prefix!(lzma_alone_decoder))]
pub unsafe extern "C" fn lzma_alone_decoder(strm: *mut lzma_stream, memlimit: u64) -> lzma_ret {
    if strm.is_null() {
        return LZMA_PROG_ERROR;
    }

    let memlimit_kb = if memlimit == u64::MAX {
        // lzma_rust2 receives memlimit in KiB, not bytes. We convert it here.
        // u64::MAX is a special value indicating no limit in liblzma
        // In lzma_rust2, that special value is u32::MAX.
        u32::MAX
    } else {
        (memlimit / 1024).clamp(0, u32::MAX as u64) as u32
    };

    let state = InternalState::new(CoderInner::LzmaDecoder(Box::new(LzmaStream::new_mem_limit(
        memlimit_kb,
        None,
    ))));

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate `internal`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}

/// Initialize a raw decoder with a filter chain.
///
/// # Safety
/// Caller must ensure `strm` is either null (handled) or points to a valid,
/// writable `lzma_stream` over which the caller has exclusive ownership
/// (no concurrent aliasing) during this call.
/// `filters` must point to a valid, LZMA_VLI_UNKNOWN-terminated array of filters.
/// If any filter's `options` field is non-null, it must point to a valid
/// options struct corresponding to its ID (e.g., `lzma_options_lzma` for LZMA1/2,
/// `lzma_options_delta` for Delta, `lzma_options_bcj` for BCJ filters).
#[unsafe(export_name = crate::prefix!(lzma_raw_decoder))]
pub unsafe extern "C" fn lzma_raw_decoder(
    strm: *mut lzma_stream,
    filters: *const lzma_filter,
) -> lzma_ret {
    if strm.is_null() || filters.is_null() {
        return LZMA_PROG_ERROR;
    }

    let mut extra_filters = Vec::new();
    let mut lzma_f_opt = None;

    // SAFETY: `filters` is non-null (checked above) and points to a valid,
    // `LZMA_VLI_UNKNOWN`-terminated array of filters per the public safety precondition.
    let filter_slice = match unsafe { helpers::parse_filters(filters) } {
        Ok(s) => s,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    for (i, f) in filter_slice.iter().enumerate() {
        if f.id == LZMA_FILTER_LZMA2.into() || f.id == LZMA_FILTER_LZMA1.into() {
            lzma_f_opt = Some(f);

            // According to both liblzma and lzma_rust2, the last filter should be either lzma1 or lzma2.
            if i + 1 != filter_slice.len() {
                return LZMA_OPTIONS_ERROR;
            }
            break;
        } else if let Some(filter_cfg) = unsafe {
            // SAFETY: The provided filter `f` is valid, and its `options` pointer
            // correctly matches `f.id` per the FFI filter array safety precondition.
            helpers::filter_config_from_c(f)
        } {
            extra_filters.push(filter_cfg);
        } else {
            return LZMA_OPTIONS_ERROR;
        }
    }

    let f = match lzma_f_opt {
        Some(ptr) => ptr,
        None => return LZMA_OPTIONS_ERROR,
    };

    // Use default dict size; raw decoder typically gets dict_size from filter options
    let raw_props = if !f.options.is_null() {
        // SAFETY: The `options` pointer is valid and non-null, pointing to a properly aligned
        // `lzma_options_lzma` struct as required by the public safety contract.
        let opts = unsafe { &*(f.options as *const lzma_options_lzma) };
        if f.id == LZMA_FILTER_LZMA1.into() {
            if opts.lc > 4 || opts.lp > 4 || opts.lc + opts.lp > 4 || opts.pb > 4 {
                return LZMA_OPTIONS_ERROR;
            }
        }
        RawProps { dict_size: opts.dict_size, lc: opts.lc, lp: opts.lp, pb: opts.pb }
    } else {
        RawProps {
            dict_size: DICT_SIZE_MIN,
            lc: LZMA_LC_DEFAULT,
            lp: LZMA_LP_DEFAULT,
            pb: LZMA_PB_DEFAULT,
        }
    };

    let coder = if f.id == LZMA_FILTER_LZMA2.into() {
        let mut stream = Lzma2Stream::new(raw_props.dict_size);
        if stream.set_filters(&extra_filters).is_err() {
            return LZMA_OPTIONS_ERROR;
        }
        CoderInner::RawLzma2Decoder(Box::new(stream))
    } else {
        // Compute properties byte. Props is encoded as `(pb * 5 + lp) * 9 + lc`
        let props_byte = (raw_props.pb * 5 + raw_props.lp) * 9 + raw_props.lc;
        match LzmaStream::new_with_props(
            u64::MAX, // uncompressed size unknown
            props_byte as u8,
            raw_props.dict_size,
            None,
        ) {
            Ok(mut stream) => {
                if stream.set_filters(&extra_filters).is_err() {
                    return LZMA_OPTIONS_ERROR;
                }
                CoderInner::LzmaDecoder(Box::new(stream))
            }
            Err(_) => return LZMA_PROG_ERROR,
        }
    };

    let state = InternalState {
        coder,
        action_state: ActionState::Run,
        allow_buf_error: false,
        lzma_options: Some(LzmaOptions::new(
            raw_props.dict_size,
            raw_props.lc,
            raw_props.lp,
            raw_props.pb,
            EncodeMode::Normal,
            64,
            MfType::Bt4,
            0,
        )),
        swallowed_unexpected_eof: false,
        tell_flags: 0,
    };

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate `internal`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}

/// Initialize Lzip decoder.
///
/// Note: `LzipStream` in `lzma_rust2` always decodes concatenated `.lz`
/// members by default. Unlike standard liblzma, which requires `LZMA_CONCATENATED`
/// to decode multiple members and otherwise stops after the first member, this
/// implementation will always decode concatenated members regardless of whether
/// `LZMA_CONCATENATED` is set in `flags`.
///
/// Note: only `.lz` version 1 is decodable, whereas liblzma also decodes version 0.
/// NOTE(b/562256728): support `.lz` version 0.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// If `strm.internal` is non-null, it must point to a valid `InternalState` allocated by
/// this crate.
/// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
#[cfg(feature = "lzip")]
#[unsafe(export_name = crate::prefix!(lzma_lzip_decoder))]
pub unsafe extern "C" fn lzma_lzip_decoder(
    strm: Option<&mut lzma_stream>,
    memlimit: u64,
    flags: u32,
) -> lzma_ret {
    let Some(strm) = strm else {
        return LZMA_PROG_ERROR;
    };
    if flags & !(LZMA_SUPPORTED_FLAGS as u32) != 0 {
        return LZMA_OPTIONS_ERROR;
    }
    let memlimit_kb = if memlimit == u64::MAX {
        u32::MAX
    } else {
        (memlimit / 1024).clamp(0, u32::MAX as u64) as u32
    };

    let state = InternalState::new(CoderInner::LzipDecoder(Box::new(LzipStream::new_mem_limit(
        memlimit_kb,
    ))));
    // SAFETY: Exclusive ownership of `strm` is guaranteed by `&mut lzma_stream`. If `strm.internal`
    // is non-null, it points to a valid `InternalState` per safety precondition.
    unsafe { helpers::install_state(strm, state) };
    LZMA_OK
}

/// Initialize auto decoder.
///
/// # Safety
/// `strm` must be null or must point to a valid, properly aligned, initialized
/// (e.g. via `LZMA_STREAM_INIT`), writable `lzma_stream` that is unaliased for this call.
/// If `strm.internal` is non-null, it must point to a valid `InternalState` allocated by
/// this crate.
#[unsafe(export_name = crate::prefix!(lzma_auto_decoder))]
pub unsafe extern "C" fn lzma_auto_decoder(
    strm: Option<&mut lzma_stream>,
    memlimit: u64,
    flags: u32,
) -> lzma_ret {
    let Some(strm) = strm else {
        return LZMA_PROG_ERROR;
    };

    if flags & !(LZMA_SUPPORTED_FLAGS as u32) != 0 {
        return LZMA_OPTIONS_ERROR;
    }

    let mut state =
        InternalState::new(CoderInner::AutoDecoder(Box::new(AutoDecoder::new(memlimit, flags))));
    state.tell_flags = flags & (LZMA_TELL_NO_CHECK | LZMA_TELL_ANY_CHECK);

    // SAFETY: Exclusive ownership of `strm` is guaranteed by `&mut lzma_stream`. If `strm.internal`
    // is non-null, it points to a valid `InternalState` per safety precondition.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}
