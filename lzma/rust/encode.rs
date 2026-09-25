//! Encoder initialization functions: lzma_easy_encoder, lzma_stream_encoder,
//! lzma_stream_encoder_mt.

use std::num::NonZeroU64;

use lzma_rust2::{
    EncodeMode, Lzma2Options, Lzma2Writer, LzmaOptions, LzmaWriter, MfType, XzOptions, XzWriter,
};
#[cfg(feature = "lzip")]
use lzma_rust2::{LzipOptions, LzipWriter};

use crate::helpers;
use crate::state::{
    self, ActionState, CoderInner, InternalState, WriterAdapter, WriterAdapterOwned,
};
use crate::types::{
    lzma_filter, lzma_mt, lzma_options_lzma, lzma_ret, lzma_stream, LZMA_FILTER_LZMA1,
    LZMA_FILTER_LZMA2, LZMA_OK, LZMA_OPTIONS_ERROR, LZMA_PRESET_EXTREME, LZMA_PROG_ERROR,
};

/// Initialize `.xz` stream encoder using a preset number.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
#[unsafe(export_name = crate::prefix!(lzma_easy_encoder))]
pub unsafe extern "C" fn lzma_easy_encoder(
    strm: *mut lzma_stream,
    preset: u32,
    check: u32,
) -> lzma_ret {
    if strm.is_null() {
        return LZMA_PROG_ERROR;
    }

    let level = preset & !LZMA_PRESET_EXTREME;
    if level > 9 {
        return LZMA_OPTIONS_ERROR;
    }

    let Some(check_type) = state::check_type_from_c(check) else {
        return LZMA_OPTIONS_ERROR;
    };

    let mut opts = XzOptions { check_type, ..XzOptions::with_preset(level) };
    if preset & LZMA_PRESET_EXTREME != 0 {
        opts.lzma_options.mode = EncodeMode::Normal;
        opts.lzma_options.mf = MfType::Bt4;
        opts.lzma_options.nice_len = LzmaOptions::NICE_LEN_MAX;
        opts.lzma_options.depth_limit = 0;
    }

    let saved_opts = opts.lzma_options.clone();
    let Ok(encoder) = XzWriter::new(Vec::new(), opts) else {
        return LZMA_OPTIONS_ERROR;
    };

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate internal state.
    unsafe {
        helpers::install_state(
            strm,
            InternalState {
                coder: CoderInner::XzEncoder(WriterAdapter::new(encoder)),
                action_state: ActionState::Run,
                allow_buf_error: false,
                lzma_options: Some(saved_opts),
                swallowed_unexpected_eof: false,
                tell_flags: 0,
            },
        );
    }

    LZMA_OK
}

/// Initialize `.xz` stream encoder using a custom filter chain.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// `filters` must point to a valid array of `lzma_filter` terminated by a filter
/// with `id == LZMA_VLI_UNKNOWN`. Each `lzma_filter->options` pointer
/// must also be valid (either null or pointing to the correct options struct based on its `id`, e.g.,
/// `lzma_options_lzma` for `LZMA_FILTER_LZMA2`, `lzma_options_delta` for `LZMA_FILTER_DELTA`,
/// or `lzma_options_bcj` for BCJ filters).
#[unsafe(export_name = crate::prefix!(lzma_stream_encoder))]
pub unsafe extern "C" fn lzma_stream_encoder(
    strm: *mut lzma_stream,
    filters: *const lzma_filter,
    check: u32,
) -> lzma_ret {
    if strm.is_null() || filters.is_null() {
        return LZMA_PROG_ERROR;
    }

    let Some(check_type) = state::check_type_from_c(check) else {
        return LZMA_OPTIONS_ERROR;
    };

    // SAFETY: `filters` points to a consecutive array per FFI filter contract.
    let Ok(opts) = (unsafe { state::xz_options_from_filters(filters, check_type) }) else {
        return LZMA_OPTIONS_ERROR;
    };

    let saved_opts = opts.lzma_options.clone();
    let Ok(encoder) = XzWriter::new(Vec::new(), opts) else {
        return LZMA_OPTIONS_ERROR;
    };

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate internal state.
    unsafe {
        helpers::install_state(
            strm,
            InternalState {
                coder: CoderInner::XzEncoder(WriterAdapter::new(encoder)),
                action_state: ActionState::Run,
                allow_buf_error: false,
                lzma_options: Some(saved_opts),
                swallowed_unexpected_eof: false,
                tell_flags: 0,
            },
        );
    }

    LZMA_OK
}

