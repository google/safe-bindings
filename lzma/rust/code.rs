//! Core processing functions: lzma_code and lzma_end.

use std::io::ErrorKind;
use std::slice;

use lzma_rust2::Status;

use crate::helpers::{self, FfiAction};
use crate::state::{ActionState, CoderInner, InternalState};
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
    ///
    /// The caller must ensure that:
    /// 1. If `strm_ref.avail_in > 0` and `!strm_ref.next_in.is_null()`, `strm_ref.next_in` is
    ///    properly aligned for `u8` (alignment 1) and points to at least `strm_ref.avail_in`
    ///    consecutive initialized `u8` values within a single live allocated object that is valid
    ///    for reads.
    /// 2. If `strm_ref.avail_out > 0` and `!strm_ref.next_out.is_null()`, `strm_ref.next_out` is
    ///    properly aligned for `u8` (alignment 1) and points to at least `strm_ref.avail_out`
    ///    consecutive writable bytes within a single live allocated object.
    /// 3. For the entire lifetime `'a` of the returned `StreamView<'a>`, the input byte range
    ///    `[next_in, next_in + avail_in)` is not mutated through any pointer or reference, and the
    ///    output byte range `[next_out, next_out + avail_out)` is not read or written through any
    ///    other pointer or reference.
    ///
    /// If these preconditions hold and this function returns `Ok(view)`, `view.input` and
    /// `view.output` are valid, non-overlapping slices of lengths `strm_ref.avail_in` and
    /// `strm_ref.avail_out`, with `strm_ref.avail_in <= isize::MAX as usize` and
    /// `strm_ref.avail_out <= isize::MAX as usize`.
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
            // SAFETY:
            // - Operation: `slice::from_raw_parts(strm_ref.next_in, strm_ref.avail_in)`.
            // - Required contract: `strm_ref.next_in` must be non-null, aligned for `u8`, and point
            //   to `strm_ref.avail_in` consecutive initialized `u8` values within a single
            //   allocated object of size `<= isize::MAX as usize` that is not mutated for lifetime
            //   `'a`.
            // - Evidence:
            //   - `strm_ref.avail_in > 0`, `!strm_ref.next_in.is_null()`, and `strm_ref.avail_in <=
            //     isize::MAX as usize` are verified by the runtime checks above, with no
            //     intervening mutation of `strm_ref`.
            //   - Alignment for `u8` is 1 by the Rust Reference layout rules.
            //   - By preconditions 1 and 3 of this function's `# Safety` contract,
            //     `strm_ref.next_in` points to `strm_ref.avail_in` initialized readable bytes in a
            //     single live allocation that is not mutated for lifetime `'a`.
            // - Postcondition: `input` is a valid shared slice `&'a [u8]` of length
            //   `strm_ref.avail_in`.
            unsafe { slice::from_raw_parts(strm_ref.next_in, strm_ref.avail_in) }
        };

        // Build output slice.
        // No overlap with input (checked above).
        let output = if strm_ref.avail_out == 0 {
            &mut []
        } else {
            // SAFETY:
            // - Operation: `slice::from_raw_parts_mut(strm_ref.next_out, strm_ref.avail_out)`.
            // - Required contract: `strm_ref.next_out` must be non-null, aligned for `u8`, and
            //   point to `strm_ref.avail_out` consecutive writable bytes within a single allocated
            //   object of size `<= isize::MAX as usize` that is not aliased by any other pointer or
            //   reference (including `input`) for lifetime `'a`. Although
            //   `slice::from_raw_parts_mut` formally documents an initialization requirement, C
            //   callers of `liblzma` pass uninitialized output buffers (`next_out`) that coders
            //   only write to; forming `&mut [u8]` over uninitialized write-only output memory
            //   without reading uninitialized bytes is relied upon for FFI output buffers under
            //   UCG#414.
            // - Evidence:
            //   - `strm_ref.avail_out > 0`, `!strm_ref.next_out.is_null()`, and `strm_ref.avail_out
            //     <= isize::MAX as usize` are verified by the runtime checks above, with no
            //     intervening mutation of `strm_ref`.
            //   - Disjointness from `input` is guaranteed by the `!helpers::buffers_overlap(...)`
            //     check above and the safety-usable invariant of `helpers::buffers_overlap`.
            //   - Alignment for `u8` is 1 by the Rust Reference layout rules.
            //   - By preconditions 2 and 3 of this function's `# Safety` contract,
            //     `strm_ref.next_out` points to `strm_ref.avail_out` writable bytes in a single
            //     live allocation that is not otherwise accessed for lifetime `'a`, and downstream
            //     coders only write to `output` without reading uninitialized bytes.
            // - Postcondition: `output` is a valid exclusive slice `&'a mut [u8]` of length
            //   `strm_ref.avail_out` disjoint from `input`.
            unsafe { slice::from_raw_parts_mut(strm_ref.next_out, strm_ref.avail_out) }
        };

        Ok(Self { input, output })
    }
}

