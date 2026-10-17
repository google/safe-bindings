use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use crate::helpers::{self, FfiAction};
use crate::types::{
    self, lzma_filter, lzma_options_lzma, lzma_ret, LZMA_CONCATENATED, LZMA_OPTIONS_ERROR,
    LZMA_PROG_ERROR,
};
use lzma_rust2::{
    CheckType, LzipStream, LzipWriter, Lzma2Stream, Lzma2Writer, LzmaOptions, LzmaStream,
    LzmaWriter, Status, StreamResult, XzOptions, XzStream, XzWriter, XzWriterMt,
};

const PENDING_DRAIN_LIMIT: usize = 64 * 1024; // 64 KB limit
const CHUNK_SIZE: usize = 4096; // 4 KB chunks

pub const MATCH_LEN_MIN: u32 = 2;
pub const MATCH_LEN_MAX: u32 = 273;
pub const LZMA_DICT_SIZE_MIN: u32 = 4096;
pub const LZMA_DICT_SIZE_MAX: u32 = (1 << 30) + (1 << 29); // 1.5 GiB

/// Action state for tracking Run vs Finish semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionState {
    Run,
    Finish { saved_avail_in: usize },
    End,
    Error,
}

pub(crate) struct CoderResult {
    pub bytes_consumed: usize,
    pub bytes_produced: usize,
    pub result: io::Result<Status>,
}

impl From<io::Result<StreamResult>> for CoderResult {
    fn from(res: io::Result<StreamResult>) -> Self {
        match res {
            Ok(r) => CoderResult {
                bytes_consumed: r.bytes_consumed,
                bytes_produced: r.bytes_produced,
                result: Ok(r.status),
            },
            Err(e) => CoderResult { bytes_consumed: 0, bytes_produced: 0, result: Err(e) },
        }
    }
}

pub(crate) struct RawProps {
    pub dict_size: u32,
    pub lc: u32,
    pub lp: u32,
    pub pb: u32,
}

/// The internal state representation for the FFI boundary.
pub(crate) struct InternalState {
    pub(crate) coder: CoderInner,
    pub(crate) action_state: ActionState,
    pub(crate) allow_buf_error: bool,
    pub(crate) swallowed_unexpected_eof: bool,
    pub(crate) lzma_options: Option<LzmaOptions>,
    pub(crate) tell_flags: u32,
}

/// Variants of encoders and decoders mapped inside the FFI.
pub(crate) enum CoderInner {
    AutoDecoder(Box<AutoDecoder>),
    XzDecoder(Box<XzStream>),
    XzEncoder(WriterAdapter<XzWriter<Vec<u8>>>),
    XzEncoderMt(WriterAdapterOwned),
    LzmaDecoder(Box<LzmaStream>),
    RawLzma2Decoder(Box<Lzma2Stream>),
    LzmaAloneEncoder(WriterAdapter<LzmaWriter<Vec<u8>>>),
    RawLzma2Encoder(WriterAdapter<Lzma2Writer<Vec<u8>>>),
    RawLzma1Encoder(WriterAdapter<LzmaWriter<Vec<u8>>>),
    LzipDecoder(Box<LzipStream>),
    LzipEncoder(WriterAdapter<LzipWriter<Vec<u8>>>),
}

impl CoderInner {
    pub fn is_action_supported(&self, action: FfiAction) -> bool {
        match self {
            CoderInner::AutoDecoder(_)
            | CoderInner::XzDecoder(_)
            | CoderInner::LzmaDecoder(_)
            | CoderInner::RawLzma2Decoder(_)
            | CoderInner::LzipDecoder(_)
            | CoderInner::LzmaAloneEncoder(_)
            | CoderInner::LzipEncoder(_) => {
                matches!(action, FfiAction::Run | FfiAction::Finish)
            }
            CoderInner::XzEncoder(_)
            | CoderInner::RawLzma2Encoder(_)
            | CoderInner::RawLzma1Encoder(_) => {
                matches!(action, FfiAction::Run | FfiAction::SyncFlush | FfiAction::Finish)
            }
            CoderInner::XzEncoderMt(_) => {
                matches!(action, FfiAction::Run | FfiAction::FullFlush | FfiAction::Finish)
            }
        }
    }