/// Initialize `.xz` multithreaded stream encoder using the provided options.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// `options` must be either null or point to a valid `lzma_mt` struct.
/// If `options` is non-null and `options->filters` is non-null, the filters array must be a
/// valid array terminated by a filter with `id == LZMA_VLI_UNKNOWN`, and each filter's
/// `options` pointer must either be null or valid according to its `id`.
#[unsafe(export_name = crate::prefix!(lzma_stream_encoder_mt))]
pub unsafe extern "C" fn lzma_stream_encoder_mt(
    strm: *mut lzma_stream,
    options: *const lzma_mt,
) -> lzma_ret {
    if strm.is_null() || options.is_null() {
        return LZMA_PROG_ERROR;
    }

    // SAFETY: Per caller promise, `options` is a valid pointer to `lzma_mt` if non-null.
    // The null check was performed above.
    let mt = unsafe { &*options };

    if mt.flags != 0
        || mt.reserved_enum1 != 0
        || mt.reserved_enum2 != 0
        || mt.reserved_enum3 != 0
        || !mt.reserved_ptr1.is_null()
        || !mt.reserved_ptr2.is_null()
        || !mt.reserved_ptr3.is_null()
        || !mt.reserved_ptr4.is_null()
        || mt.reserved_int1 != 0
        || mt.reserved_int2 != 0
        || mt.reserved_int3 != 0
        || mt.reserved_int4 != 0
        || mt.memlimit_threading != 0
        || mt.memlimit_stop != 0
        || mt.reserved_int7 != 0
        || mt.reserved_int8 != 0
        || mt.timeout != 0
    {
        return LZMA_OPTIONS_ERROR;
    }
    let threads = match mt.threads {
        0 => crate::util::lzma_cputhreads().max(1),
        n => n,
    };

    let Some(check_type) = state::check_type_from_c(mt.check) else {
        return LZMA_OPTIONS_ERROR;
    };

    let mut opts = if !mt.filters.is_null() {
        // SAFETY: `mt.filters` points to a consecutive array per FFI filter contract.
        let Ok(o) = (unsafe { state::xz_options_from_filters(mt.filters, check_type) }) else {
            return LZMA_OPTIONS_ERROR;
        };
        o
    } else {
        // The preset might contain the EXTREME flag. We remove it here
        // so we can check if the base compression level is between 0 and 9.
        let level = mt.preset & !LZMA_PRESET_EXTREME;
        if level > 9 {
            return LZMA_OPTIONS_ERROR;
        }

        let mut o = XzOptions::with_preset(level);
        o.check_type = check_type;
        o
    };

    if mt.block_size > 0 {
        opts.block_size = NonZeroU64::new(mt.block_size);
    } else {
        let default_block = (opts.lzma_options.dict_size as u64) * 3;
        opts.block_size = NonZeroU64::new(default_block.max(1));
    }

    let saved_opts = opts.lzma_options.clone();
    let adapter = match WriterAdapterOwned::try_new(opts, threads) {
        Ok(a) => a,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    // SAFETY: `strm` is non-null and valid per caller promise. We have exclusive ownership of `strm`
    // during this initialization call to safely mutate internal state.
    unsafe {
        helpers::install_state(
            strm,
            InternalState {
                coder: CoderInner::XzEncoderMt(adapter),
                action_state: ActionState::Run,
                allow_buf_error: false,
                lzma_options: Some(saved_opts),
                swallowed_unexpected_eof: false,
                tell_flags: 0,
            },
        );
    }

    LZMA_OK
}

/// Initialize `.lzma` encoder (legacy LZMA_Alone format).
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// If `strm` is non-null and `strm.internal` is non-null, it must point to a valid `InternalState`
/// allocated by this crate.
/// If `options` is non-null and `options.preset_dict` is non-null, it must point to a valid memory
/// buffer of at least `options.preset_dict_size` bytes.
#[unsafe(export_name = crate::prefix!(lzma_alone_encoder))]
pub unsafe extern "C" fn lzma_alone_encoder(
    strm: Option<&mut lzma_stream>,
    options: Option<&lzma_options_lzma>,
) -> lzma_ret {
    let (Some(strm), Some(options)) = (strm, options) else {
        return LZMA_PROG_ERROR;
    };

    if let Err(e) = state::validate_encoder_options(options) {
        return e;
    }

    // SAFETY: `options.preset_dict`, if non-null, is valid per safety precondition.
    let mut lzma_opts = match unsafe { state::lzma_options_from_c(options as *const _) } {
        Ok(opts) => opts,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    lzma_opts.dict_size = state::round_up_dict_size(lzma_opts.dict_size);

    let encoder = match LzmaWriter::new_use_header(Vec::new(), &lzma_opts, None) {
        Ok(e) => e,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    // SAFETY: Exclusive ownership of `strm` is guaranteed by `&mut lzma_stream`. If `strm.internal`
    // is non-null, it points to a valid `InternalState` per safety precondition.
    unsafe {
        helpers::install_state(
            strm,
            InternalState {
                coder: CoderInner::LzmaAloneEncoder(WriterAdapter::new(encoder)),
                action_state: ActionState::Run,
                allow_buf_error: false,
                lzma_options: Some(lzma_opts),
                swallowed_unexpected_eof: false,
                tell_flags: 0,
            },
        );
    }

    LZMA_OK
}

/// Initialize raw encoder.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// If `strm` is non-null and `strm.internal` is non-null, it must point to a valid `InternalState`
/// allocated by this crate.
/// `filters` must point to a valid array of `lzma_filter` terminated by a filter
/// with `id == LZMA_VLI_UNKNOWN`.
/// Each `filter.options` must point to an initialized valid struct for that filter type
/// (e.g., `lzma_options_lzma` for LZMA1/2).
#[unsafe(export_name = crate::prefix!(lzma_raw_encoder))]
pub unsafe extern "C" fn lzma_raw_encoder(
    strm: Option<&mut lzma_stream>,
    filters: *const lzma_filter,
) -> lzma_ret {
    let Some(strm) = strm else {
        return LZMA_PROG_ERROR;
    };
    if filters.is_null() {
        return LZMA_PROG_ERROR;
    }

    // SAFETY: `filters` is non-null and points to a valid array terminated by LZMA_VLI_UNKNOWN
    // per safety precondition.
    let filter_slice = match unsafe { helpers::parse_filters(filters) } {
        Ok(s) => s,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    if filter_slice.is_empty() {
        return LZMA_PROG_ERROR;
    }

    if filter_slice.len() != 1 {
        return LZMA_OPTIONS_ERROR;
    }

    let f = &filter_slice[0];
    if f.id != LZMA_FILTER_LZMA2 as u64 && f.id != LZMA_FILTER_LZMA1 as u64 {
        return LZMA_OPTIONS_ERROR;
    }

    let lzma_opts = if f.options.is_null() {
        return LZMA_PROG_ERROR;
    } else {
        // SAFETY: `f.options` is non-null (checked above) and, per filter array safety
        // precondition, points to a valid `lzma_options_lzma`.
        let options = unsafe { &*(f.options as *const lzma_options_lzma) };

        if let Err(e) = state::validate_encoder_options(options) {
            return e;
        }

        // SAFETY: `options.preset_dict`, if non-null, is valid per safety precondition.
        match unsafe { state::lzma_options_from_c(options as *const _) } {
            Ok(opts) => opts,
            Err(_) => return LZMA_OPTIONS_ERROR,
        }
    };

    let coder = if f.id == LZMA_FILTER_LZMA2 as u64 {
        let lzma2_opts = Lzma2Options { lzma_options: lzma_opts.clone(), chunk_size: None };
        let encoder = Lzma2Writer::new(Vec::new(), lzma2_opts);
        CoderInner::RawLzma2Encoder(WriterAdapter::new(encoder))
    } else {
        let encoder = match LzmaWriter::new_no_header(Vec::new(), &lzma_opts, true) {
            Ok(e) => e,
            Err(_) => return LZMA_OPTIONS_ERROR,
        };
        CoderInner::RawLzma1Encoder(WriterAdapter::new(encoder))
    };

    // SAFETY: Exclusive ownership of `strm` is guaranteed by `&mut lzma_stream`. If `strm.internal`
    // is non-null, it points to a valid `InternalState` per safety precondition.
    unsafe {
        helpers::install_state(
            strm,
            InternalState {
                coder,
                action_state: ActionState::Run,
                allow_buf_error: false,
                lzma_options: Some(lzma_opts),
                swallowed_unexpected_eof: false,
                tell_flags: 0,
            },
        );
    }

    LZMA_OK
}

/// Initialize Lzip encoder. This method is supported by the lzma_rust2 crate,
/// but doesn't exist in the original liblzma.
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`. The caller must have
/// exclusive ownership of the struct (no concurrent aliasing) during this call.
/// If `strm.internal` is non-null, it must point to a valid `InternalState` allocated by
/// this crate.
#[cfg(feature = "lzip")]
#[unsafe(export_name = crate::prefix!(lzma_lzip_encoder))]
pub unsafe extern "C" fn lzma_lzip_encoder(
    strm: Option<&mut lzma_stream>,
    preset: u32,
) -> lzma_ret {
    let Some(strm) = strm else {
        return LZMA_PROG_ERROR;
    };
    let level = preset & !LZMA_PRESET_EXTREME;
    if level > 9 {
        return LZMA_OPTIONS_ERROR;
    }
    let opts = LzipOptions::with_preset(level);
    let encoder = LzipWriter::new(Vec::new(), opts);
    let state = InternalState::new(CoderInner::LzipEncoder(WriterAdapter::new(encoder)));
    // SAFETY: Exclusive ownership of `strm` is guaranteed by `&mut lzma_stream`. If `strm.internal`
    // is non-null, it points to a valid `InternalState` per safety precondition.
    unsafe { helpers::install_state(strm, state) };
    LZMA_OK
}
