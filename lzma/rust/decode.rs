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
/// `memlimit` is converted to KiB internally; a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` (e.g. initialized with `LZMA_STREAM_INIT`) over which this call has
///    exclusive access for the duration of this call.
/// 2. If `strm` is non-null and `(*strm).internal` is non-null, `(*strm).internal` points to a live
///    `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `(*strm).internal` points
/// to a newly allocated `InternalState` owned by `*strm` until freed by `lzma_end` or replaced
/// by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_stream_decoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_stream_decoder(lzma_stream* strm, uint64_t memlimit, uint32_t flags)` in
//   `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, state)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and preconditions 1
    //   and 2 of this function's `# Safety` contract guarantee alignment, initialization, exclusive
    //   access, and `(*strm).internal` provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}

/// Initialize `.lzma` decoder.
///
/// `memlimit` is converted to KiB internally; a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` (e.g. initialized with `LZMA_STREAM_INIT`) over which this call has
///    exclusive access for the duration of this call.
/// 2. If `strm` is non-null and `(*strm).internal` is non-null, `(*strm).internal` points to a live
///    `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `(*strm).internal` points
/// to a newly allocated `InternalState` owned by `*strm` until freed by `lzma_end` or replaced
/// by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_alone_decoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_alone_decoder(lzma_stream* strm, uint64_t memlimit)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, state)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and preconditions 1
    //   and 2 of this function's `# Safety` contract guarantee alignment, initialization, exclusive
    //   access, and `(*strm).internal` provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}

