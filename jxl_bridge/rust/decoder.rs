//! JXL decoder bridge for C++ consumption via Crubit.
//!
//! Provides a simple, stateful decoder that wraps jxl-rs's `JxlDecoder`
//! into a linear workflow suitable for C++ callers.
//!
//! Each decoding stage takes a `&[u8]` input and returns a `StatusOr<JxlBridgeProcessResult>`.
//! The C++ wrapper advances the caller's span by the consumed count, enabling zero-copy streaming.

use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::panic::{self, AssertUnwindSafe};

use crate::types::{
    self, JxlBridgeBasicInfo, JxlBridgeChannelLayout, JxlBridgeDataType, JxlBridgeDecoderOptions,
    JxlBridgeFeedResult, JxlBridgeFrameHeader, JxlBridgeProcessResult, JxlBridgeSampleFormat,
};
use cpp_std::vector;
use jxl::api::{Event, JxlBasicInfo, JxlDecoder, JxlOutputBuffer, JxlPixelFormat, JxlSignature};
use jxl::error::Error as JxlError;

use status::{Status, StatusError, StatusOr};

/// Maps a `JxlError` to an appropriate `StatusError`.
fn to_status(e: JxlError) -> StatusError {
    let msg = e.to_string();
    match e {
        // Malformed input data.
        JxlError::InvalidSignature
        | JxlError::InvalidBox
        | JxlError::InvalidEnum(..)
        | JxlError::InvalidBitsPerSample(..)
        | JxlError::InvalidExponent(..)
        | JxlError::InvalidMantissa(..)
        | JxlError::InvalidColorEncoding
        | JxlError::InvalidColorSpace
        | JxlError::InvalidGamma(..)
        | JxlError::InvalidRenderingIntent
        | JxlError::InvalidIntensityTarget(..)
        | JxlError::InvalidHuffman
        | JxlError::InvalidAnsHistogram
        | JxlError::InvalidContextMap(..)
        | JxlError::InvalidContextMapHole(..)
        | JxlError::InvalidIccStream
        | JxlError::IccEndOfStream
        | JxlError::IccTooLarge
        | JxlError::NonZeroPadding
        | JxlError::Lz77Disallowed
        | JxlError::UnexpectedLz77Repeat
        | JxlError::InvalidTransformId
        | JxlError::InvalidRCT(..)
        | JxlError::InvalidImageSize(..)
        | JxlError::InvalidQuantEncodingMode
        | JxlError::InvalidQuantEncoding { .. }
        | JxlError::InvalidPermutationSize { .. }
        | JxlError::InvalidPermutationLehmerCode { .. }
        | JxlError::UnexpectedCodestreamBoxEnd
        | JxlError::BlendingPreColorTransform(..)
        | JxlError::PaletteTooLarge(..)
        | JxlError::LevelLimitExceeded { .. }
        | JxlError::SectionTooShort => status::invalid_argument(msg),

        // Resource exhaustion.
        JxlError::OutOfMemory(..)
        | JxlError::ImageOutOfMemory(..)
        | JxlError::ImageDimensionTooLarge(..)
        | JxlError::SizeOverflow => status::resource_exhausted(msg),

        JxlError::ImageSizeTooLarge(..) => status::out_of_range(msg),

        // Data integrity / corruption.
        JxlError::AnsChecksumMismatch => status::data_loss(msg),

        // Caller-provided output buffer issues.
        JxlError::WrongBufferCount(..)
        | JxlError::NotGrayscale
        | JxlError::NotCmyk
        | JxlError::InvalidOutputBufferSize(..) => status::invalid_argument(msg),

        // Caller used the decoder API in the wrong order.
        JxlError::ApiUsageError(..) => status::failed_precondition(msg),

        // Everything else is an internal decoder error.
        _ => status::internal(msg),
    }
}

