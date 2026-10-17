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
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_easy_encoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_easy_encoder(lzma_stream* strm, uint32_t preset, lzma_check check)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, ...)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and preconditions 1
    //   and 2 of this function's `# Safety` contract guarantee alignment, initialization, exclusive
    //   access, and `(*strm).internal` provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
///    - When `id == u64::from(LZMA_FILTER_LZMA2)`, `options` is properly aligned for
///      `lzma_options_lzma` and points to a live, initialized `lzma_options_lzma` valid for reads
///      and not mutated for the duration of this call; furthermore, if its `preset_dict` is
///      non-null and `preset_dict_size > 0`, `preset_dict` points to at least `preset_dict_size as
///      usize` consecutive initialized readable `u8` bytes within a single live allocated object
///      (`<= isize::MAX as usize`) not mutated during this call.
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
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_stream_encoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_stream_encoder(lzma_stream* strm, const lzma_filter* filters, lzma_check check)` in
//   `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `state::xz_options_from_filters(filters, check_type)`.
    // - Required contract: `filters` must be non-null, aligned for `lzma_filter`, and point to a
    //   valid `LZMA_VLI_UNKNOWN`-terminated (or `>= FILTERS_MAX + 1` length) filter array whose
    //   non-null `options` (and any `preset_dict`) satisfy the alignment, initialization, and
    //   single-allocation bounds for each filter ID.
    // - Evidence: `!filters.is_null()` is checked above, and preconditions 3 and 4 of this
    //   function's `# Safety` contract guarantee all requirements on `filters`, `options`, and
    //   `preset_dict`.
    // - Postcondition: Returns an owned `io::Result<XzOptions>` without retaining raw pointers.
    let Ok(opts) = (unsafe { state::xz_options_from_filters(filters, check_type) }) else {
        return LZMA_OPTIONS_ERROR;
    };

    let saved_opts = opts.lzma_options.clone();
    let Ok(encoder) = XzWriter::new(Vec::new(), opts) else {
        return LZMA_OPTIONS_ERROR;
    };

    // SAFETY:
    // - Operation: `helpers::install_state(strm, ...)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and preconditions 1
    //   and 2 of this function's `# Safety` contract guarantee alignment, initialization, exclusive
    //   access, and `(*strm).internal` provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` (e.g. initialized with `LZMA_STREAM_INIT`) over which this call has
///    exclusive access for the duration of this call.
/// 2. If `strm` is non-null and `(*strm).internal` is non-null, `(*strm).internal` points to a live
///    `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
/// 3. `options` is either null or a non-null, properly aligned pointer to a live, initialized
///    `lzma_mt` struct valid for reads and not mutated or aliased by `*strm` for the duration of
///    this call.
/// 4. If `options` is non-null and `(*options).filters` is non-null:
///    - `(*options).filters` is properly aligned for `lzma_filter` and points to a contiguous
///      sequence of initialized `lzma_filter` elements within a single live allocated object that
///      either contains a terminator element with `id == LZMA_VLI_UNKNOWN` at some index `i <=
///      FILTERS_MAX` (4) or contains at least `FILTERS_MAX + 1` (5) initialized elements, and is
///      not mutated during this call.
///    - For every filter element preceding the `LZMA_VLI_UNKNOWN` terminator (within the first
///      `FILTERS_MAX` elements) whose `options` field is non-null:
///      - When `id == u64::from(LZMA_FILTER_LZMA2)`, `options` is properly aligned for
///        `lzma_options_lzma` and points to a live, initialized `lzma_options_lzma` valid for reads
///        and not mutated during this call; furthermore, if its `preset_dict` is non-null and
///        `preset_dict_size > 0`, `preset_dict` points to at least `preset_dict_size as usize`
///        consecutive initialized readable `u8` bytes within a single live allocated object (`<=
///        isize::MAX as usize`) not mutated during this call.
///      - When `id == u64::from(LZMA_FILTER_DELTA)`, `options` is properly aligned for
///        `lzma_options_delta` and points to a live, initialized `lzma_options_delta` valid for
///        reads and not mutated during this call.
///      - When `id` is a supported BCJ filter ID, `options` is properly aligned for
///        `lzma_options_bcj` and points to a live, initialized `lzma_options_bcj` valid for reads
///        and not mutated during this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `(*strm).internal` points
/// to a newly allocated `InternalState` owned by `*strm` until freed by `lzma_end` or replaced
/// by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_stream_encoder_mt))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_stream_encoder_mt(lzma_stream* strm, const lzma_mt* options)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_stream_encoder_mt))]
pub unsafe extern "C" fn lzma_stream_encoder_mt(
    strm: *mut lzma_stream,
    options: *const lzma_mt,
) -> lzma_ret {
    if strm.is_null() || options.is_null() {
        return LZMA_PROG_ERROR;
    }

    // SAFETY:
    // - Operation: Creating a shared reference `&*options` of type `&lzma_mt`.
    // - Required contract: `options` must be non-null, properly aligned for `lzma_mt`, and point to
    //   a live, initialized `lzma_mt` not mutated or mutably aliased for the lifetime of `mt`.
    // - Evidence: `!options.is_null()` is checked above, and precondition 3 of this function's `#
    //   Safety` contract guarantees alignment, initialization, liveness, and immutability during
    //   this call.
    // - Postcondition: `mt` is a valid shared reference `&lzma_mt` confined to this call.
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
        // SAFETY:
        // - Operation: `state::xz_options_from_filters(mt.filters, check_type)`.
        // - Required contract: `mt.filters` must be non-null, aligned for `lzma_filter`, and point
        //   to a valid `LZMA_VLI_UNKNOWN`-terminated (or `>= FILTERS_MAX + 1` length) filter array
        //   whose non-null `options` (and any `preset_dict`) satisfy the alignment, initialization,
        //   and single-allocation bounds for each filter ID.
        // - Evidence: `!mt.filters.is_null()` is checked above, and precondition 4 of this
        //   function's `# Safety` contract guarantees all requirements on `mt.filters`, `options`,
        //   and `preset_dict`.
        // - Postcondition: Returns an owned `io::Result<XzOptions>` without retaining raw pointers.
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, ...)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null
    //   `(*strm).internal` must have been allocated by this crate via
    //   `Box::into_raw(Box::new(InternalState))` and not yet freed.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, `mt` is not used
    //   after this point, and preconditions 1–3 of this function's `# Safety` contract guarantee
    //   alignment, initialization, exclusive access disjoint from `options`, and `(*strm).internal`
    //   provenance.
    // - Postcondition: Any old `(*strm).internal` is freed, `(*strm).internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
