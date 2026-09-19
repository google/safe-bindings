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
/// `filters` must point to a valid array of `lzma_filter` terminated by a filter
/// with `id == LZMA_VLI_UNKNOWN` or at least `FILTERS_MAX + 1` length.
pub(crate) unsafe fn parse_filters<'a>(
    filters: *const lzma_filter,
) -> Result<&'a [lzma_filter], ()> {
    for i in 0..=FILTERS_MAX {
        // SAFETY: The caller guarantees `filters` points to a valid, continuous array of `lzma_filter`
        // terminated by `LZMA_VLI_UNKNOWN` within `FILTERS_MAX + 1` elements.
        let f = unsafe { &*filters.add(i) };
        if f.id == LZMA_VLI_UNKNOWN {
            // SAFETY: `filters` points to `i` initialized, valid, properly aligned `lzma_filter`
            // elements contained within a single allocated object, and no concurrent mutation occurs for lifetime 'a.
            return Ok(unsafe { std::slice::from_raw_parts(filters, i) });
        }
    }
    Err(())
}

/// Converts a C `lzma_filter` into a `FilterConfig`.
///
/// # Safety
/// If `f.options` is not null, it must point to a valid configuration struct matching `f.id`
/// (`lzma_options_delta` for DELTA filters, `lzma_options_bcj` for BCJ filters).
pub(crate) unsafe fn filter_config_from_c(f: &lzma_filter) -> Option<FilterConfig> {
    if f.id == LZMA_FILTER_DELTA.into() {
        let property = if f.options.is_null() {
            1
        } else {
            // SAFETY: The `options` pointer is valid if non-null, pointing to `lzma_options_delta`.
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
                // SAFETY: The `options` pointer is valid if non-null, pointing to `lzma_options_bcj`.
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

/// Helper to drop old state and safely install a new one.
///
/// # Safety
/// `strm` must be non-null and point to a valid `lzma_stream`. The caller must have exclusive
/// ownership to modify `strm.internal` without racing with other threads.
pub(crate) unsafe fn install_state(strm: *mut lzma_stream, state: InternalState) {
    // SAFETY: `strm` is non-null per caller promise. The caller guarantees that if
    // `(*strm).internal` is non-null, it was allocated by `Box::into_raw` with the same
    // type `InternalState`, making it safe to reconstruct and drop here.
    unsafe {
        if !(*strm).internal.is_null() {
            let ptr = core::mem::take(&mut (*strm).internal) as *mut InternalState;
            drop(Box::from_raw(ptr));
        }
        (*strm).internal = Box::into_raw(Box::new(state)) as *mut lzma_internal_s;
        (*strm).total_in = 0;
        (*strm).total_out = 0;
    }
}