impl<'a> StreamView<'a> {
    /// Advances the stream pointers.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// 1. If `strm_ref.avail_in > 0` and `bytes_consumed > 0`, `strm_ref.next_in` is non-null and
    ///    points to a live allocated object of at least `strm_ref.avail_in` bytes, with
    ///    `strm_ref.avail_in <= isize::MAX as usize`.
    /// 2. If `strm_ref.avail_out > 0` and `bytes_produced > 0`, `strm_ref.next_out` is non-null and
    ///    points to a live allocated object of at least `strm_ref.avail_out` bytes, with
    ///    `strm_ref.avail_out <= isize::MAX as usize`.
    ///
    /// If these preconditions hold, `strm_ref.next_in` and `strm_ref.next_out` remain within or
    /// one-past-the-end of their respective allocations, and `avail_in`/`avail_out` are decremented
    /// by the advanced byte counts so that the buffer validity invariant is preserved for the
    /// remaining lengths.
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
            // SAFETY:
            // - Operation: `strm_ref.next_in.add(bytes_consumed)`.
            // - Required contract: Both `strm_ref.next_in` and the resulting pointer must be in
            //   bounds of (or one byte past the end of) the same allocated object, and
            //   `bytes_consumed` (in bytes, since `size_of::<u8>() == 1`) must not exceed
            //   `isize::MAX as usize` or wrap around the address space.
            // - Evidence: `bytes_consumed` was clamped to `strm_ref.avail_in` above and is `> 0` in
            //   this branch, so `0 < bytes_consumed <= strm_ref.avail_in`. By precondition 1 of
            //   this function's `# Safety` contract, `strm_ref.next_in` points to a live allocation
            //   of at least `strm_ref.avail_in` bytes with `strm_ref.avail_in <= isize::MAX as
            //   usize`.
            // - Postcondition: `strm_ref.next_in` is advanced by `bytes_consumed` bytes and remains
            //   within (or one-past-the-end of) the same allocation, while `strm_ref.avail_in` is
            //   reduced by `bytes_consumed`.
            strm_ref.next_in = unsafe { strm_ref.next_in.add(bytes_consumed) };
            strm_ref.avail_in -= bytes_consumed;
            strm_ref.total_in = strm_ref.total_in.wrapping_add(bytes_consumed as u64);
        }
        if bytes_produced > 0 {
            // SAFETY:
            // - Operation: `strm_ref.next_out.add(bytes_produced)`.
            // - Required contract: Both `strm_ref.next_out` and the resulting pointer must be in
            //   bounds of (or one byte past the end of) the same allocated object, and
            //   `bytes_produced` (in bytes, since `size_of::<u8>() == 1`) must not exceed
            //   `isize::MAX as usize` or wrap around the address space.
            // - Evidence: `bytes_produced` was clamped to `strm_ref.avail_out` above and is `> 0`
            //   in this branch, so `0 < bytes_produced <= strm_ref.avail_out`. By precondition 2 of
            //   this function's `# Safety` contract, `strm_ref.next_out` points to a live
            //   allocation of at least `strm_ref.avail_out` bytes with `strm_ref.avail_out <=
            //   isize::MAX as usize`.
            // - Postcondition: `strm_ref.next_out` is advanced by `bytes_produced` bytes and
            //   remains within (or one-past-the-end of) the same allocation, while
            //   `strm_ref.avail_out` is reduced by `bytes_produced`.
            strm_ref.next_out = unsafe { strm_ref.next_out.add(bytes_produced) };
            strm_ref.avail_out -= bytes_produced;
            strm_ref.total_out = strm_ref.total_out.wrapping_add(bytes_produced as u64);
        }
    }
}

const LZMA_STREAM_HEADER_SIZE: usize = 12;

