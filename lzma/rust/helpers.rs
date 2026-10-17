//! Safety helpers for FFI boundary validation.

use crate::state::InternalState;
use crate::types::{
    lzma_filter, lzma_internal_s, lzma_options_bcj, lzma_options_delta, lzma_ret, lzma_stream,
    LZMA_DATA_ERROR, LZMA_FILTER_ARM, LZMA_FILTER_ARM64, LZMA_FILTER_ARMTHUMB, LZMA_FILTER_DELTA,
    LZMA_FILTER_IA64, LZMA_FILTER_POWERPC, LZMA_FILTER_RISCV, LZMA_FILTER_SPARC, LZMA_FILTER_X86,
    LZMA_FORMAT_ERROR, LZMA_MEMLIMIT_ERROR, LZMA_MEM_ERROR, LZMA_MF_BT4, LZMA_MF_HC4,
    LZMA_MODE_FAST, LZMA_MODE_NORMAL, LZMA_OPTIONS_ERROR, LZMA_PROG_ERROR,
};

use lzma_rust2::{Action, EncodeMode, FilterConfig, MfType};
use std::io::{self, ErrorKind};

pub(crate) const LZMA_VLI_UNKNOWN: u64 = u64::MAX;
pub(crate) const FILTERS_MAX: usize = 4;

/// Safely extracts a slice of `lzma_filter` up to `LZMA_VLI_UNKNOWN` terminator
/// or a defined maximum size.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `filters` is non-null, properly aligned for `lzma_filter`, and points to a contiguous
///    sequence of initialized `lzma_filter` elements within a single live allocated object.
/// 2. The allocated sequence either contains a terminator element with `id == LZMA_VLI_UNKNOWN` at
///    some index `i <= FILTERS_MAX` (with all elements `0..=i` initialized and valid for reads), or
///    contains at least `FILTERS_MAX + 1` (5) initialized `lzma_filter` elements valid for reads.
/// 3. For the entire lifetime `'a` of the returned slice, no element in the accessed range is
///    mutated through any pointer or reference.
///
/// If these preconditions hold and this function returns `Ok(slice)`, `slice` is a valid shared
/// slice `&'a [lzma_filter]` of length `<= FILTERS_MAX` containing all filters preceding the
/// first `LZMA_VLI_UNKNOWN` terminator.
pub(crate) unsafe fn parse_filters<'a>(
    filters: *const lzma_filter,
) -> Result<&'a [lzma_filter], ()> {
    for i in 0..=FILTERS_MAX {
        // SAFETY:
        // - Operation: `filters.add(i)`.
        // - Required contract: `filters` and the resulting pointer must lie within (or one past the
        //   end of) the same allocated object, and `i * size_of::<lzma_filter>()` must not exceed
        //   `isize::MAX as usize`.
        // - Evidence: `i <= FILTERS_MAX == 4`, so the byte offset is at most `4 *
        //   size_of::<lzma_filter>() <= isize::MAX as usize`. No terminator was found at any index
        //   `< i`, so by preconditions 1 and 2 of this function's `# Safety` contract, the
        //   allocation contains at least `i + 1` elements in a single live allocated object.
        // - Postcondition: `elem_ptr` is a valid, in-bounds pointer to the `i`-th `lzma_filter`.
        let elem_ptr = unsafe { filters.add(i) };
        // SAFETY:
        // - Operation: Creating a shared reference `&*elem_ptr` of type `&lzma_filter`.
        // - Required contract: `elem_ptr` must be non-null, properly aligned for `lzma_filter`, and
        //   point to a live, initialized `lzma_filter` not mutated during the borrow.
        // - Evidence: `elem_ptr` is in-bounds of the aligned `filters` array as established by
        //   `filters.add(i)` above, and by preconditions 2 and 3 of this function's `# Safety`
        //   contract, element `i` is initialized, valid for reads, and not mutated during this
        //   call.
        // - Postcondition: `f` borrows the `i`-th `lzma_filter` element for this iteration.
        let f = unsafe { &*elem_ptr };
        if f.id == LZMA_VLI_UNKNOWN {
            // SAFETY:
            // - Operation: `std::slice::from_raw_parts(filters, i)`.
            // - Required contract: `filters` must be non-null, properly aligned for `lzma_filter`,
            //   and point to `i` consecutive initialized `lzma_filter` values within a single
            //   allocated object of size `<= isize::MAX as usize` that is not mutated for lifetime
            //   `'a`.
            // - Evidence: `i <= FILTERS_MAX == 4`, so `i * size_of::<lzma_filter>() <= isize::MAX
            //   as usize`. By preconditions 1–3 of this function's `# Safety` contract and the
            //   preceding loop iterations, `filters` is non-null, aligned for `lzma_filter`, points
            //   to `i` initialized `lzma_filter` elements within a single live allocation, and is
            //   not mutated for lifetime `'a`.
            // - Postcondition: Returns a valid shared slice `&'a [lzma_filter]` of length `i`.
            return Ok(unsafe { std::slice::from_raw_parts(filters, i) });
        }
    }
    Err(())
}