/// Converts `Result<T, StatusError>` into Crubit-bridged `StatusOr<T>` (or `Status` when `T = ()`).
fn result_to_status_or<T>(result: Result<T, StatusError>) -> StatusOr<T> {
    match result {
        Ok(value) => status::ok(value),
        Err(err) => status::err(err),
    }
}

// Crubit generates `extern "C"` FFI thunks for every public entry point in this module, where an
// uncaught Rust panic from `jxl` would abort the C++ process. Catch panics at the FFI boundary as
// defense-in-depth and poison stateful decoder handles so half-mutated inner state is dropped.
fn format_panic(payload: Box<dyn Any + Send>) -> String {
    let detail = if let Some(msg) = payload.downcast_ref::<&str>() {
        *msg
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.as_str()
    } else {
        "unknown panic payload"
    };
    format!("Rust panic caught in jxl_bridge: {detail}")
}

/// Tracks which decoding state the bridge is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderState {
    /// Initial state, waiting for header data.
    Initializing,
    /// Header has been parsed, ready to decode frames.
    HeaderDecoded,
    /// Frame header has been parsed, ready to decode pixels.
    FrameHeaderDecoded,
    /// All frames have been decoded.
    Done,
    /// Terminal state entered after an unrecoverable decoder error or panic.
    Poisoned,
}

/// Stateful JXL decoder handle.
///
/// # Usage from C++:
/// 1. `JxlBridgeDecoder::new_()` -> decoder
/// 2. `decoder.decode_header(data)` until `HeaderReady` -> returns bytes consumed
/// 3. `decoder.basic_info()` -> image info
/// 4. `decoder.set_pixel_layout(channel_layout, data_type)`
/// 5. Optionally `decoder.decode_frame_header(data)` -> returns bytes consumed
/// 6. `decoder.decode_frame(data, output)` -> returns bytes consumed
/// 7. For animation: repeat steps 5-6 for each frame
pub struct JxlBridgeDecoder {
    decoder: Option<Box<JxlDecoder>>,
    state: DecoderState,
    /// Cached basic info after header decode.
    basic_info: Option<JxlBridgeBasicInfo>,
    /// Pixel format set by `set_pixel_layout`, used for frame decoding.
    pixel_format: Option<JxlPixelFormat>,
    /// Output channel layout set by `set_pixel_layout`.
    output_channel_layout: JxlBridgeChannelLayout,
    /// Output sample format set by `set_pixel_layout`.
    output_sample_format: JxlBridgeSampleFormat,
    /// Cached jxl basic info (internal) for pixel buffer sizing.
    jxl_info: Option<JxlBasicInfo>,
    /// Cached frame header from `frame_header`, cleared after each frame.
    frame_header: Option<JxlBridgeFrameHeader>,
    /// Cached ICC profile of the output color profile, filled lazily by
    /// `icc_profile` and cleared whenever the pixel layout changes. `None`
    /// means "not computed yet".
    icc_profile: Option<Vec<u8>>,
    /// Whether to coalesce animation frames (use image-level dimensions).
    /// When false, per-frame dimensions from the frame header are used.
    coalescing: bool,
}

impl Default for JxlBridgeDecoder {
    fn default() -> Self {
        Self::new_with_options(JxlBridgeDecoderOptions::default())
    }
}

impl Debug for JxlBridgeDecoder {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("JxlBridgeDecoder").finish()
    }
}

impl Drop for JxlBridgeDecoder {
    fn drop(&mut self) {
        let _ = panic::catch_unwind(AssertUnwindSafe(|| drop(self.decoder.take())));
    }
}

impl JxlBridgeDecoder {
    const POISONED_MSG: &'static str =
        "Decoder is in an invalid state after a previous error or internal panic";