    pub fn process(&mut self, input: &[u8], output: &mut [u8], action: FfiAction) -> CoderResult {
        match self {
            CoderInner::AutoDecoder(auto) => auto.process(input, output, action),
            CoderInner::XzDecoder(xz) => {
                xz.process(input, output, action.as_lzma_rust2_decoder_action()).into()
            }
            CoderInner::XzEncoder(xz) => xz.process(input, output, action),
            CoderInner::XzEncoderMt(xz_mt) => xz_mt.process(input, output, action),
            CoderInner::LzmaDecoder(lzma) => {
                lzma.process(input, output, action.as_lzma_rust2_decoder_action()).into()
            }
            CoderInner::RawLzma2Decoder(lzma2) => {
                lzma2.process(input, output, action.as_lzma_rust2_decoder_action()).into()
            }
            CoderInner::RawLzma2Encoder(lzma2) => lzma2.process(input, output, action),
            // Reject flush() for LZMA1 here, as the underlying crate only exposes a no-op.
            CoderInner::LzmaAloneEncoder(lzma) | CoderInner::RawLzma1Encoder(lzma) => {
                if action == FfiAction::SyncFlush {
                    return CoderResult {
                        bytes_consumed: 0,
                        bytes_produced: 0,
                        result: Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "LZMA1 does not support sync flushing",
                        )),
                    };
                }
                lzma.process(input, output, action)
            }
            CoderInner::LzipDecoder(lzip) => {
                let mut result: CoderResult =
                    lzip.process(input, output, action.as_lzma_rust2_decoder_action()).into();
                // `LzipStream` absorbs the bytes trailing the last member; liblzma leaves them
                // in the input buffer for the caller.
                if matches!(result.result, Ok(Status::StreamEnd)) {
                    result.bytes_consumed =
                        result.bytes_consumed.saturating_sub(lzip.unused_input().len());
                }
                result
            }
            CoderInner::LzipEncoder(lzip) => lzip.process(input, output, action),
        }
    }

    pub fn absorbed_trailing_input(&self) -> usize {
        match self {
            CoderInner::LzmaDecoder(lzma) => lzma.unused_input().len(),
            _ => 0,
        }
    }
}

impl InternalState {
    pub fn new(coder: CoderInner) -> Self {
        Self {
            coder,
            action_state: ActionState::Run,
            allow_buf_error: false,
            swallowed_unexpected_eof: false,
            lzma_options: None,
            tell_flags: 0,
        }
    }

    pub fn memusage(&self) -> u64 {
        match &self.coder {
            CoderInner::RawLzma2Decoder(_) => {
                if let Some(opts) = &self.lzma_options {
                    let dict_size = opts.dict_size.max(lzma_rust2::DICT_SIZE_MIN);
                    return (lzma_rust2::lzma2_get_memory_usage(dict_size) as u64) * 1024;
                }
                0
            }
            CoderInner::LzmaDecoder(_) => {
                if let Some(opts) = &self.lzma_options {
                    let dict_size = opts.dict_size.max(lzma_rust2::DICT_SIZE_MIN);
                    if let Ok(mem_kb) =
                        lzma_rust2::lzma_get_memory_usage(dict_size, opts.lc, opts.lp)
                    {
                        return (mem_kb as u64) * 1024;
                    }
                }
                0
            }
            // NOTE (b/562586054): Add support for lzma_stream_decoder,
            // lzma_auto_decoder, lzma_lzip_decoder, and lzma_alone_decoder.
            _ => 0,
        }
    }
}