/// Inner logic for `lzma_code`.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. If `strm_ref.avail_in > 0` and `!strm_ref.next_in.is_null()`, `strm_ref.next_in` points to at
///    least `strm_ref.avail_in` consecutive initialized readable `u8` bytes within a single live
///    allocated object.
/// 2. If `strm_ref.avail_out > 0` and `!strm_ref.next_out.is_null()`, `strm_ref.next_out` points to
///    at least `strm_ref.avail_out` consecutive writable bytes within a single live allocated
///    object.
/// 3. For the duration of this call, the input buffer `[next_in, next_in + avail_in)` is not
///    mutated through any other pointer or reference, and the output buffer `[next_out, next_out +
///    avail_out)` is not read or written through any other pointer or reference.
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

    let total_in = strm_ref.total_in as usize;

    // SAFETY:
    // - Operation: `StreamView::try_from_c(&mut *strm_ref)`.
    // - Required contract: Non-null `next_in`/`next_out` with non-zero `avail_in`/`avail_out` must
    //   point to single-allocation buffers valid for reads (initialized `u8` for `next_in`) and
    //   writes (`next_out`) and unaliased for the view's lifetime.
    // - Evidence: Guaranteed by preconditions 1–3 of `lzma_code_inner`'s `# Safety` contract, with
    //   no intervening mutation of `strm_ref`.
    // - Postcondition: `buffers` borrows `strm_ref` for the initial validation and header check
    //   until its last use before `StreamView::advance`, establishing `avail_in <= isize::MAX as
    //   usize`, `avail_out <= isize::MAX as usize`, and non-null, non-overlapping buffers when
    //   non-empty.
    let buffers = match unsafe { StreamView::try_from_c(&mut *strm_ref) } {
        Ok(buf) => buf,
        Err(e) => return e,
    };

    if !state.coder.is_action_supported(action) {
        return LZMA_PROG_ERROR;
    }

    // NOTE(b/563249255): Report the check type once per concatenated stream.
    if state.tell_flags != 0 && !buffers.input.is_empty() {
        let detection_pending = match &state.coder {
            CoderInner::AutoDecoder(auto) => auto.inner().is_none(),
            _ => false,
        };
        let first_byte = buffers.input[0];
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

        if total_in < LZMA_STREAM_HEADER_SIZE {
            let need = LZMA_STREAM_HEADER_SIZE - total_in;
            let to_feed = buffers.input.len().min(need);
            let header_res =
                state.coder.process(&buffers.input[..to_feed], buffers.output, FfiAction::Run);
            if let Err(e) = header_res.result {
                state.allow_buf_error = false;
                state.action_state = ActionState::Error;
                return helpers::map_rust_error(e);
            }

            // SAFETY:
            // - Operation: `StreamView::advance(strm_ref, header_res.bytes_consumed,
            //   header_res.bytes_produced)`.
            // - Required contract: When `avail_in > 0` / `avail_out > 0`, `next_in` / `next_out`
            //   must be non-null and point to live allocations of at least `avail_in` / `avail_out`
            //   bytes with `avail_in, avail_out <= isize::MAX as usize`.
            // - Evidence: `StreamView::try_from_c` succeeded above, `buffers` is not used after the
            //   `state.coder.process` call above so its borrow of `strm_ref` has ended, and
            //   `lzma_code_inner`'s `# Safety` preconditions 1–2 guarantee single-allocation
            //   validity.
            // - Postcondition: Advances `next_in`/`next_out` and decrements `avail_in`/`avail_out`
            //   in-bounds, preserving the buffer validity preconditions for the remaining bytes.
            unsafe {
                StreamView::advance(strm_ref, header_res.bytes_consumed, header_res.bytes_produced)
            };
            if let ActionState::Finish { .. } = state.action_state {
                state.action_state = ActionState::Finish { saved_avail_in: strm_ref.avail_in };
            }

            if strm_ref.total_in == LZMA_STREAM_HEADER_SIZE as u64 {
                let check = crate::check::check_of(state);
                if check == LZMA_CHECK_NONE && (state.tell_flags & LZMA_TELL_NO_CHECK != 0) {
                    state.tell_flags = 0;
                    state.allow_buf_error = false;
                    return LZMA_NO_CHECK;
                } else if state.tell_flags & LZMA_TELL_ANY_CHECK != 0 {
                    state.tell_flags = 0;
                    state.allow_buf_error = false;
                    return LZMA_GET_CHECK;
                }

                state.tell_flags = 0;
                state.allow_buf_error = false;
                if strm_ref.avail_in == 0 {
                    return LZMA_OK;
                }
            } else {
                state.allow_buf_error = false;
                return LZMA_OK;
            }
        }
    }

    // liblzma never returns an error when no progress was made — it
    // returns Ok with zero progress and lets the BUF_ERROR two-strike
    // logic surface the problem. lzma-rust2 proactively errors on e.g.
    // Finish with insufficient data, so we convert zero-progress errors
    // into zero-progress Ok to match liblzma's behavior.
    let previous_swallowed_unexpected_eof = state.swallowed_unexpected_eof;
    state.swallowed_unexpected_eof = false;

    let mut coder_result = {
        // SAFETY:
        // - Operation: `StreamView::try_from_c(&mut *strm_ref)`.
        // - Required contract: `next_in`/`next_out` single-allocation buffers valid for reads
        //   (initialized `u8` for `next_in`) and writes (`next_out`) and unaliased for the view's
        //   lifetime.
        // - Evidence: Preconditions 1–3 of `lzma_code_inner` together with the in-bounds sub-range
        //   postcondition of `StreamView::advance` (if the header block ran) guarantee the
        //   remaining `avail_in`/`avail_out` ranges are valid and unaliased.
        // - Postcondition: `buffers` is dropped at the end of this block before
        //   `StreamView::advance` is called.
        let buffers = match unsafe { StreamView::try_from_c(&mut *strm_ref) } {
            Ok(buf) => buf,
            Err(e) => return e,
        };
        state.coder.process(buffers.input, buffers.output, action)
    };

    if let Err(ref e) = coder_result.result {
        // lzma-rust2 returns UnexpectedEof only once for each decoder.
        // Subsequent calls would return InvalidData.
        // We're sure to only jump into this branch once in normal execution.
        if e.kind() == ErrorKind::UnexpectedEof && !previous_swallowed_unexpected_eof {
            state.swallowed_unexpected_eof = true;
            coder_result.result = Ok(Status::Ok);
        } else if e.kind() == ErrorKind::InvalidData && previous_swallowed_unexpected_eof {
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

    // SAFETY:
    // - Operation: `StreamView::advance(strm_ref, bytes_consumed, bytes_produced)`.
    // - Required contract: When `avail_in > 0` / `avail_out > 0`, `next_in` / `next_out` must be
    //   non-null and point to live allocations of at least `avail_in` / `avail_out` bytes with
    //   `avail_in, avail_out <= isize::MAX as usize`.
    // - Evidence: `StreamView::try_from_c` succeeded above, `buffers` was dropped at the end of the
    //   block, and `lzma_code_inner`'s `# Safety` preconditions 1–2 guarantee single-allocation
    //   validity.
    // - Postcondition: Advances `next_in`/`next_out` and decrements `avail_in`/`avail_out`
    //   in-bounds.
    unsafe { StreamView::advance(strm_ref, bytes_consumed, bytes_produced) };

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

                    if bytes_consumed == 0 && bytes_produced == 0 {
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
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access (no concurrent or reentrant
///    reads or writes to `*strm` for the duration of this call).
/// 2. If `strm` is non-null, `(*strm).internal` is either null or a live pointer previously
///    allocated by an initialization function in this crate via
///    `Box::into_raw(Box::new(InternalState))` that has not been modified or freed since, and is
///    exclusively accessed by this call.
/// 3. If `strm` is non-null, `(*strm).avail_in > 0`, and `!(*strm).next_in.is_null()`, then
///    `(*strm).next_in` points to at least `(*strm).avail_in` consecutive initialized readable `u8`
///    bytes within a single live allocated object that is not mutated for the duration of this
///    call.
/// 4. If `strm` is non-null, `(*strm).avail_out > 0`, and `!(*strm).next_out.is_null()`, then
///    `(*strm).next_out` points to at least `(*strm).avail_out` consecutive writable bytes within a
///    single live allocated object that is not read or written through any other pointer or
///    reference for the duration of this call.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_code))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `lzma_ret
//   lzma_code(lzma_stream* strm, lzma_action action)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_code))]
pub unsafe extern "C" fn lzma_code(strm: *mut lzma_stream, action: u32) -> lzma_ret {
    if strm.is_null() {
        return LZMA_PROG_ERROR;
    }

    // SAFETY:
    // - Operation: Creating an exclusive reference `&mut *strm` of type `&mut lzma_stream`.
    // - Required contract: `strm` must be non-null, properly aligned for `lzma_stream`, point to a
    //   live initialized `lzma_stream`, and have exclusive access for the duration of this call.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and precondition 1
    //   of this function's `# Safety` contract guarantees alignment, initialization, liveness, and
    //   exclusive access.
    // - Postcondition: `strm_ref` is a valid exclusive reference `&mut lzma_stream`.
    let strm_ref = unsafe { &mut *strm };

    let internal = strm_ref.internal as *mut InternalState;
    if internal.is_null() {
        return LZMA_PROG_ERROR;
    }
    // SAFETY:
    // - Operation: Creating an exclusive reference `&mut *internal` of type `&mut InternalState`.
    // - Required contract: `internal` must be non-null, properly aligned for `InternalState`, point
    //   to a live initialized `InternalState` disjoint from `*strm_ref`, and have exclusive access
    //   for the duration of this call.
    // - Evidence: `internal` is non-null after the `if internal.is_null()` check above. By
    //   precondition 2 of this function's `# Safety` contract, `strm_ref.internal` was allocated by
    //   `Box::into_raw(Box::new(InternalState))` on the heap (disjoint from `*strm_ref`), has not
    //   been freed, and is exclusively accessed by this call.
    // - Postcondition: `state` is a valid exclusive reference `&mut InternalState` disjoint from
    //   `strm_ref`.
    let state = unsafe { &mut *internal };

    // SAFETY:
    // - Operation: `lzma_code_inner(strm_ref, state, action)`.
    // - Required contract: `strm_ref.next_in` and `strm_ref.next_out` (when non-null with non-zero
    //   `avail_in`/`avail_out`) must point to single-allocation buffers valid for reads
    //   (initialized `u8` for `next_in`) and writes (`next_out`) and unaliased for the duration of
    //   the call.
    // - Evidence: Guaranteed directly by preconditions 3 and 4 of `lzma_code`'s `# Safety`
    //   contract, with no intervening mutation of `strm_ref`.
    // - Postcondition: Advances `strm_ref` buffer pointers/counters in-bounds and updates `state`.
    unsafe { lzma_code_inner(strm_ref, state, action) }
}

/// Free decoder memory. Sets `strm->internal` to NULL.
///
/// Safe to call with a NULL `strm` or after a previous `lzma_end()`.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `strm` is either null or a non-null, properly aligned pointer to a live, initialized,
///    writable `lzma_stream` over which this call has exclusive access for the duration of this
///    call.
/// 2. If `strm` is non-null and `(*strm).internal` is non-null, `(*strm).internal` was previously
///    allocated by an initialization function in this crate via
///    `Box::into_raw(Box::new(InternalState))`, has not yet been freed, and no other pointer or
///    reference accesses `*(*strm).internal` concurrently with or after this call.
///
/// If these preconditions hold and `strm` is non-null, any non-null `(*strm).internal`
/// allocation is deallocated and `(*strm).internal` is set to null.
// SAFETY:
// - Operation: `#[unsafe(export_name = crate::prefix!(lzma_end))]`.
// - Required contract: The exported symbol name must not collide with any incompatible symbol in
//   the final linked binary, and the `extern "C"` signature must match `void lzma_end(lzma_stream*
//   strm)` in `lzma_rust_api.h`.
// - Evidence: `LZMA_SYS_PREFIX` and `BUILD` target configuration guarantee symbol uniqueness, and
//   `lzma_rust2_signatures_test` verifies ABI compatibility with `lzma_rust_api.h` at compile time.
#[unsafe(export_name = crate::prefix!(lzma_end))]
pub unsafe extern "C" fn lzma_end(strm: *mut lzma_stream) {
    if strm.is_null() {
        return;
    }
    // SAFETY:
    // - Operation: Creating an exclusive reference `&mut *strm` of type `&mut lzma_stream`.
    // - Required contract: `strm` must be non-null, properly aligned for `lzma_stream`, point to a
    //   live initialized `lzma_stream`, and have exclusive access for the duration of this call.
    // - Evidence: `strm` is non-null after the `if strm.is_null()` check above, and precondition 1
    //   of this function's `# Safety` contract guarantees alignment, initialization, liveness, and
    //   exclusive access.
    // - Postcondition: `strm_ref` is a valid exclusive reference `&mut lzma_stream` for this call.
    let strm_ref = unsafe { &mut *strm };
    // SAFETY:
    // - Operation: `helpers::free_internal_state(strm_ref)`.
    // - Required contract: If `strm_ref.internal` is non-null, it must point to a live
    //   `InternalState` allocated by this crate via `Box::into_raw(Box::new(InternalState))` and
    //   transfer exclusive ownership to this call.
    // - Evidence: Guaranteed by precondition 2 of this function's `# Safety` contract, with no
    //   intervening mutation of `strm_ref.internal`.
    // - Postcondition: Any non-null `strm_ref.internal` allocation is deallocated and
    //   `strm_ref.internal` is set to null.
    unsafe { helpers::free_internal_state(strm_ref) };
}