    fn from_constructor_result(
        init_result: Result<Box<JxlDecoder>, Box<dyn Any + Send>>,
        coalescing: bool,
    ) -> Self {
        let (decoder, state) = match init_result {
            Ok(inner) => (Some(inner), DecoderState::Initializing),
            Err(_) => (None, DecoderState::Poisoned),
        };
        Self {
            decoder,
            state,
            basic_info: None,
            pixel_format: None,
            output_channel_layout: JxlBridgeChannelLayout::default(),
            output_sample_format: JxlBridgeSampleFormat::default(),
            jxl_info: None,
            frame_header: None,
            icc_profile: None,
            coalescing,
        }
    }

    fn poison(&mut self) {
        self.decoder = None;
        self.state = DecoderState::Poisoned;
        self.basic_info = None;
        self.pixel_format = None;
        self.jxl_info = None;
        self.frame_header = None;
        self.icc_profile = None;
    }

    fn with_decoder_catching_panics<T, F>(&mut self, f: F) -> Result<T, StatusError>
    where
        F: FnOnce(&mut Self, &mut JxlDecoder) -> Result<T, StatusError>,
    {
        if self.state == DecoderState::Poisoned {
            return Err(status::internal(Self::POISONED_MSG));
        }
        let Some(mut inner) = self.decoder.take() else {
            self.poison();
            return Err(status::internal(Self::POISONED_MSG));
        };

        let result = panic::catch_unwind(AssertUnwindSafe(|| f(self, &mut inner)));

        match result {
            Ok(inner_res) => {
                if self.state != DecoderState::Poisoned {
                    self.decoder = Some(inner);
                } else {
                    let _ = panic::catch_unwind(AssertUnwindSafe(move || drop(inner)));
                }
                inner_res
            }
            Err(payload) => {
                let _ = panic::catch_unwind(AssertUnwindSafe(move || drop(inner)));
                self.poison();
                Err(status::internal(&format_panic(payload)))
            }
        }
    }