/// Convert a C `lzma_options_lzma` to a Rust `LzmaOptions`.
///
/// # Safety
///
/// The caller must ensure that if `opts.preset_dict` is non-null and `opts.preset_dict_size > 0`,
/// `opts.preset_dict` is properly aligned for `u8` (alignment 1) and points to at least
/// `opts.preset_dict_size as usize` consecutive initialized `u8` bytes within a single live
/// allocated object (with `opts.preset_dict_size as usize <= isize::MAX as usize`) that is valid
/// for reads and not mutated for the duration of this call.
///
/// If this precondition holds and this function returns `Ok(lzma_opts)`, any preset
/// dictionary has been copied into an owned `Vec<u8>` inside `lzma_opts` and no raw pointers
/// are retained.
pub(crate) unsafe fn lzma_options_from_c(
    opts: &lzma_options_lzma,
) -> Result<LzmaOptions, lzma_ret> {
    // These constraints follow liblzma's logic.
    // lc, lp, and pb are required to be lower than 8, 4, 4 respectively, according to the LZMA
    // specification. liblzma specifically restricts `lc + lp <= 4` to prevent large memory usage.
    if opts.lc > 8 || opts.lp > 4 || opts.pb > 4 || opts.lc + opts.lp > 4 {
        return Err(LZMA_PROG_ERROR);
    }

    let nice_len = opts.nice_len.max(4);

    let mut lzma_opts = LzmaOptions::new(
        opts.dict_size,
        opts.lc,
        opts.lp,
        opts.pb,
        helpers::mode_from_c(opts.mode)?,
        nice_len,
        helpers::mf_from_c(opts.mf)?,
        i32::try_from(opts.depth).map_err(|_| LZMA_OPTIONS_ERROR)?,
    );
    if !opts.preset_dict.is_null() && opts.preset_dict_size > 0 {
        // SAFETY:
        // - Operation: `std::slice::from_raw_parts(opts.preset_dict, opts.preset_dict_size as
        //   usize)`.
        // - Required contract: `opts.preset_dict` must be non-null, aligned for `u8`, and point to
        //   `opts.preset_dict_size as usize` consecutive initialized `u8` values within a single
        //   allocated object of size `<= isize::MAX as usize` that is not mutated for the lifetime
        //   of `dict`.
        // - Evidence:
        //   - `!opts.preset_dict.is_null()` and `opts.preset_dict_size > 0` are checked above.
        //   - Alignment for `u8` is 1 by the Rust Reference layout rules.
        //   - By this function's `# Safety` precondition, `opts.preset_dict` points to
        //     `opts.preset_dict_size as usize` initialized readable bytes in a single live
        //     allocation (`<= isize::MAX as usize`) that is not mutated during this call.
        // - Postcondition: `dict` is immediately copied into an owned `Vec<u8>` via `dict.to_vec()`
        //   and does not outlive this call.
        let dict =
            unsafe { std::slice::from_raw_parts(opts.preset_dict, opts.preset_dict_size as usize) };
        lzma_opts.preset_dict = Some(dict.to_vec());
    }
    Ok(lzma_opts)
}

/// Validates the constraints liblzma enforces when *initializing* an LZMA1/LZMA2 encoder:
/// `is_options_valid()` in `lzma/lzma_encoder.c` plus the LZ encoder's dictionary size
/// range.
///
/// These are deliberately kept out of [`lzma_options_from_c`] because
/// `lzma_properties_encode` does not apply them: it happily encodes options that no
/// encoder would accept.
pub(crate) fn validate_encoder_options(opts: &lzma_options_lzma) -> Result<(), lzma_ret> {
    if opts.dict_size < LZMA_DICT_SIZE_MIN || opts.dict_size > LZMA_DICT_SIZE_MAX {
        return Err(LZMA_OPTIONS_ERROR);
    }

    if opts.nice_len < MATCH_LEN_MIN || opts.nice_len > MATCH_LEN_MAX {
        return Err(LZMA_OPTIONS_ERROR);
    }

    Ok(())
}

/// Rounds `dict_size` up to the next `2^n` or `2^n + 2^(n - 1)`, saturating at `u32::MAX`.
///
/// liblzma stores this rounded value in the 13-byte `.lzma` header (`alone_encoder.c`)
/// because its own decoder only accepts those dictionary sizes and reports
/// `LZMA_FORMAT_ERROR` for anything else (`alone_decoder.c`).
pub(crate) fn round_up_dict_size(dict_size: u32) -> u32 {
    let mut d = dict_size.max(LZMA_DICT_SIZE_MIN) - 1;
    d |= d >> 2;
    d |= d >> 3;
    d |= d >> 4;
    d |= d >> 8;
    d |= d >> 16;

    if d == u32::MAX {
        d
    } else {
        d + 1
    }
}

// WriterAdapter: wraps a Write-based encoder that has inner_mut() -> &mut Vec<u8>

pub struct WriterAdapter<E: InnerMutVec> {
    encoder: Option<E>,
    pending: Vec<u8>,
    pending_pos: usize,
    finished: bool,
}

pub trait InnerMutVec: Write {
    fn inner_mut(&mut self) -> &mut Vec<u8>;
    fn finish(self) -> io::Result<Vec<u8>>;
}

