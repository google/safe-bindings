//! Core processing functions: lzma_code and lzma_end.

use std::io::ErrorKind;
use std::slice;

use lzma_rust2::Status;

use crate::helpers::{self, FfiAction};
use crate::state::{ActionState, CoderInner, CoderResult, InternalState};
use crate::types::{
    lzma_ret, lzma_stream, LZMA_BUF_ERROR, LZMA_CHECK_NONE, LZMA_GET_CHECK, LZMA_NO_CHECK, LZMA_OK,
    LZMA_OPTIONS_ERROR, LZMA_PROG_ERROR, LZMA_STREAM_END, LZMA_TELL_ANY_CHECK, LZMA_TELL_NO_CHECK,
};

pub(crate) struct StreamView<'a> {
    pub input: &'a [u8],
    pub output: &'a mut [u8],
}

impl<'a> StreamView<'a> {
    /// Constructs a safe `StreamView` from a raw `lzma_stream`.
    ///
    /// # Safety
    /// The caller must guarantee that if `avail_in > 0`, `next_in` points to a valid,
    /// continuous buffer of at least `avail_in` bytes of reading. Similarly, if `avail_out > 0`,
    /// `next_out` must point to a valid, continuous buffer of at least `avail_out` bytes
    /// of writing. The buffers must not overlap, and must not be read or written through any
    /// other pointer while the view is alive.
    pub unsafe fn try_from_c(strm_ref: &'a mut lzma_stream) -> Result<Self, lzma_ret> {
        if strm_ref.avail_in > 0 && strm_ref.next_in.is_null() {
            return Err(LZMA_PROG_ERROR);
        }
        if strm_ref.avail_out > 0 && strm_ref.next_out.is_null() {
            return Err(LZMA_PROG_ERROR);
        }
        if strm_ref.avail_in > isize::MAX as usize {
            return Err(LZMA_PROG_ERROR);
        }
        if strm_ref.avail_out > isize::MAX as usize {
            return Err(LZMA_PROG_ERROR);
        }
        if helpers::buffers_overlap(
            strm_ref.next_in,
            strm_ref.avail_in,
            strm_ref.next_out,
            strm_ref.avail_out,
        ) {
            return Err(LZMA_PROG_ERROR);
        }

        // Build input slice.
        // When avail_in == 0, we use an empty slice to avoid creating a
        // slice from a potentially-null pointer (which is UB even with len 0).
        let input = if strm_ref.avail_in == 0 {
            &[]
        } else {
            // SAFETY: next_in is non-null (checked above for avail_in > 0).
            // Caller guarantees next_in points to avail_in readable bytes.
            unsafe { slice::from_raw_parts(strm_ref.next_in, strm_ref.avail_in) }
        };

        // Build output slice.
        // No overlap with input (checked above).
        let output = if strm_ref.avail_out == 0 {
            &mut []
        } else {
            // SAFETY: next_out is non-null (checked above for avail_out > 0).
            // Caller guarantees next_out points to avail_out writable bytes.
            unsafe { slice::from_raw_parts_mut(strm_ref.next_out, strm_ref.avail_out) }
        };

        Ok(Self { input, output })
    }
}

impl<'a> StreamView<'a> {
    /// Advances the stream pointers.
    ///
    /// # Safety
    /// The caller must guarantee that `strm_ref.next_in` and `strm_ref.next_out`
    /// point to valid, continuous buffers of at least `strm_ref.avail_in`
    /// and `strm_ref.avail_out` bytes respectively.
    pub unsafe fn advance(
        strm_ref: &mut lzma_stream,
        mut bytes_consumed: usize,
        mut bytes_produced: usize,
    ) {
        // Saturate as a safety net in release builds.
        bytes_consumed = bytes_consumed.min(strm_ref.avail_in);
        bytes_produced = bytes_produced.min(strm_ref.avail_out);

        // Advance pointers only when > 0 to avoid NULL + 0 UB.
        if bytes_consumed > 0 {
            // SAFETY: `bytes_consumed` is capped to `avail_in` above. Therefore,
            // `bytes_consumed > 0` means `avail_in > 0`. By the caller's guarantee,
            // `next_in` points to a valid buffer of at least `avail_in` bytes, so it cannot
            // be null. The capping also ensures the addition remains in bounds.
            strm_ref.next_in = unsafe { strm_ref.next_in.add(bytes_consumed) };
            strm_ref.avail_in -= bytes_consumed;
            strm_ref.total_in = strm_ref.total_in.wrapping_add(bytes_consumed as u64);
        }
        if bytes_produced > 0 {
            // SAFETY: `bytes_produced` is capped to `avail_out` above. Therefore,
            // `bytes_produced > 0` means `avail_out > 0`. By the caller's guarantee,
            // `next_out` points to a valid buffer of at least `avail_out` bytes, so it cannot
            // be null. The capping also ensures the addition remains in bounds.
            strm_ref.next_out = unsafe { strm_ref.next_out.add(bytes_produced) };
            strm_ref.avail_out -= bytes_produced;
            strm_ref.total_out = strm_ref.total_out.wrapping_add(bytes_produced as u64);
        }
    }
}