    /// Creates a new decoder instance with default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new decoder instance with the specified options.
    #[must_use]
    pub fn new_with_options(options: JxlBridgeDecoderOptions) -> Self {
        let coalescing = options.coalescing;
        let init_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let jxl_options = options.to_jxl_options();
            Box::new(JxlDecoder::new(jxl_options))
        }));
        Self::from_constructor_result(init_result, coalescing)
    }

    fn decode_header_inner(
        &mut self,
        decoder: &mut JxlDecoder,
        data: &[u8],
    ) -> Result<JxlBridgeProcessResult, StatusError> {
        if self.state == DecoderState::HeaderDecoded {
            // Header already decoded.
            return Ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::HeaderReady,
                consumed: 0,
            });
        }
        if self.state != DecoderState::Initializing {
            return Err(status::failed_precondition("decode_header called in invalid state"));
        }

        let mut input: &[u8] = data;
        let original_len = data.len();
        let result = decoder.process(&mut input, None, None).map_err(|err| {
            self.poison();
            to_status(err)
        })?;
        let consumed = original_len - input.len();

        match result {
            Event::BasicInfo => {
                let profile = decoder.output_color_profile().ok_or_else(|| {
                    self.poison();
                    status::internal("Output color profile not available after header decode")
                })?;
                let info = decoder.basic_info().ok_or_else(|| {
                    self.poison();
                    status::internal("Basic info not available after header decode")
                })?;

                let bridge_info = JxlBridgeBasicInfo::from_jxl_info(info, profile);
                self.basic_info = Some(bridge_info);
                self.jxl_info = Some(info.clone());
                self.state = DecoderState::HeaderDecoded;
                Ok(JxlBridgeProcessResult { status: JxlBridgeFeedResult::HeaderReady, consumed })
            }
            Event::NeedMoreInput { .. } => {
                Ok(JxlBridgeProcessResult { status: JxlBridgeFeedResult::NeedsMoreInput, consumed })
            }
            other => {
                self.poison();
                Err(status::internal(format!(
                    "Unexpected decoder event during header decode: {other:?}"
                )))
            }
        }
    }

    /// Decodes the image header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance
    /// its input span by `bytes_consumed`.
    ///
    /// - `HeaderReady`: header parsed, call `basic_info()` and `set_pixel_layout()`.
    /// - `NeedsMoreInput`: provide more data and call again.
    pub fn decode_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        result_to_status_or(
            self.with_decoder_catching_panics(|this, decoder| {
                this.decode_header_inner(decoder, data)
            }),
        )
    }

    fn decode_frame_header_inner(
        &mut self,
        decoder: &mut JxlDecoder,
        data: &[u8],
    ) -> Result<JxlBridgeProcessResult, StatusError> {
        if self.pixel_format.is_none() {
            return Err(status::failed_precondition(
                "set_pixel_layout must be called before decode_frame_header",
            ));
        }

        if self.state == DecoderState::FrameHeaderDecoded {
            // Frame header already parsed.
            return Ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::FrameHeaderReady,
                consumed: 0,
            });
        }
        if self.state != DecoderState::HeaderDecoded {
            return Err(status::failed_precondition(
                "decode_frame_header can only be called after header is decoded",
            ));
        }

        let mut input: &[u8] = data;
        let original_len = data.len();
        let result = decoder.process(&mut input, None, None).map_err(|err| {
            self.poison();
            to_status(err)
        })?;
        let consumed = original_len - input.len();

        match result {
            Event::FrameHeader => {
                self.state = DecoderState::FrameHeaderDecoded;
                // Eagerly cache the frame header so that frame_header()
                // can be called with &self.
                let jxl_fh = decoder.frame_header().ok_or_else(|| {
                    self.poison();
                    status::internal("Frame header not available from inner decoder")
                })?;
                self.frame_header = Some(JxlBridgeFrameHeader::from_jxl_frame_header(&jxl_fh));
                Ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::FrameHeaderReady,
                    consumed,
                })
            }
            Event::NeedMoreInput { .. } => {
                Ok(JxlBridgeProcessResult { status: JxlBridgeFeedResult::NeedsMoreInput, consumed })
            }
            other => {
                self.poison();
                Err(status::internal(format!(
                    "Unexpected decoder event during frame header decode: {other:?}"
                )))
            }
        }
    }

    /// Decodes the next frame header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance
    /// its input span by `bytes_consumed`.
    ///
    /// Requires `set_pixel_layout()` to have been called.
    /// After success, call `frame_header()` to retrieve the header.
    ///
    /// - `NeedsMoreInput`: provide more data and call again.
    /// - `FrameHeaderReady`: frame header parsed, call `frame_header()`.
    pub fn decode_frame_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        result_to_status_or(self.with_decoder_catching_panics(|this, decoder| {
            this.decode_frame_header_inner(decoder, data)
        }))
    }

    fn decode_frame_inner(
        &mut self,
        decoder: &mut JxlDecoder,
        data: &[u8],
        output: &mut [u8],
    ) -> Result<JxlBridgeProcessResult, StatusError> {
        let Some(pixel_format) = self.pixel_format.as_ref() else {
            return Err(status::failed_precondition(
                "set_pixel_layout must be called before decode_frame",
            ));
        };
        let Some(jxl_info) = self.jxl_info.as_ref() else {
            return Err(status::failed_precondition("Header has not been decoded yet"));
        };

        // Extract what we need from the borrowed references before the
        // mutable borrows below invalidate them.
        let color_type = &pixel_format.color_type;
        let data_format = pixel_format
            .color_data_format
            .as_ref()
            .ok_or_else(|| status::internal("No color data format set"))?;
        let samples_per_pixel = color_type.samples_per_pixel();
        let bytes_per_sample = data_format.bytes_per_sample();
        let image_size = jxl_info.size;

        // Drive through frame header if needed, then decode the frame.
        let header_result = self.decode_frame_header_inner(decoder, data)?;
        let header_consumed = header_result.consumed;
        if header_result.status == JxlBridgeFeedResult::NeedsMoreInput {
            return Ok(header_result);
        }

        // Use frame header dimensions when not coalescing, otherwise use
        // the image-level dimensions.
        let (width, height) = if self.coalescing {
            (image_size.0, image_size.1)
        } else {
            let fh = self.frame_header.as_ref().ok_or_else(|| {
                status::internal("Frame header not cached after decode_frame_header")
            })?;
            (fh.width as usize, fh.height as usize)
        };
        if width == 0 || height == 0 {
            return Err(status::invalid_argument("Frame has zero width or height"));
        }
        let row_bytes = width
            .checked_mul(samples_per_pixel)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or_else(|| {
                status::internal(format!(
                    "Integer overflow: width({width}) * samples({samples_per_pixel}) \
                     * bps({bytes_per_sample})"
                ))
            })?;

        let required_bytes = height.checked_mul(row_bytes).ok_or_else(|| {
            status::internal(format!("Integer overflow: height({height}) * row_bytes({row_bytes})"))
        })?;
        if required_bytes > isize::MAX as usize {
            return Err(status::internal(format!(
                "Frame buffer size ({required_bytes}) exceeds addressable range"
            )));
        }
        if output.len() < required_bytes {
            return Err(status::failed_precondition(format!(
                "Output buffer too small: need {required_bytes} bytes \
                 ({width}x{height}, {samples_per_pixel} samples, \
                 {bytes_per_sample} bytes/sample), got {}",
                output.len()
            )));
        }

        let output_buf = JxlOutputBuffer::new(output, height, row_bytes);
        let mut output_bufs = [output_buf];

        // Use the remaining data after header consumption.
        let remaining_data = &data[header_consumed..];
        let mut input: &[u8] = remaining_data;

        let result = decoder.process(&mut input, Some(&mut output_bufs), None).map_err(|err| {
            self.poison();
            to_status(err)
        })?;

        let frame_consumed = remaining_data.len() - input.len();
        let total_consumed = header_consumed + frame_consumed;

        match result {
            Event::FrameComplete { has_more_frames } => {
                // Clear cached frame header from this frame.
                self.frame_header = None;
                self.state =
                    if has_more_frames { DecoderState::HeaderDecoded } else { DecoderState::Done };
                if has_more_frames {
                    Ok(JxlBridgeProcessResult {
                        status: JxlBridgeFeedResult::FrameReady,
                        consumed: total_consumed,
                    })
                } else {
                    Ok(JxlBridgeProcessResult {
                        status: JxlBridgeFeedResult::Done,
                        consumed: total_consumed,
                    })
                }
            }
            Event::NeedMoreInput { .. } => Ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::NeedsMoreInput,
                consumed: total_consumed,
            }),
            other => {
                self.poison();
                Err(status::internal(format!(
                    "Unexpected decoder event during frame decode: {other:?}"
                )))
            }
        }
    }

    /// Decodes the next frame from the provided input data into the output buffer.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance
    /// its input span by `bytes_consumed`.
    ///
    /// - `NeedsMoreInput`: provide more data and call again.
    /// - `FrameReady`: frame decoded, more frames remain.
    /// - `Done`: last frame decoded.
    pub fn decode_frame(
        &mut self,
        data: &[u8],
        output: &mut [u8],
    ) -> StatusOr<JxlBridgeProcessResult> {
        result_to_status_or(self.with_decoder_catching_panics(|this, decoder| {
            this.decode_frame_inner(decoder, data, output)
        }))
    }

    /// Returns basic image information such as dimensions and animation metadata.
    ///
    /// Must be called after `decode_header` has successfully parsed the image header
    /// (`HeaderReady`).
    pub fn basic_info(&self) -> StatusOr<JxlBridgeBasicInfo> {
        if self.state == DecoderState::Poisoned {
            return result_to_status_or(Err(status::internal(Self::POISONED_MSG)));
        }
        result_to_status_or(self.basic_info.clone().ok_or_else(|| {
            status::failed_precondition("Header has not been decoded yet; call decode_header first")
        }))
    }

    /// Returns the ICC profile describing the color space of the decoded pixels.
    ///
    /// Must be called after `decode_header` has successfully parsed the image header
    /// (`HeaderReady`). Since `set_pixel_layout` can change the output color profile, call this
    /// after the pixel layout has been set to obtain the profile the decoded pixels will actually
    /// be in.
    ///
    /// Returns an empty profile if the output color encoding has no ICC representation. The result
    /// is cached until the pixel layout changes.
    pub fn icc_profile(&mut self) -> StatusOr<vector<u8>> {
        result_to_status_or(self.with_decoder_catching_panics(|this, decoder| {
            if this.icc_profile.is_none() {
                let profile = decoder.output_color_profile().ok_or_else(|| {
                    status::failed_precondition(
                        "Header has not been decoded yet; call decode_header first",
                    )
                })?;
                let icc = profile.try_as_icc().map(|cow| cow.into_owned()).unwrap_or_default();
                this.icc_profile = Some(icc);
            }
            let cached = this.icc_profile.as_deref().unwrap_or(&[]);
            Ok(vector::from(cached))
        }))
    }

    fn set_pixel_layout_with_format_inner(
        &mut self,
        decoder: &mut JxlDecoder,
        channel_layout: JxlBridgeChannelLayout,
        sample_format: JxlBridgeSampleFormat,
    ) -> Result<(), StatusError> {
        if let Err(msg) = channel_layout.validate() {
            return Err(status::invalid_argument(msg));
        }
        if let Err(msg) = sample_format.validate() {
            return Err(status::invalid_argument(msg));
        }

        let Some(jxl_info) = self.jxl_info.as_ref() else {
            return Err(status::failed_precondition(
                "Header has not been decoded yet; call decode_header until HeaderReady",
            ));
        };

        if self.state != DecoderState::HeaderDecoded {
            return Err(status::failed_precondition(
                "set_pixel_layout can only be called after HeaderReady",
            ));
        }

        let num_extra = jxl_info.extra_channels.len();
        let pixel_format = types::build_pixel_format(&channel_layout, &sample_format, num_extra);

        decoder.set_pixel_format(pixel_format).map_err(to_status)?;
        // Re-read from the decoder to capture any adjustments.
        self.pixel_format = decoder.current_pixel_format().cloned();
        self.output_channel_layout = channel_layout;
        self.output_sample_format = sample_format;
        // The pixel format can change the output color profile.
        self.icc_profile = None;
        Ok(())
    }

    /// Sets how decoded pixels are written into the output buffer: which channels are interleaved
    /// (`channel_layout`) and how each sample is stored (`data_type`).
    ///
    /// Samples use the full range of `data_type` and the host byte order. Use
    /// `set_pixel_layout_with_format` to control either.
    ///
    /// This only describes the caller's buffer. In particular the channel layout does not convert
    /// between color spaces. The color channels are always emitted in the image's own color space,
    /// which `icc_profile` reports.
    ///
    /// Must be called after the header is parsed (`HeaderReady`) and before decoding frames.
    pub fn set_pixel_layout(
        &mut self,
        channel_layout: JxlBridgeChannelLayout,
        data_type: JxlBridgeDataType,
    ) -> Status {
        result_to_status_or(self.with_decoder_catching_panics(|this, decoder| {
            let sample_format = JxlBridgeSampleFormat::new(data_type);
            this.set_pixel_layout_with_format_inner(decoder, channel_layout, sample_format)
        }))
    }

    /// Like `set_pixel_layout`, but `sample_format` also carries the bit depth the decoded samples
    /// are scaled to and the byte order multi-byte samples are written in.
    pub fn set_pixel_layout_with_format(
        &mut self,
        channel_layout: JxlBridgeChannelLayout,
        sample_format: JxlBridgeSampleFormat,
    ) -> Status {
        result_to_status_or(self.with_decoder_catching_panics(|this, decoder| {
            this.set_pixel_layout_with_format_inner(decoder, channel_layout, sample_format)
        }))
    }

    /// Returns true if `set_pixel_layout` has been called successfully.
    #[must_use]
    pub fn has_pixel_layout(&self) -> bool {
        self.pixel_format.is_some()
    }

    /// Returns the output channel layout, or the default if not yet set.
    #[must_use]
    pub fn output_channel_layout(&self) -> JxlBridgeChannelLayout {
        self.output_channel_layout
    }

    /// Returns the output data type, or the default if not yet set.
    #[must_use]
    pub fn output_data_type(&self) -> JxlBridgeDataType {
        self.output_sample_format.data_type
    }

    /// Returns the output sample format, or the default if not yet set.
    #[must_use]
    pub fn output_sample_format(&self) -> JxlBridgeSampleFormat {
        self.output_sample_format.clone()
    }

    /// Returns `true` if there are more frames left to decode in the image.
    #[must_use]
    pub fn has_more_frames(&self) -> bool {
        !matches!(self.state, DecoderState::Done | DecoderState::Poisoned)
    }

    /// Returns the frame header for the next frame to be decoded.
    ///
    /// Must be called after `decode_frame_header()` has successfully parsed a
    /// frame header (i.e. the decoder is in
    /// [`DecoderState::FrameHeaderDecoded`] state).
    /// The returned header is cached; calling this multiple times returns
    /// the same result.
    pub fn frame_header(&self) -> StatusOr<JxlBridgeFrameHeader> {
        if self.state == DecoderState::Poisoned {
            return result_to_status_or(Err(status::internal(Self::POISONED_MSG)));
        }
        result_to_status_or(self.frame_header.clone().ok_or_else(|| {
            status::failed_precondition(
                "frame_header can only be called after decode_frame_header \
                 has successfully parsed a frame header",
            )
        }))
    }
}