impl InnerMutVec for XzWriter<Vec<u8>> {
    fn inner_mut(&mut self) -> &mut Vec<u8> {
        XzWriter::inner_mut(self)
    }
    fn finish(self) -> io::Result<Vec<u8>> {
        XzWriter::finish(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl InnerMutVec for LzmaWriter<Vec<u8>> {
    fn inner_mut(&mut self) -> &mut Vec<u8> {
        LzmaWriter::inner_mut(self)
    }
    fn finish(self) -> io::Result<Vec<u8>> {
        LzmaWriter::finish(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl InnerMutVec for Lzma2Writer<Vec<u8>> {
    fn inner_mut(&mut self) -> &mut Vec<u8> {
        Lzma2Writer::inner_mut(self)
    }
    fn finish(self) -> io::Result<Vec<u8>> {
        Lzma2Writer::finish(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl InnerMutVec for LzipWriter<Vec<u8>> {
    fn inner_mut(&mut self) -> &mut Vec<u8> {
        LzipWriter::inner_mut(self)
    }
    fn finish(self) -> io::Result<Vec<u8>> {
        LzipWriter::finish(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl<E: InnerMutVec> WriterAdapter<E> {
    pub fn new(encoder: E) -> Self {
        Self { encoder: Some(encoder), pending: Vec::new(), pending_pos: 0, finished: false }
    }

    #[inline(never)]
    pub fn process(&mut self, input: &[u8], output: &mut [u8], action: FfiAction) -> CoderResult {
        let mut bytes_consumed = 0;
        let mut result = Ok(Status::Ok);

        if !input.is_empty() {
            if let Some(enc) = &mut self.encoder {
                let mut current_input = input;
                let mut available = self.pending.len() - self.pending_pos;

                while !current_input.is_empty() && available < output.len() {
                    let to_write = current_input.len().min(CHUNK_SIZE);
                    let chunk = &current_input[..to_write];

                    // write() could return a partial write count; we advance by that amount
                    match enc.write(chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            bytes_consumed += n;
                            current_input = &current_input[n..];
                        }
                        Err(e) => {
                            result = Err(e);
                            break;
                        }
                    }

                    let inner = enc.inner_mut();
                    if !inner.is_empty() {
                        self.pending.append(inner);
                        available = self.pending.len() - self.pending_pos;
                    }
                }
            }
        }

        // If the caller asked to Finish we should finish the encoder, but ONLY if we have
        // successfully consumed all the input and there is room in the pending buffer footprint.
        if result.is_ok() {
            if action == FfiAction::Finish && !self.finished && bytes_consumed == input.len() {
                if let Some(enc) = self.encoder.take() {
                    match enc.finish() {
                        Ok(mut remaining) => {
                            if !remaining.is_empty() {
                                self.pending.append(&mut remaining);
                            }
                            self.finished = true;
                        }
                        Err(e) => {
                            self.finished = true;
                            result = Err(e);
                        }
                    }
                }
            } else if action == FfiAction::SyncFlush && bytes_consumed == input.len() {
                if let Some(enc) = &mut self.encoder {
                    if let Err(e) = enc.flush() {
                        result = Err(e);
                    } else {
                        let inner = enc.inner_mut();
                        if !inner.is_empty() {
                            self.pending.append(inner);
                        }
                    }
                }
            } else if action == FfiAction::FullFlush {
                result = Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "FULL_FLUSH unsupported on single-threaded encoder",
                ));
            }
        }

        let available = self.pending.len() - self.pending_pos;
        let n = available.min(output.len());
        if n > 0 {
            output[..n].copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + n]);
            self.pending_pos += n;
            if self.pending_pos == self.pending.len() {
                self.pending.clear();
                self.pending_pos = 0;
            } else if self.pending_pos > PENDING_DRAIN_LIMIT {
                // Remove already consumed bytes from pending buffer periodically
                // We don't remove them every time to avoid too much copying, only when the pending
                // prefix size is large enough.
                self.pending.drain(..self.pending_pos);
                self.pending_pos = 0;
            }
        }

        if result.is_ok() && self.finished && self.pending_pos == self.pending.len() {
            result = Ok(Status::StreamEnd);
        }

        CoderResult { bytes_consumed, bytes_produced: n, result }
    }
}

pub(crate) fn check_type_from_c(raw: u32) -> Option<CheckType> {
    match raw {
        types::LZMA_CHECK_NONE => Some(CheckType::None),
        types::LZMA_CHECK_CRC32 => Some(CheckType::Crc32),
        types::LZMA_CHECK_CRC64 => Some(CheckType::Crc64),
        types::LZMA_CHECK_SHA256 => Some(CheckType::Sha256),
        _ => None,
    }
}

/// Converts a C array of `lzma_filter` into `XzOptions`.
///
/// # Safety
///
/// The caller must ensure that:
/// 1. `filters` is non-null, properly aligned for `lzma_filter`, and points to a contiguous
///    sequence of initialized `lzma_filter` elements within a single live allocated object that
///    either contains a terminator element with `id == LZMA_VLI_UNKNOWN` at some index `i <=
///    FILTERS_MAX` or contains at least `FILTERS_MAX + 1` (5) initialized elements, and is not
///    mutated for the duration of this call.
/// 2. For every filter element preceding the `LZMA_VLI_UNKNOWN` terminator (within the first
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
/// If these preconditions hold and this function returns `Ok(xz_opts)`, all filter options have
/// been copied into owned Rust values inside `xz_opts` and no raw pointers are retained.
pub(crate) unsafe fn xz_options_from_filters(
    filters: *const lzma_filter,
    check: CheckType,
) -> io::Result<XzOptions> {
    let mut opts = XzOptions {
        lzma_options: LzmaOptions::with_preset(6),
        check_type: check,
        block_size: None,
        filters: Vec::new(),
    };

    // SAFETY:
    // - Operation: `helpers::parse_filters(filters)`.
    // - Required contract: `filters` must be non-null, aligned for `lzma_filter`, and point to a
    //   single live allocation containing either an `LZMA_VLI_UNKNOWN` terminator within index `<=
    //   FILTERS_MAX` or at least `FILTERS_MAX + 1` initialized `lzma_filter` elements not mutated
    //   during the returned slice's lifetime.
    // - Evidence: Guaranteed directly by precondition 1 of this function's `# Safety` contract,
    //   with no intervening mutation of `filters`.
    // - Postcondition: `filter_slice` is a valid shared slice `&[lzma_filter]` of length `<=
    //   FILTERS_MAX` confined to this call.
    let filter_slice = unsafe { helpers::parse_filters(filters) }.map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "filter chain exceeds maximum length")
    })?;

    for f in filter_slice.iter() {
        if f.id == types::LZMA_FILTER_LZMA2.into() {
            if !f.options.is_null() {
                // SAFETY:
                // - Operation: Creating a shared reference `&*(f.options as *const
                //   lzma_options_lzma)`.
                // - Required contract: `f.options` must be non-null, properly aligned for
                //   `lzma_options_lzma`, and point to a live, initialized `lzma_options_lzma` not
                //   mutated for the duration of the reference.
                // - Evidence: `f.id == u64::from(LZMA_FILTER_LZMA2)` and `!f.options.is_null()`
                //   hold in this branch. By precondition 2 of this function's `# Safety` contract,
                //   `f.options` is aligned, initialized, and not mutated during this call.
                // - Postcondition: `lzma_opts` is a valid shared reference `&lzma_options_lzma`.
                let lzma_opts = unsafe { &*(f.options as *const lzma_options_lzma) };
                let invalid_options =
                    || io::Error::new(io::ErrorKind::InvalidInput, "invalid LZMA2 options");

                validate_encoder_options(lzma_opts).map_err(|_| invalid_options())?;

                // SAFETY:
                // - Operation: `lzma_options_from_c(lzma_opts)`.
                // - Required contract: When `lzma_opts.preset_dict` is non-null and
                //   `lzma_opts.preset_dict_size > 0`, `lzma_opts.preset_dict` must point to at
                //   least `lzma_opts.preset_dict_size as usize` initialized readable bytes in a
                //   single live allocation (`<= isize::MAX as usize`) not mutated during this call.
                // - Evidence: Guaranteed by precondition 2 of this function's `# Safety` contract,
                //   with no intervening mutation of `lzma_opts`.
                // - Postcondition: Returns an owned `LzmaOptions` without retaining raw pointers.
                let parsed_opts = unsafe { lzma_options_from_c(lzma_opts) };
                opts.lzma_options = parsed_opts.map_err(|_| invalid_options())?;
            }
        } else {
            // SAFETY:
            // - Operation: `helpers::filter_config_from_c(f)`.
            // - Required contract: If `f.options` is non-null and `f.id` is a Delta or BCJ filter
            //   ID, `f.options` must be aligned and point to a live, initialized
            //   `lzma_options_delta` or `lzma_options_bcj` not mutated during the call.
            // - Evidence: As established by `helpers::parse_filters` above, `f` is an element of
            //   `filter_slice` preceding the `LZMA_VLI_UNKNOWN` terminator, so precondition 2 of
            //   this function's `# Safety` contract guarantees the required `f.options` validity.
            // - Postcondition: Returns an owned `Option<FilterConfig>` without retaining pointers.
            if let Some(filter_cfg) = unsafe { helpers::filter_config_from_c(f) } {
                opts.filters.push(filter_cfg);
            } else {
                return Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported filter ID"));
            }
        }
    }

    Ok(opts)
}

#[derive(Clone)]
pub(crate) struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("SharedBuffer mutex corrupted").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct WriterAdapterOwned {
    encoder: Option<XzWriterMt<SharedBuffer>>,
    buffer: SharedBuffer,
    pending: Vec<u8>,
    pending_pos: usize,
    finished: bool,
}

impl WriterAdapterOwned {
    pub fn try_new(opts: XzOptions, threads: u32) -> io::Result<Self> {
        let buffer = SharedBuffer(Arc::new(Mutex::new(Vec::new())));
        let encoder = XzWriterMt::new(buffer.clone(), opts, threads as u32)?;
        Ok(Self {
            encoder: Some(encoder),
            buffer,
            pending: Vec::new(),
            pending_pos: 0,
            finished: false,
        })
    }

    #[inline(never)]
    pub fn process(&mut self, input: &[u8], output: &mut [u8], action: FfiAction) -> CoderResult {
        let mut bytes_consumed = 0;
        let mut available = self.pending.len() - self.pending_pos;
        let mut result = Ok(Status::Ok);

        if let Some(enc) = &mut self.encoder {
            let mut current_input = input;

            if input.is_empty() && available < output.len() {
                if let Err(e) = enc.flush() {
                    result = Err(e);
                } else {
                    let mut inner = self.buffer.0.lock().expect("SharedBuffer mutex corrupted");
                    if !inner.is_empty() {
                        self.pending.append(&mut inner);
                    }
                }
            }

            while !current_input.is_empty() && available < output.len() {
                let to_write = current_input.len().min(CHUNK_SIZE);
                let chunk = &current_input[..to_write];

                match enc.write(chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        bytes_consumed += n;
                        current_input = &current_input[n..];
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }

                let mut inner = self.buffer.0.lock().expect("SharedBuffer mutex corrupted");
                if !inner.is_empty() {
                    self.pending.append(&mut inner);
                    available = self.pending.len() - self.pending_pos;
                }
            }

            // Always try to collect any remaining output produced asynchronously
            let mut inner = self.buffer.0.lock().expect("SharedBuffer mutex corrupted");
            if !inner.is_empty() {
                self.pending.append(&mut inner);
            }
        }

        if result.is_ok() {
            if action == FfiAction::Finish && !self.finished && bytes_consumed == input.len() {
                if let Some(enc) = self.encoder.take() {
                    match enc.finish() {
                        Ok(_) => {
                            let mut inner =
                                self.buffer.0.lock().expect("SharedBuffer mutex corrupted");
                            if !inner.is_empty() {
                                self.pending.append(&mut inner);
                            }
                            self.finished = true;
                        }
                        Err(e) => {
                            self.finished = true;
                            result = Err(io::Error::new(io::ErrorKind::Other, e));
                        }
                    }
                }
            } else if action == FfiAction::FullFlush && bytes_consumed == input.len() {
                if let Some(enc) = &mut self.encoder {
                    if let Err(e) = enc.flush() {
                        result = Err(e);
                    } else {
                        let mut inner = self.buffer.0.lock().expect("SharedBuffer mutex corrupted");
                        if !inner.is_empty() {
                            self.pending.append(&mut inner);
                        }
                    }
                }
            }
        }

        let available = self.pending.len() - self.pending_pos;
        let n = available.min(output.len());
        if n > 0 {
            output[..n].copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + n]);
            self.pending_pos += n;
            if self.pending_pos == self.pending.len() {
                self.pending.clear();
                self.pending_pos = 0;
            } else if self.pending_pos > PENDING_DRAIN_LIMIT {
                self.pending.drain(..self.pending_pos);
                self.pending_pos = 0;
            }
        }

        if result.is_ok() {
            let available = self.pending.len() - self.pending_pos;
            if self.finished && available == 0 {
                result = Ok(Status::StreamEnd);
            }
        }

        CoderResult { bytes_consumed, bytes_produced: n, result }
    }
}

pub struct AutoDecoder {
    memlimit: u64,
    flags: u32,
    coder: Option<Box<CoderInner>>,
    /// Set once the detected coder finished its stream while `LZMA_CONCATENATED` was
    /// requested. Mirrors `SEQ_FINISH` of liblzma's auto decoder (`auto_decoder.c`).
    seq_finish: bool,
}

impl AutoDecoder {
    pub fn new(memlimit: u64, flags: u32) -> Self {
        Self { memlimit, flags, coder: None, seq_finish: false }
    }

    /// The coder picked for the detected format, or `None` before detection.
    pub fn inner(&self) -> Option<&CoderInner> {
        self.coder.as_deref()
    }

    pub fn process(&mut self, input: &[u8], output: &mut [u8], action: FfiAction) -> CoderResult {
        if self.seq_finish {
            return seq_finish_result(input.len(), 0, 0, action);
        }

        if self.coder.is_none() {
            if input.is_empty() {
                // Not enough data to detect stream type yet.
                return CoderResult {
                    bytes_consumed: 0,
                    bytes_produced: 0,
                    result: Ok(Status::Ok),
                };
            }
            let memlimit_kb = if self.memlimit == u64::MAX {
                u32::MAX
            } else {
                (self.memlimit / 1024).clamp(0, u32::MAX as u64) as u32
            };

            // Detect the file format. .xz files start with 0xFD which
            // cannot be the first byte of .lzma (LZMA_Alone) format.
            // The .lz format starts with 0x4C which could be the
            // first byte of a .lzma file but luckily it would mean
            // lc/lp/pb being 4/3/1 which liblzma doesn't support because
            // lc + lp > 4. So using just 0x4C to detect .lz is OK here.
            // This logic is the same as liblzma's auto decoder.
            //
            // liblzma only probes for .lz when built with HAVE_LZIP_DECODER, which
            // google3's liblzma does not define, so .lz falls through to the alone
            // decoder there. We keep the behavior the same by introducing the
            // lzip feature, but keep it disabled by default.
            if input[0] == 0xFD {
                let allow_multiple = (self.flags & LZMA_CONCATENATED) != 0;
                let decoder = XzStream::new_mem_limit(allow_multiple, memlimit_kb);
                self.coder = Some(Box::new(CoderInner::XzDecoder(Box::new(decoder))));
            } else if cfg!(feature = "lzip") && input[0] == 0x4C {
                self.coder = Some(Box::new(CoderInner::LzipDecoder(Box::new(
                    LzipStream::new_mem_limit(memlimit_kb),
                ))));
            } else {
                self.coder = Some(Box::new(CoderInner::LzmaDecoder(Box::new(
                    LzmaStream::new_mem_limit(memlimit_kb, None),
                ))));
            }
        }

        let coder = self.coder.as_mut().unwrap();
        let result = coder.process(input, output, action);

        if self.flags & LZMA_CONCATENATED == 0 || !matches!(result.result, Ok(Status::StreamEnd)) {
            return result;
        }

        // `LZMA_CONCATENATED` promises that the input holds nothing but streams, so a finished
        // stream is only the end of the input if nothing trails it. The formats that stop at
        // their own end (`.lzma` and `.lz`) would otherwise silently accept trailing garbage.
        let leftover = input.len() - result.bytes_consumed + coder.absorbed_trailing_input();
        self.seq_finish = true;
        seq_finish_result(leftover, result.bytes_consumed, result.bytes_produced, action)
    }
}

/// The verdict of liblzma's `SEQ_FINISH`: anything behind the last stream is a data error,
/// and the end of the input is only reached once the caller says `LZMA_FINISH`.
fn seq_finish_result(
    leftover: usize,
    bytes_consumed: usize,
    bytes_produced: usize,
    action: FfiAction,
) -> CoderResult {
    let result = if leftover > 0 {
        Err(io::Error::new(io::ErrorKind::InvalidData, "trailing data after the last stream"))
    } else if action == FfiAction::Finish {
        Ok(Status::StreamEnd)
    } else {
        Ok(Status::Ok)
    };
    CoderResult { bytes_consumed, bytes_produced, result }
}