///
/// The caller must ensure that:
/// 1. `strm` is either `None` (null in C) or a valid exclusive reference to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access for the duration of this
///    call.
/// 2. If `strm` is `Some(strm_ref)` and `strm_ref.internal` is non-null, `strm_ref.internal` points
///    to a live `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
/// 3. `options` is either `None` (null in C) or a valid shared reference to a live, initialized
///    `lzma_options_lzma` disjoint from `*strm` and not mutated for the duration of this call.
/// 4. If `options` is `Some(opts)` and `opts.preset_dict` is non-null with `opts.preset_dict_size >
///    0`, `opts.preset_dict` is properly aligned for `u8` (alignment 1) and points to at least
///    `opts.preset_dict_size as usize` consecutive initialized readable `u8` bytes within a single
///    live allocated object (`<= isize::MAX as usize`) disjoint from `*strm` and not mutated for
///    the duration of this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `strm_ref.internal` points
/// to a newly allocated `InternalState` owned by `*strm_ref` until freed by `lzma_end` or
/// replaced by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_alone_encoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_alone_encoder(lzma_stream* strm, const lzma_options_lzma* options)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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

    // SAFETY:
    // - Operation: `state::lzma_options_from_c(options)`.
    // - Required contract: When `options.preset_dict` is non-null and `options.preset_dict_size >
    //   0`, `options.preset_dict` must point to at least `options.preset_dict_size as usize`
    //   initialized readable bytes in a single live allocation (`<= isize::MAX as usize`) not
    //   mutated during this call.
    // - Evidence: Guaranteed directly by precondition 4 of this function's `# Safety` contract,
    //   with no intervening mutation of `options`.
    // - Postcondition: Returns an owned `Result<LzmaOptions, lzma_ret>` without retaining raw
    //   pointers.
    let mut lzma_opts = match unsafe { state::lzma_options_from_c(options) } {
        Ok(opts) => opts,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    lzma_opts.dict_size = state::round_up_dict_size(lzma_opts.dict_size);

    let encoder = match LzmaWriter::new_use_header(Vec::new(), &lzma_opts, None) {
        Ok(e) => e,
        Err(_) => return LZMA_OPTIONS_ERROR,
    };

    // SAFETY:
    // - Operation: `helpers::install_state(strm, ...)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null `strm.internal`
    //   must have been allocated by this crate via `Box::into_raw(Box::new(InternalState))` and not
    //   yet freed.
    // - Evidence: `strm` is a valid `&mut lzma_stream` by precondition 1, and precondition 2 of
    //   this function's `# Safety` contract guarantees the provenance and exclusive ownership of
    //   any non-null `strm.internal`.
    // - Postcondition: Any old `strm.internal` is freed, `strm.internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
///
/// The caller must ensure that:
/// 1. `strm` is either `None` (null in C) or a valid exclusive reference to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access for the duration of this
///    call.
/// 2. If `strm` is `Some(strm_ref)` and `strm_ref.internal` is non-null, `strm_ref.internal` points
///    to a live `InternalState` previously allocated by this crate via
///    `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive ownership
///    transferred to this call so it can be deallocated before installing the new state.
/// 3. `filters` is either null or a non-null, properly aligned pointer to a contiguous sequence of
///    initialized `lzma_filter` elements within a single live allocated object that either contains
///    a terminator element with `id == LZMA_VLI_UNKNOWN` at some index `i <= FILTERS_MAX` (4) or
///    contains at least `FILTERS_MAX + 1` (5) initialized elements, disjoint from `*strm` and not
///    mutated for the duration of this call.
/// 4. If `filters` is non-null and its first element has `id == u64::from(LZMA_FILTER_LZMA1)` or
///    `id == u64::from(LZMA_FILTER_LZMA2)` with a non-null `options` field:
///    - `options` is properly aligned for `lzma_options_lzma` and points to a live, initialized
///      `lzma_options_lzma` valid for reads, disjoint from `*strm`, and not mutated for the
///      duration of this call.
///    - If `(*(options as *const lzma_options_lzma)).preset_dict` is non-null and `preset_dict_size
///      > 0`, `preset_dict` points to at least `preset_dict_size as usize` consecutive initialized
///      readable `u8` bytes within a single live allocated object (`<= isize::MAX as usize`),
///      disjoint from `*strm` and not mutated for the duration of this call.
///
/// If these preconditions hold and this function returns `LZMA_OK`, `strm_ref.internal` points
/// to a newly allocated `InternalState` owned by `*strm_ref` until freed by `lzma_end` or
/// replaced by another initialization function.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_raw_encoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_raw_encoder(lzma_stream* strm, const lzma_filter* filters)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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
        // SAFETY:
        // - Operation: Creating a shared reference `&*(f.options as *const lzma_options_lzma)`.
        // - Required contract: `f.options` must be non-null, properly aligned for
        //   `lzma_options_lzma`, and point to a live, initialized `lzma_options_lzma` not mutated
        //   for the duration of the reference.
        // - Evidence: `!f.options.is_null()` and `f.id` being `LZMA_FILTER_LZMA1` or
        //   `LZMA_FILTER_LZMA2` are checked above, and precondition 4 of this function's `# Safety`
        //   contract guarantees alignment, initialization, and immutability during this call.
        // - Postcondition: `options` is a valid shared reference `&lzma_options_lzma`.
        let options = unsafe { &*(f.options as *const lzma_options_lzma) };

        if let Err(e) = state::validate_encoder_options(options) {
            return e;
        }

        // SAFETY:
        // - Operation: `state::lzma_options_from_c(options)`.
        // - Required contract: When `options.preset_dict` is non-null and `options.preset_dict_size
        //   > 0`, `options.preset_dict` must point to at least `options.preset_dict_size as usize`
        //   initialized readable bytes in a single live allocation (`<= isize::MAX as usize`) not
        //   mutated during this call.
        // - Evidence: Guaranteed by precondition 4 of this function's `# Safety` contract, with no
        //   intervening mutation of `options`.
        // - Postcondition: Returns an owned `Result<LzmaOptions, lzma_ret>` without retaining raw
        //   pointers.
        match unsafe { state::lzma_options_from_c(options) } {
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

    // SAFETY:
    // - Operation: `helpers::install_state(strm, ...)`.
    // - Required contract: `strm` must be non-null, properly aligned, and point to a live,
    //   initialized, writable `lzma_stream` with exclusive access, and any non-null `strm.internal`
    //   must have been allocated by this crate via `Box::into_raw(Box::new(InternalState))` and not
    //   yet freed.
    // - Evidence: `strm` is a valid `&mut lzma_stream` by precondition 1, and precondition 2 of
    //   this function's `# Safety` contract guarantees the provenance and exclusive ownership of
    //   any non-null `strm.internal`.
    // - Postcondition: Any old `strm.internal` is freed, `strm.internal` stores the new
    //   heap-allocated `InternalState`, and `total_in`/`total_out` are reset to `0`.
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
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_lzip_encoder))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_lzip_encoder(lzma_stream* strm, uint32_t preset)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
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