/// Checks if the given data starts with a valid JXL signature.
/// Returns true for both bare codestream and container formats.
#[must_use]
pub fn has_jxl_signature(data: &[u8]) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| {
        matches!(
            jxl::api::check_signature(data),
            JxlSignature::Codestream | JxlSignature::Container
        )
    }))
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
        use googletest::prelude::*;
    use status::StatusCode;

    #[gtest]
    fn format_panic_handles_str_string_and_non_string_payloads() {
        let str_payload =
            panic::catch_unwind(|| panic!("static str panic")).expect_err("must catch str panic");
        expect_that!(format_panic(str_payload), contains_substring("static str panic"));

        let string_payload = panic::catch_unwind(|| {
            panic!("{}", String::from("owned string panic"));
        })
        .expect_err("must catch String panic");
        expect_that!(format_panic(string_payload), contains_substring("owned string panic"));

        let custom_payload =
            panic::catch_unwind(|| panic::panic_any(42_i32)).expect_err("must catch i32 panic");
        expect_that!(format_panic(custom_payload), contains_substring("unknown panic payload"));
    }

    #[gtest]
    fn with_decoder_catching_panics_poisons_decoder_and_rejects_subsequent_calls() {
        let mut decoder = JxlBridgeDecoder::new();
        expect_true!(decoder.has_more_frames());

        let result: StatusOr<()> = result_to_status_or(
            decoder.with_decoder_catching_panics(|_this, _inner| panic!("simulated decoder panic")),
        );
        expect_false!(result.is_ok());
        expect_eq!(result.code(), StatusCode::INTERNAL);
        expect_eq!(decoder.state, DecoderState::Poisoned);
        expect_true!(decoder.decoder.is_none());
        expect_false!(decoder.has_more_frames());

        let header_retry = decoder.decode_header(&[0xff, 0x0a]);
        expect_false!(header_retry.is_ok());
        expect_eq!(header_retry.code(), StatusCode::INTERNAL);

        let info_retry = decoder.basic_info();
        expect_false!(info_retry.is_ok());
        expect_eq!(info_retry.code(), StatusCode::INTERNAL);

        let fh_retry = decoder.frame_header();
        expect_false!(fh_retry.is_ok());
        expect_eq!(fh_retry.code(), StatusCode::INTERNAL);

        let icc_retry = decoder.icc_profile();
        expect_false!(icc_retry.is_ok());
        expect_eq!(icc_retry.code(), StatusCode::INTERNAL);

        let layout_retry =
            decoder.set_pixel_layout(JxlBridgeChannelLayout::Rgb, JxlBridgeDataType::U8);
        expect_false!(layout_retry.is_ok());
        expect_eq!(layout_retry.code(), StatusCode::INTERNAL);
    }

    #[gtest]
    fn constructor_panic_fallback_creates_poisoned_decoder() {
        let init_result: Result<Box<JxlDecoder>, _> =
            panic::catch_unwind(|| panic!("simulated constructor panic"));
        let mut decoder = JxlBridgeDecoder::from_constructor_result(init_result, true);
        expect_eq!(decoder.state, DecoderState::Poisoned);
        expect_true!(decoder.decoder.is_none());
        expect_false!(decoder.has_more_frames());

        let header_result = decoder.decode_header(&[0xff, 0x0a]);
        expect_false!(header_result.is_ok());
        expect_eq!(header_result.code(), StatusCode::INTERNAL);
    }

    #[gtest]
    fn poison_clears_cached_fields_and_options_sample_format_helpers_roundtrip() {
        const BASIC_JXL: [u8; 65] = [
            0xff, 0x0a, 0x00, 0x90, 0x01, 0x00, 0x12, 0x88, 0x02, 0x00, 0xd4, 0x00, 0x55, 0x0f,
            0x00, 0x00, 0xa8, 0x50, 0x19, 0x65, 0xdc, 0xe0, 0xe5, 0x5c, 0xcf, 0x97, 0x1f, 0x3a,
            0x2c, 0xa6, 0x6d, 0x5c, 0x67, 0x68, 0xab, 0x6d, 0x0b, 0x4b, 0x12, 0x45, 0xc6, 0xb1,
            0x49, 0x3a, 0x81, 0x43, 0x92, 0x58, 0x04, 0x36, 0x2e, 0x98, 0x07, 0x18, 0x00, 0x86,
            0x99, 0x03, 0x27, 0x33, 0x50, 0xe4, 0x4a, 0x12, 0x00,
        ];

        let mut opts = JxlBridgeDecoderOptions::new();
        opts.sample_limit = 64;
        opts.high_precision = true;
        let cloned_opts = opts.clone();
        expect_eq!(cloned_opts.sample_limit, 64);
        expect_true!(cloned_opts.high_precision);
        let mut target_opts = JxlBridgeDecoderOptions::default();
        target_opts.clone_from(&cloned_opts);
        expect_eq!(target_opts.sample_limit, 64);

        let mut fmt = JxlBridgeSampleFormat::new(JxlBridgeDataType::U16);
        expect_eq!(fmt.effective_bit_depth(), 16);
        fmt.bit_depth = 12;
        expect_eq!(fmt.effective_bit_depth(), 12);

        let mut decoder = JxlBridgeDecoder::new_with_options(target_opts);
        let hdr = decoder.decode_header(&BASIC_JXL);
        expect_true!(hdr.is_ok());
        let layout_res = decoder.set_pixel_layout_with_format(JxlBridgeChannelLayout::Rgb, fmt);
        expect_true!(layout_res.is_ok());
        expect_true!(decoder.has_pixel_layout());
        expect_true!(decoder.icc_profile().is_ok());

        decoder.poison();
        expect_false!(decoder.has_pixel_layout());
        expect_false!(decoder.has_more_frames());
        expect_eq!(decoder.basic_info().code(), StatusCode::INTERNAL);
        drop(decoder);
    }
}