/// Initialize a raw decoder with a filter chain.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` (e.g. initialized with `LZMA_STREAM_INIT`) over which this call has
///    exclusive access for the duration of this call.
/// 2. If `strm` is non-null and `(*strm).internal` is non-null, `(*strm).internal` points to a live
///    `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
/// 3. `filters` is either null or a non-null, properly aligned pointer to a contiguous sequence of
///    initialized `lzma_filter` elements within a single live allocated object that either contains
///    a terminator element with `id == LZMA_VLI_UNKNOWN` at some index `i <= FILTERS_MAX` (4) or
///    contains at least `FILTERS_MAX + 1` (5) initialized elements, and is not mutated or aliased
///    by `*strm` for the duration of this call.
/// 4. For every filter element preceding the `LZMA_VLI_UNKNOWN` terminator (within the first
///    `FILTERS_MAX` elements) whose `options` field is non-null:
///    - When `id == u64::from(LZMA_FILTER_LZMA1)` or `id == u64::from(LZMA_FILTER_LZMA2)`,
///      `options` is properly aligned for `lzma_options_lzma` and points to a live, initialized
///      `lzma_options_lzma` valid for reads and not mutated for the duration of this call.
///    - When `id == u64::from(LZMA_FILTER_DELTA)`, `options` is properly aligned for
///      `lzma_options_delta` and points to a live, initialized `lzma_options_delta` valid for reads
///      and not mutated for the duration of this call.
///    - When `id` is a supported BCJ filter ID, `options` is properly aligned for
///      `lzma_options_bcj` and points to a live, initialized `lzma_options_bcj` valid for reads and
///      not mutated for the duration of this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `(*strm).internal` points
/// to a newly allocated `InternalState` owned by `*strm` until freed by `lzma_end` or replaced
/// by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_raw_decoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_raw_decoder(lzma_stream* strm, const lzma_filter* filters)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `helpers::parse_filters(filters)`.
    // - Required contract: `filters` must be non-null, aligned for `lzma_filter`, and point to a
    //   single live allocation containing either an `LZMA_VLI_UNKNOWN` terminator within index `<=
    //   FILTERS_MAX` or at least `FILTERS_MAX + 1` initialized `lzma_filter` elements not mutated
    //   during the returned slice's lifetime.
    // - Evidence: `!filters.is_null()` is checked above, and precondition 3 of this function's `#
    //   Safety` contract guarantees alignment, single allocation, termination/length bounds, and
    //   immutability during this call.
    // - Postcondition: `filter_slice` is a valid shared slice `&[lzma_filter]` of length `<=
    //   FILTERS_MAX` confined to this call.
    let filter_slice = match unsafe { helpers::parse_filters(filters) } {
        Ok(s) => s,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    for (i, f) in filter_slice.iter().enumerate() {
        if f.id == LZMA_FILTER_LZMA2.into() || f.id == LZMA_FILTER_LZMA1.into() {
            lzma_f_opt = Some(f);

            // According to both liblzma and lzma_rust2, the last filter should be either lzma1 or
            // lzma2.
            if i + 1 != filter_slice.len() {
                return LZMA_OPTIONS_ERROR;
            }
            break;
        } else {
            // SAFETY:
            // - Operation: `helpers::filter_config_from_c(f)`.
            // - Required contract: If `f.options` is non-null and `f.id` is a Delta or BCJ filter
            //   ID, `f.options` must be aligned and point to a live, initialized
            //   `lzma_options_delta` or `lzma_options_bcj` not mutated during the call.
            // - Evidence: As established by `helpers::parse_filters` above, `f` is an element of
            //   `filter_slice` preceding the `LZMA_VLI_UNKNOWN` terminator, so precondition 4 of
            //   this function's `# Safety` contract guarantees the required `f.options` validity.
            // - Postcondition: Returns an owned `Option<FilterConfig>` without retaining pointers.
            if let Some(filter_cfg) = unsafe { helpers::filter_config_from_c(f) } {
                extra_filters.push(filter_cfg);
            } else {
                return LZMA_OPTIONS_ERROR;
            }
        }
    }

    let f = match lzma_f_opt {
        Some(ptr) => ptr,
        None => return LZMA_OPTIONS_ERROR,
    };

    // Use default dict size; raw decoder typically gets dict_size from filter options
    let raw_props = if !f.options.is_null() {
        // SAFETY:
        // - Operation: Creating a shared reference `&*(f.options as *const lzma_options_lzma)`.
        // - Required contract: `f.options` must be non-null, properly aligned for
        //   `lzma_options_lzma`, and point to a live, initialized `lzma_options_lzma` not mutated
        //   for the duration of the reference.
        // - Evidence: `!f.options.is_null()` is checked above, `f.id` was verified in the loop to
        //   be `LZMA_FILTER_LZMA1` or `LZMA_FILTER_LZMA2`, and precondition 4 of this function's `#
        //   Safety` contract guarantees alignment, initialization, and immutability during this
        //   call.
        // - Postcondition: `opts` is a valid shared reference `&lzma_options_lzma` whose integer
        //   fields are copied into `RawProps`.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, state)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, all borrows of
    //   `filters` have ended, and preconditions 1 and 2 of this function's `# Safety` contract
    //   guarantee alignment, initialization, exclusive access, and `(*strm).internal` provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
/// `memlimit` is converted to KiB internally; a limit lower than 1024 bytes will cause the
/// decoder to fail. This is similar to liblzma.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either `None` (null in C) or a valid exclusive reference to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access for the duration of this
///    call.
/// 2. If `strm` is `Some(strm_ref)` and `strm_ref.internal` is non-null, `strm_ref.internal` points
///    to a live `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `strm_ref.internal` points
/// to a newly allocated `InternalState` owned by `*strm_ref` until freed by `lzma_end` or
/// replaced by another initialization function.
#[cfg(feature = "lzip")]
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_lzip_decoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_lzip_decoder(lzma_stream* strm, uint64_t memlimit, uint32_t flags)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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
    // SAFETY:
    // - Operation: `helpers::install_state(strm, state)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null `strm.internal`
    //   must have been allocated by this crate via `Box::into_raw(Box::new(InternalState))` and not
    //   yet freed.
    // - Evidence: `strm` is a valid `&mut lzma_stream` by precondition 1, and precondition 2 of
    //   this function's `# Safety` contract guarantees the provenance and exclusive ownership of
    //   any non-null `strm.internal`.
    // - Postcondition: Any old `strm.internal` is freed, `strm.internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
    unsafe { helpers::install_state(strm, state) };
    LZMA_OK
}

/// Initialize auto decoder.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either `None` (null in C) or a valid exclusive reference to a live, initialized,
///    writable `lzma_stream` (e.g. initialized via `LZMA_STREAM_INIT`) over which this call has
///    exclusive access for the duration of this call.
/// 2. If `strm` is `Some(strm_ref)` and `strm_ref.internal` is non-null, `strm_ref.internal` points
///    to a live `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `strm_ref.internal` points
/// to a newly allocated `InternalState` owned by `*strm_ref` until freed by `lzma_end` or
/// replaced by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_auto_decoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_auto_decoder(lzma_stream* strm, uint64_t memlimit, uint32_t flags)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, state)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null `strm.internal`
    //   must have been allocated by this crate via `Box::into_raw(Box::new(InternalState))` and not
    //   yet freed.
    // - Evidence: `strm` is a valid `&mut lzma_stream` by precondition 1, and precondition 2 of
    //   this function's `# Safety` contract guarantees the provenance and exclusive ownership of
    //   any non-null `strm.internal`.
    // - Postcondition: Any old `strm.internal` is freed, `strm.internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
    unsafe { helpers::install_state(strm, state) };

    LZMA_OK
}