/// Number of stream headers parsed so far by the `.xz` decoder behind `state`; 0 for any other
/// decoder.
fn xz_streams_started(state: &InternalState) -> usize {
    let coder = match &state.coder {
        CoderInner::AutoDecoder(auto) => match auto.inner() {
            Some(coder) => coder,
            None => return 0,
        },
        coder => coder,
    };
    match coder {
        CoderInner::XzDecoder(xz) => xz.streams_started(),
        _ => 0,
    }
}

fn check_report(state: &InternalState) -> Option<lzma_ret> {
    if state.tell_flags & LZMA_TELL_NO_CHECK != 0
        && crate::check::check_of(state) == LZMA_CHECK_NONE
    {
        Some(LZMA_NO_CHECK)
    } else if state.tell_flags & LZMA_TELL_ANY_CHECK != 0 {
        Some(LZMA_GET_CHECK)
    } else {
        None
    }
}

/// Inner logic for `lzma_code`.
///
/// # Safety
/// `strm_ref` must point to a valid, initialized `lzma_stream`. Buffer pointers
/// (`next_in`, `next_out`) must point to valid, continuous buffers of at least `avail_in`
/// and `avail_out` bytes respectively. Input and output buffers must not overlap.
unsafe fn lzma_code_inner(
    strm_ref: &mut lzma_stream,
    state: &mut InternalState,
    action_raw: u32,
) -> lzma_ret {
    let action = match helpers::action_from_c(action_raw) {
        Some(a) => a,
        None => return LZMA_PROG_ERROR,
    };

    if !strm_ref.reserved_ptr1.is_null()
        || !strm_ref.reserved_ptr2.is_null()
        || !strm_ref.reserved_ptr3.is_null()
        || !strm_ref.reserved_ptr4.is_null()
        || strm_ref.reserved_int2 != 0
        || strm_ref.reserved_int3 != 0
        || strm_ref.reserved_int4 != 0
        || strm_ref.reserved_enum1 != 0
        || strm_ref.reserved_enum2 != 0
    {
        return LZMA_OPTIONS_ERROR;
    }

    match state.action_state {
        ActionState::End => return LZMA_STREAM_END,
        ActionState::Error => return LZMA_PROG_ERROR,
        ActionState::Finish { saved_avail_in } => {
            if action != FfiAction::Finish || strm_ref.avail_in != saved_avail_in {
                return LZMA_PROG_ERROR;
            }
        }
        ActionState::Run => {
            if action == FfiAction::Finish {
                state.action_state = ActionState::Finish { saved_avail_in: strm_ref.avail_in };
            }
        }
    }

    // SAFETY: The caller guarantees `strm_ref` contains valid, non-overlapping
    // buffer pointers for their stated lengths.
    if let Err(e) = unsafe { StreamView::try_from_c(&mut *strm_ref) } {
        return e;
    }

    if !state.coder.is_action_supported(action) {
        return LZMA_PROG_ERROR;
    }

    if state.tell_flags != 0 && strm_ref.avail_in > 0 {
        let detection_pending = match &state.coder {
            CoderInner::AutoDecoder(auto) => auto.inner().is_none(),
            _ => false,
        };
        // SAFETY: The caller guarantees `strm_ref` contains valid, non-overlapping
        // buffer pointers for their stated lengths.
        let first_byte = match unsafe { StreamView::try_from_c(&mut *strm_ref) } {
            Ok(buffers) => buffers.input[0],
            Err(e) => return e,
        };
        if detection_pending && first_byte != 0xFD {
            let ret = if state.tell_flags & LZMA_TELL_NO_CHECK != 0 {
                LZMA_NO_CHECK
            } else {
                LZMA_GET_CHECK
            };
            state.tell_flags = 0;
            state.allow_buf_error = false;
            if let ActionState::Finish { .. } = state.action_state {
                state.action_state = ActionState::Finish { saved_avail_in: strm_ref.avail_in };
            }
            return ret;
        }
    }

    // With a check report requested, the `.xz` decoder returns at every stream header, but
    // liblzma only returns to the caller when it needs to report something.
    // To match liblzma, we keep calling `process()` in a loop if lzma_rust2 stops at a stream
    // header and there is no need to report the check type.

    let mut total_consumed = 0;
    let mut total_produced = 0;
    let coder_result = loop {
        let streams_started = xz_streams_started(state);
        // The crate reports running out of input under `Finish` as a permanent `UnexpectedEof`;
        // liblzma just reports no progress and lets the BUF_ERROR two-strike logic complain.
        // Mirror that by never calling the crate again and returning zero progress.
        let mut coder_result = if state.swallowed_unexpected_eof {
            CoderResult { bytes_consumed: 0, bytes_produced: 0, result: Ok(Status::Ok) }
        } else {
            // SAFETY: On the first iteration, `strm_ref` contains valid, non-overlapping buffer
            // pointers for their stated lengths by the caller's contract and checks at function
            // entry. On subsequent loop iterations, `StreamView::advance` in the
            // previous iteration advanced `next_in` and `next_out` within their
            // original allocations by at most `avail_in` and `avail_out`, leaving the
            // remaining slices contiguous, valid, and non-overlapping.

            let buffers = match unsafe { StreamView::try_from_c(&mut *strm_ref) } {
                Ok(buf) => buf,
                Err(e) => return e,
            };
            state.coder.process(buffers.input, buffers.output, action)
        };

        if let Err(ref e) = coder_result.result {
            if e.kind() == ErrorKind::UnexpectedEof {
                state.swallowed_unexpected_eof = true;
                coder_result.result = Ok(Status::Ok);
            }
        }

        let bytes_consumed = coder_result.bytes_consumed;
        let bytes_produced = coder_result.bytes_produced;

        debug_assert!(
            bytes_consumed <= strm_ref.avail_in,
            "process() returned bytes_consumed ({}) > avail_in ({})",
            bytes_consumed,
            strm_ref.avail_in
        );
        debug_assert!(
            bytes_produced <= strm_ref.avail_out,
            "process() returned bytes_produced ({}) > avail_out ({})",
            bytes_produced,
            strm_ref.avail_out
        );

        // SAFETY: By the caller's contract and the loop invariant, `next_in` and `next_out`
        // point to valid, continuous buffers of at least `avail_in` and `avail_out` bytes
        // respectively. `advance` internally clamps `bytes_consumed` to `avail_in` and
        // `bytes_produced` to `avail_out`, ensuring that pointer arithmetic remains in bounds
        // of the original allocation.

        unsafe { StreamView::advance(strm_ref, bytes_consumed, bytes_produced) };
        total_consumed += bytes_consumed;
        total_produced += bytes_produced;

        let stopped_at_header = state.tell_flags != 0
            && matches!(coder_result.result, Ok(Status::Ok))
            && xz_streams_started(state) != streams_started;
        if !stopped_at_header {
            break coder_result;
        }
        if let Some(ret) = check_report(state) {
            state.allow_buf_error = false;
            if let ActionState::Finish { .. } = state.action_state {
                state.action_state = ActionState::Finish { saved_avail_in: strm_ref.avail_in };
            }
            return ret;
        }
        // Nothing to report and no input left: liblzma returns LZMA_OK here.
        if strm_ref.avail_in == 0 {
            break coder_result;
        }
    };

    match coder_result.result {
        Ok(status) => {
            if let ActionState::Finish { .. } = state.action_state {
                state.action_state = ActionState::Finish { saved_avail_in: strm_ref.avail_in };
            }

            match status {
                Status::Ok => {
                    // A flush is complete when the encoder took everything and had output
                    // space left over, i.e. nothing is being held back.
                    if matches!(action, FfiAction::FullFlush | FfiAction::SyncFlush)
                        && strm_ref.avail_in == 0
                        && strm_ref.avail_out > 0
                    {
                        state.allow_buf_error = false;
                        state.action_state = ActionState::Run;
                        return LZMA_STREAM_END;
                    }

                    if total_consumed == 0 && total_produced == 0 {
                        if state.allow_buf_error {
                            return LZMA_BUF_ERROR;
                        } else {
                            state.allow_buf_error = true;
                        }
                    } else {
                        state.allow_buf_error = false;
                    }
                    LZMA_OK
                }
                Status::StreamEnd => {
                    state.allow_buf_error = false;
                    state.action_state = ActionState::End;
                    LZMA_STREAM_END
                }
            }
        }
        Err(e) => {
            state.allow_buf_error = false;
            // NOTE(b/563249135): liblzma keeps reporting LZMA_MEMLIMIT_ERROR
            // instead of failing the stream.
            state.action_state = ActionState::Error;
            helpers::map_rust_error(e)
        }
    }
}

