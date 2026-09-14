//! JXL decoder bridge for C++ consumption via Crubit.
//!
//! Provides a simple, stateful decoder that wraps jxl-rs's `JxlDecoderInner`
//! into a linear workflow suitable for C++ callers.
//!
//! Each decoding stage takes a `&[u8]` input and returns a `StatusOr<JxlBridgeProcessResult>`.
//! The C++ wrapper advances the caller's span by the consumed count, enabling zero-copy streaming.

use std::fmt::{self, Debug, Formatter};

use crate::types::{
    self, JxlBridgeBasicInfo, JxlBridgeColorType, JxlBridgeDataType, JxlBridgeDecoderOptions,
    JxlBridgeFeedResult, JxlBridgeFrameHeader, JxlBridgeProcessResult,
};
use jxl::api::{JxlBasicInfo, JxlDecoderInner, JxlOutputBuffer, JxlPixelFormat, ProcessingResult};
use jxl::error::Error as JxlError;

use status::{NewStatus as Status, NewStatusOr as StatusOr, StatusError};

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
        | JxlError::InvalidOutputBufferSize(..) => status::invalid_argument(msg),

        // Everything else is an internal decoder error.
        _ => status::internal(msg),
    }
}

/// Creates a `StatusOr` with `FAILED_PRECONDITION` status code.
fn precondition_err<T>(msg: &str) -> StatusOr<T> {
    status::err(status::failed_precondition(msg))
}

/// Creates a `Status` with `FAILED_PRECONDITION` status code.
fn precondition_status(msg: &str) -> Status {
    status::err(status::failed_precondition(msg))
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
}

/// Stateful JXL decoder handle.
///
/// # Usage from C++:
/// 1. `JxlBridgeDecoder::new_()` -> decoder
/// 2. `decoder.decode_header(data)` until `HeaderReady` -> returns bytes consumed
/// 3. `decoder.basic_info()` -> image info
/// 4. `decoder.set_output_format(color_type, data_type)`
/// 5. Optionally `decoder.decode_frame_header(data)` -> returns bytes consumed
/// 6. `decoder.decode_frame(data, output)` -> returns bytes consumed
/// 7. For animation: repeat steps 5-6 for each frame
pub struct JxlBridgeDecoder {
    decoder: Box<JxlDecoderInner>,
    state: DecoderState,
    /// Cached basic info after header decode.
    basic_info: Option<JxlBridgeBasicInfo>,
    /// Pixel format set by `set_output_format`, used for frame decoding.
    pixel_format: Option<JxlPixelFormat>,
    /// Output color type set by `set_output_format`.
    output_color_type: JxlBridgeColorType,
    /// Output data type set by `set_output_format`.
    output_data_type: JxlBridgeDataType,
    /// Cached jxl basic info (internal) for pixel buffer sizing.
    jxl_info: Option<JxlBasicInfo>,
    /// Cached frame header from `frame_header`, cleared after each frame.
    frame_header: Option<JxlBridgeFrameHeader>,
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

impl JxlBridgeDecoder {
    /// Creates a new decoder instance with default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new decoder instance with the specified options.
    #[must_use]
    pub fn new_with_options(options: JxlBridgeDecoderOptions) -> Self {
        let coalescing = options.coalescing;
        let jxl_options = options.to_jxl_options();
        Self {
            decoder: Box::new(JxlDecoderInner::new(jxl_options)),
            state: DecoderState::Initializing,
            basic_info: None,
            pixel_format: None,
            output_color_type: JxlBridgeColorType::default(),
            output_data_type: JxlBridgeDataType::default(),
            jxl_info: None,
            frame_header: None,
            coalescing,
        }
    }