/// Converts a C `lzma_filter` into a `FilterConfig`.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. When `f.id == u64::from(LZMA_FILTER_DELTA)` and `!f.options.is_null()`, `f.options` is
///    properly aligned for `lzma_options_delta` and points to a live, initialized
///    `lzma_options_delta` valid for reads and not mutated for the duration of this call.
/// 2. When `f.id` is one of the supported BCJ filter IDs (`LZMA_FILTER_X86`, `LZMA_FILTER_POWERPC`,
///    `LZMA_FILTER_IA64`, `LZMA_FILTER_ARM`, `LZMA_FILTER_ARMTHUMB`, `LZMA_FILTER_SPARC`,
///    `LZMA_FILTER_ARM64`, or `LZMA_FILTER_RISCV`) and `!f.options.is_null()`, `f.options` is
///    properly aligned for `lzma_options_bcj` and points to a live, initialized `lzma_options_bcj`
///    valid for reads and not mutated for the duration of this call.
///
/// If these preconditions hold, this function reads the filter options without retaining any
/// pointer and returns the corresponding `FilterConfig` (or `None` if `f.id` is not a Delta
/// or BCJ filter).
pub(crate) unsafe fn filter_config_from_c(f: &lzma_filter) -> Option<FilterConfig> {
    if f.id == LZMA_FILTER_DELTA.into() {
        let property = if f.options.is_null() {
            1
        } else {
            // SAFETY:
            // - Operation: Creating a shared reference `&*(f.options as *const
            //   lzma_options_delta)`.
            // - Required contract: `f.options` must be non-null, properly aligned for
            //   `lzma_options_delta`, and point to a live, initialized `lzma_options_delta` not
            //   mutated for the duration of the reference.
            // - Evidence: `!f.options.is_null()` and `f.id == u64::from(LZMA_FILTER_DELTA)` hold in
            //   this branch. By precondition 1 of this function's `# Safety` contract, `f.options`
            //   is aligned, initialized, and not mutated during this call.
            // - Postcondition: Reads `lzma_opts.dist` by value without retaining the reference.
            let lzma_opts = unsafe { &*(f.options as *const lzma_options_delta) };
            lzma_opts.dist
        };
        Some(FilterConfig::new_delta(property))
    } else {
        let constructor: Option<fn(u32) -> FilterConfig> = match f.id {
            id if id == LZMA_FILTER_X86.into() => Some(FilterConfig::new_bcj_x86),
            id if id == LZMA_FILTER_POWERPC.into() => Some(FilterConfig::new_bcj_ppc),
            id if id == LZMA_FILTER_IA64.into() => Some(FilterConfig::new_bcj_ia64),
            id if id == LZMA_FILTER_ARM.into() => Some(FilterConfig::new_bcj_arm),
            id if id == LZMA_FILTER_ARMTHUMB.into() => Some(FilterConfig::new_bcj_arm_thumb),
            id if id == LZMA_FILTER_SPARC.into() => Some(FilterConfig::new_bcj_sparc),
            id if id == LZMA_FILTER_ARM64.into() => Some(FilterConfig::new_bcj_arm64),
            id if id == LZMA_FILTER_RISCV.into() => Some(FilterConfig::new_bcj_risc_v),
            _ => None,
        };

        if let Some(make_filter) = constructor {
            let start_offset = if !f.options.is_null() {
                // SAFETY:
                // - Operation: Creating a shared reference `&*(f.options as *const
                //   lzma_options_bcj)`.
                // - Required contract: `f.options` must be non-null, properly aligned for
                //   `lzma_options_bcj`, and point to a live, initialized `lzma_options_bcj` not
                //   mutated for the duration of the reference.
                // - Evidence: `!f.options.is_null()` and `f.id` matching one of the supported BCJ
                //   filter IDs hold in this branch. By precondition 2 of this function's `# Safety`
                //   contract, `f.options` is aligned, initialized, and not mutated during this
                //   call.
                // - Postcondition: Reads `bcj_opts.start_offset` by value without retaining the
                //   reference.
                let bcj_opts = unsafe { &*(f.options as *const lzma_options_bcj) };
                bcj_opts.start_offset
            } else {
                0
            };
            Some(make_filter(start_offset))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FfiAction {
    Run,
    SyncFlush,
    FullFlush,
    Finish,
}

impl FfiAction {
    pub(crate) fn as_lzma_rust2_decoder_action(self) -> Action {
        match self {
            FfiAction::Run => Action::Run,
            FfiAction::SyncFlush => Action::Run, // Decoders don't flush -> map to Run.
            FfiAction::FullFlush => Action::Run, // Decoders don't flush -> map to Run.
            FfiAction::Finish => Action::Finish,
        }
    }
}

/// Convert a raw `u32` action value to our internal `FfiAction` enum.
pub(crate) fn action_from_c(raw: u32) -> Option<FfiAction> {
    match raw {
        0 => Some(FfiAction::Run),       // LZMA_RUN
        1 => Some(FfiAction::SyncFlush), // LZMA_SYNC_FLUSH
        2 => Some(FfiAction::FullFlush), // LZMA_FULL_FLUSH
        3 => Some(FfiAction::Finish),    // LZMA_FINISH
        _ => None,                       // FULL_BARRIER or garbage
    }
}

pub(crate) fn mode_from_c(raw: u32) -> Result<EncodeMode, lzma_ret> {
    match raw {
        LZMA_MODE_FAST => Ok(EncodeMode::Fast),
        LZMA_MODE_NORMAL => Ok(EncodeMode::Normal),
        _ => Err(LZMA_PROG_ERROR),
    }
}

pub(crate) fn mf_from_c(raw: u32) -> Result<MfType, lzma_ret> {
    match raw {
        LZMA_MF_HC4 => Ok(MfType::Hc4),
        LZMA_MF_BT4 => Ok(MfType::Bt4),
        _ => Err(LZMA_PROG_ERROR),
    }
}

pub(crate) fn mode_to_c(mode: EncodeMode) -> u32 {
    match mode {
        EncodeMode::Fast => LZMA_MODE_FAST,
        EncodeMode::Normal => LZMA_MODE_NORMAL,
    }
}

pub(crate) fn mf_to_c(mf: MfType) -> u32 {
    match mf {
        MfType::Hc4 => LZMA_MF_HC4,
        MfType::Bt4 => LZMA_MF_BT4,
    }
}

/// Check whether the input and output buffer regions overlap.
/// Overlapping `&[u8]` and `&mut [u8]` is instant UB in Rust.
///
/// # Safety-usable invariant
///
/// If this function returns `false`, then:
/// 1. Neither `in_ptr.addr().overflowing_add(in_len)` nor `out_ptr.addr().overflowing_add(out_len)`
///    overflows `usize`.
/// 2. Either `in_len == 0`, `out_len == 0`, or the byte address ranges `[in_ptr.addr(),
///    in_ptr.addr() + in_len)` and `[out_ptr.addr(), out_ptr.addr() + out_len)` are strictly
///    disjoint.
pub(crate) fn buffers_overlap(
    in_ptr: *const u8,
    in_len: usize,
    out_ptr: *mut u8,
    out_len: usize,
) -> bool {
    let in_start = in_ptr.addr();
    let (in_end, in_overflow) = in_start.overflowing_add(in_len);
    let out_start = out_ptr.addr();
    let (out_end, out_overflow) = out_start.overflowing_add(out_len);

    if in_overflow || out_overflow {
        return true;
    }

    if in_len == 0 || out_len == 0 {
        return false;
    }

    in_start < out_end && out_start < in_end
}

/// Map a Rust `std::io::Error` to the appropriate `lzma_ret` value.
pub(crate) fn map_rust_error(e: io::Error) -> lzma_ret {
    match e.kind() {
        ErrorKind::InvalidData => {
            // We probably have to rely on this string match.
            // As InvalidData is already a reasonable error type for an invalid stream,
            // making an upstream PR is hard to justify.
            let msg = e.to_string();
            // According to liblzma, a header magic error is a format error, but a footer
            // error is a data error. The upstream lzma_rust2 crate returns "invalid XZ magic bytes"
            // for the header and "invalid XZ footer magic bytes" for the footer.
            // Since the word "header" is omitted from the header error string, we identify it
            // by checking for the absence of "footer".
            let is_header_magic_error = msg.contains("magic") && !msg.contains("footer");
            if is_header_magic_error {
                LZMA_FORMAT_ERROR
            } else if msg.contains("unsupported LZIP version") {
                // liblzma reports an unsupported `.lz` version as an options error.
                LZMA_OPTIONS_ERROR
            } else {
                LZMA_DATA_ERROR
            }
        }
        ErrorKind::UnexpectedEof => LZMA_DATA_ERROR,
        ErrorKind::InvalidInput => LZMA_DATA_ERROR,
        ErrorKind::OutOfMemory => {
            if e.to_string().contains("mem_limit_kb") {
                LZMA_MEMLIMIT_ERROR
            } else {
                LZMA_MEM_ERROR
            }
        }

        ErrorKind::Unsupported => LZMA_OPTIONS_ERROR,
        ErrorKind::Other => LZMA_DATA_ERROR,
        _ => LZMA_PROG_ERROR,
    }
}

/// Frees and clears `strm_ref.internal` if it is non-null.
///
/// # Safety
///
/// The caller must ensure that if `strm_ref.internal` is non-null, it points to a live
/// `InternalState` previously allocated by this crate via `Box::into_raw(Box::new(InternalState))`
/// and not yet freed, and that no other pointer or reference accesses `*strm_ref.internal`
/// concurrently with or after this call (exclusive ownership is transferred to this call).
///
/// If this precondition holds, any non-null `strm_ref.internal` allocation is dropped and
/// deallocated via `Box::from_raw`, and `strm_ref.internal` is set to null.
pub(crate) unsafe fn free_internal_state(strm_ref: &mut lzma_stream) {
    if !strm_ref.internal.is_null() {
        let ptr = core::mem::take(&mut strm_ref.internal) as *mut InternalState;
        // SAFETY:
        // - Operation: `Box::from_raw(ptr)`.
        // - Required contract: `ptr` must be non-null and have been allocated via the global
        //   allocator with `Layout::new::<InternalState>()` (via `Box::into_raw`), and no other
        //   owner or alias may access or free `ptr` after ownership is reconstructed.
        // - Evidence: `ptr` is non-null after the `!strm_ref.internal.is_null()` check above,
        //   `strm_ref.internal` has already been replaced with null via `core::mem::take`, and this
        //   function's `# Safety` precondition guarantees that a non-null `strm_ref.internal` was
        //   allocated by this crate via `Box::into_raw(Box::new(InternalState))` and transfers
        //   exclusive ownership to this call.
        // - Postcondition: The `Box<InternalState>` is reconstructed and immediately dropped,
        //   deallocating the state while `strm_ref.internal` is null.
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Helper to drop old state and safely install a new one.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is non-null, properly aligned for `lzma_stream`, and points to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access for the duration of this
///    call.
/// 2. If `(*strm).internal` is non-null, it points to a live `InternalState` previously allocated
///    by this crate via `Box::into_raw(Box::new(InternalState))` and not yet freed, with exclusive
///    ownership transferred to this call so it can be deallocated.
///
/// If these preconditions hold, any previous `(*strm).internal` allocation is dropped and
/// deallocated, `(*strm).internal` is set to a newly heap-allocated `InternalState` created via
/// `Box::into_raw(Box::new(state))`, and `(*strm).total_in` and `(*strm).total_out` are reset
/// to `0`.
pub(crate) unsafe fn install_state(strm: *mut lzma_stream, state: InternalState) {
    // SAFETY:
    // - Operation: Creating an exclusive reference `&mut *strm` of type `&mut lzma_stream`.
    // - Required contract: `strm` must be non-null, properly aligned for `lzma_stream`, point to a
    //   live initialized `lzma_stream`, and have exclusive access for the duration of this call.
    // - Evidence: Guaranteed directly by precondition 1 of this function's `# Safety` contract.
    // - Postcondition: `strm_ref` is a valid exclusive reference `&mut lzma_stream` for this call.
    let strm_ref = unsafe { &mut *strm };
    // SAFETY:
    // - Operation: `free_internal_state(strm_ref)`.
    // - Required contract: If `strm_ref.internal` is non-null, it must point to a live
    //   `InternalState` allocated by this crate via `Box::into_raw(Box::new(InternalState))` and
    //   transfer exclusive ownership to this call.
    // - Evidence: Guaranteed by precondition 2 of this function's `# Safety` contract, with no
    //   intervening mutation of `strm_ref.internal`.
    // - Postcondition: Any previous `strm_ref.internal` allocation is deallocated and
    //   `strm_ref.internal` is set to null.
    unsafe { free_internal_state(strm_ref) };
    strm_ref.internal = Box::into_raw(Box::new(state)) as *mut lzma_internal_s;
    strm_ref.total_in = 0;
    strm_ref.total_out = 0;
}