/// Encode or decode data.
///
/// # Safety
/// `strm` must be null or point to a valid, initialized `lzma_stream`.
/// If `strm` is not null and `strm.internal` is not null, `strm.internal` must point
/// to a valid state initialized by a corresponding initialization function and must not
/// have been modified or freed since.
/// Buffer pointers (`next_in`, `next_out`) must point to valid, continuous buffers
/// of at least `avail_in` and `avail_out` bytes respectively. Input and output
/// buffers must not overlap. The caller must have exclusive ownership of the struct.
#[unsafe(export_name = crate::prefix!(lzma_code))]
pub unsafe extern "C" fn lzma_code(strm: *mut lzma_stream, action: u32) -> lzma_ret {
    if strm.is_null() {
        return LZMA_PROG_ERROR;
    }

    // SAFETY: strm is checked for null, and the caller guarantees it points to a valid lzma_stream.
    let strm_ref = unsafe { &mut *strm };

    let internal = strm_ref.internal as *mut InternalState;
    if internal.is_null() {
        return LZMA_PROG_ERROR;
    }
    // SAFETY: internal is non-null and was created by Box::into_raw in
    // a decoder init function. We have exclusive access (no concurrent
    // calls. This is documented as caller's responsibility, matching liblzma).
    let state = unsafe { &mut *internal };

    // SAFETY: The caller guarantees `strm` points to a valid `lzma_stream`
    // with valid buffer pointers for their stated lengths, and buffers that do not overlap.
    unsafe { lzma_code_inner(strm_ref, state, action) }
}

/// Free decoder memory. Sets `strm->internal` to NULL.
///
/// Safe to call with a NULL `strm` or after a previous `lzma_end()`.
///
/// # Safety
/// `strm` must be null or point to a valid, writable `lzma_stream`.
/// If `strm` is not null and `strm.internal` is not null, `strm.internal` must point
/// to a valid state initialized by a corresponding initialization function and must not
/// have been modified or freed since.
/// The caller must have exclusive ownership of the struct (no concurrent aliasing).
#[unsafe(export_name = crate::prefix!(lzma_end))]
pub unsafe extern "C" fn lzma_end(strm: *mut lzma_stream) {
    if strm.is_null() {
        return;
    }
    // SAFETY: Safe to dereference because `strm` is non-null (checked above),
    // a valid `lzma_stream`, and the caller guarantees exclusive access.
    unsafe {
        if !(*strm).internal.is_null() {
            // SAFETY: `(*strm).internal` is guaranteed to have been allocated by
            // `Box::into_raw` with the same type `InternalState` in an init function,
            // making it safe to reconstruct and drop. We null it immediately after
            // to prevent double-free.
            drop(Box::from_raw((*strm).internal as *mut InternalState));
            (*strm).internal = core::ptr::null_mut();
        }
    }
}