    /// Decodes the image header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance
    /// its input span by `bytes_consumed`.
    ///
    /// - `HeaderReady`: header parsed, call `basic_info()` and `set_output_format()`.
    /// - `NeedsMoreInput`: provide more data and call again.
    pub fn decode_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        if self.state == DecoderState::HeaderDecoded {
            // Header already decoded.
            return status::ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::HeaderReady,
                consumed: 0,
            });
        }
        if self.state != DecoderState::Initializing {
            return precondition_err("decode_header called in invalid state");
        }

        let mut input: &[u8] = data;
        let original_len = data.len();
        let result = self.decoder.process(&mut input, None, None).map_err(to_status)?;
        let consumed = original_len - input.len();

        match result {
            ProcessingResult::Complete { .. } => {
                let profile = self.decoder.output_color_profile().ok_or_else(|| {
                    status::internal("Output color profile not available after header decode")
                })?;
                let info = self.decoder.basic_info().ok_or_else(|| {
                    status::internal("Basic info not available after header decode")
                })?;

                let bridge_info = JxlBridgeBasicInfo::from_jxl_info(info, profile);
                self.basic_info = Some(bridge_info);
                self.jxl_info = Some(info.clone());
                self.state = DecoderState::HeaderDecoded;
                status::ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::HeaderReady,
                    consumed,
                })
            }
            ProcessingResult::NeedsMoreInput { .. } => status::ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::NeedsMoreInput,
                consumed,
            }),
        }
    }

    /// Decodes the next frame header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance
    /// its input span by `bytes_consumed`.
    ///
    /// Requires `set_output_format()` to have been called.
    /// After success, call `frame_header()` to retrieve the header.
    ///
    /// - `NeedsMoreInput`: provide more data and call again.
    /// - `FrameHeaderReady`: frame header parsed, call `frame_header()`.
    pub fn decode_frame_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        if self.pixel_format.is_none() {
            return precondition_err("set_output_format must be called before decode_frame_header");
        }

        if self.state == DecoderState::FrameHeaderDecoded {
            // Frame header already parsed.
            return status::ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::FrameHeaderReady,
                consumed: 0,
            });
        }
        if self.state != DecoderState::HeaderDecoded {
            return precondition_err(
                "decode_frame_header can only be called after header is decoded",
            );
        }

        let mut input: &[u8] = data;
        let original_len = data.len();
        let result = self.decoder.process(&mut input, None, None).map_err(to_status)?;
        let consumed = original_len - input.len();

        match result {
            ProcessingResult::Complete { .. } => {
                self.state = DecoderState::FrameHeaderDecoded;
                // Eagerly cache the frame header so that frame_header()
                // can be called with &self.
                let jxl_fh = self.decoder.frame_header().ok_or_else(|| {
                    status::internal("Frame header not available from inner decoder")
                })?;
                self.frame_header = Some(JxlBridgeFrameHeader::from_jxl_frame_header(&jxl_fh));
                status::ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::FrameHeaderReady,
                    consumed,
                })
            }
            ProcessingResult::NeedsMoreInput { .. } => status::ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::NeedsMoreInput,
                consumed,
            }),
        }
    }

    /// Decodes the next frame from the provided input data into the output
    /// buffer.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance its
    /// input span by `bytes_consumed`.
    ///
    /// - `NeedsMoreInput`: provide more data and call again.
    /// - `FrameReady`: frame decoded, more frames remain.
    /// - `Done`: last frame decoded.
    pub fn decode_frame(
        &mut self,
        data: &[u8],
        output: &mut [u8],
    ) -> StatusOr<JxlBridgeProcessResult> {
        let Some(pixel_format) = self.pixel_format.as_ref() else {
            return precondition_err("set_output_format must be called before decode_frame");
        };
        let Some(jxl_info) = self.jxl_info.as_ref() else {
            return precondition_err("Header has not been decoded yet");
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
        let header_result = self.decode_frame_header(data)?;
        let header_consumed = header_result.consumed;
        if header_result.status == JxlBridgeFeedResult::NeedsMoreInput {
            return status::ok(header_result);
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
        if output.len() < required_bytes {
            return status::err(status::failed_precondition(format!(
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

        let result =
            self.decoder.process(&mut input, Some(&mut output_bufs), None).map_err(to_status)?;

        let frame_consumed = remaining_data.len() - input.len();
        let total_consumed = header_consumed + frame_consumed;

        match result {
            ProcessingResult::Complete { .. } => {
                let has_more = self.decoder.has_more_frames();
                // Clear cached frame header from this frame.
                self.frame_header = None;
                self.state =
                    if has_more { DecoderState::HeaderDecoded } else { DecoderState::Done };
                if has_more {
                    status::ok(JxlBridgeProcessResult {
                        status: JxlBridgeFeedResult::FrameReady,
                        consumed: total_consumed,
                    })
                } else {
                    status::ok(JxlBridgeProcessResult {
                        status: JxlBridgeFeedResult::Done,
                        consumed: total_consumed,
                    })
                }
            }
            ProcessingResult::NeedsMoreInput { .. } => status::ok(JxlBridgeProcessResult {
                status: JxlBridgeFeedResult::NeedsMoreInput,
                consumed: total_consumed,
            }),
        }
    }

    /// Returns basic image information such as dimensions and animation metadata.
    ///
    /// Must be called after `decode_header` has successfully parsed the image header
    /// (`HeaderReady`).
    pub fn basic_info(&self) -> StatusOr<JxlBridgeBasicInfo> {
        match &self.basic_info {
            Some(info) => status::ok(info.clone()),
            None => precondition_err("Header has not been decoded yet; call decode_header first"),
        }
    }

    /// Sets the desired output pixel format (color type and data type) for decoded frames.
    ///
    /// Must be called after the header is parsed (`HeaderReady`) and before decoding frames.
    pub fn set_output_format(
        &mut self,
        color_type: JxlBridgeColorType,
        data_type: JxlBridgeDataType,
    ) -> Status {
        let Some(jxl_info) = self.jxl_info.as_ref() else {
            return precondition_status(
                "Header has not been decoded yet; call decode_header until HeaderReady",
            );
        };

        if self.state != DecoderState::HeaderDecoded {
            return precondition_status("set_output_format can only be called after HeaderReady");
        }

        let num_extra = jxl_info.extra_channels.len();
        let pixel_format = types::build_pixel_format(&color_type, &data_type, num_extra);

        self.decoder.set_pixel_format(pixel_format);
        // Re-read from the decoder to capture any adjustments.
        self.pixel_format = self.decoder.current_pixel_format().cloned();
        self.output_color_type = color_type;
        self.output_data_type = data_type;
        status::ok(())
    }

    /// Returns true if `set_output_format` has been called successfully.
    #[must_use]
    pub fn has_output_format(&self) -> bool {
        self.pixel_format.is_some()
    }

    /// Returns the output color type, or the default if not yet set.
    #[must_use]
    pub fn output_color_type(&self) -> JxlBridgeColorType {
        self.output_color_type
    }

    /// Returns the output data type, or the default if not yet set.
    #[must_use]
    pub fn output_data_type(&self) -> JxlBridgeDataType {
        self.output_data_type
    }

    /// Returns `true` if there are more frames left to decode in the image.
    #[must_use]
    pub fn has_more_frames(&self) -> bool {
        self.state != DecoderState::Done && self.decoder.has_more_frames()
    }

    /// Returns the frame header for the next frame to be decoded.
    ///
    /// Must be called after `decode_frame_header()` has successfully parsed a
    /// frame header (i.e. the decoder is in
    /// [`DecoderState::FrameHeaderDecoded`] state).
    /// The returned header is cached; calling this multiple times returns
    /// the same result.
    pub fn frame_header(&self) -> StatusOr<JxlBridgeFrameHeader> {
        match &self.frame_header {
            Some(fh) => status::ok(fh.clone()),
            None => precondition_err(
                "frame_header can only be called after decode_frame_header \
                 has successfully parsed a frame header",
            ),
        }
    }
}

/// Checks if the given data starts with a valid JXL signature.
/// Returns true for both bare codestream and container formats.
#[must_use]
pub fn has_jxl_signature(data: &[u8]) -> bool {
    match jxl::api::check_signature(data) {
        ProcessingResult::Complete { result } => result.is_some(),
        _ => false,
    }
}
