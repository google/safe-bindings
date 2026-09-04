//! JXL decoder bridge for C++ consumption via Crubit.
//!
//! Provides a simple, stateful decoder that wraps jxl-rs's typestate-based API
//! into a linear workflow suitable for C++ callers.
//!
//! Each decoding phase takes a `&[u8]` input and returns a `StatusOr<JxlBridgeProcessResult>`.
//! The C++ wrapper advances the caller's span by the consumed count, enabling zero-copy streaming.

use std::fmt::{self, Debug, Display, Formatter};
use std::mem;

use crate::types::{
    self, JxlBridgeBasicInfo, JxlBridgeColorType, JxlBridgeDataType, JxlBridgeDecoderOptions,
    JxlBridgeFeedResult, JxlBridgeFrameHeader, JxlBridgeProcessResult,
};
use jxl::api::{
    states, JxlBasicInfo, JxlDecoder, JxlOutputBuffer, JxlPixelFormat, ProcessingResult,
};

use status::{NewStatus as Status, NewStatusOr as StatusOr, StatusError};

/// Converts an Error into an InternalError `StatusError`.
fn to_internal(e: impl Display) -> StatusError {
    status::internal(e.to_string())
}

/// Creates a `StatusOr` with `FAILED_PRECONDITION` status code.
fn precondition_err<T>(msg: &str) -> StatusOr<T> {
    status::err(status::failed_precondition(msg))
}

/// Creates a `Status` with `FAILED_PRECONDITION` status code.
fn precondition_status(msg: &str) -> Status {
    status::err(status::failed_precondition(msg))
}

/// Opaque decoder state. Holds the jxl-rs decoder across its various typestates.
///
/// # Usage from C++:
/// 1. `JxlBridgeDecoder::new_()` → decoder
/// 2. `decoder.decode_header(data)` until `HeaderReady` → returns bytes consumed
/// 3. `decoder.basic_info()` → image info
/// 4. `decoder.set_output_format(color_type, data_type)`
/// 5. Optionally `decoder.decode_frame_header(data)` → returns bytes consumed
/// 6. `decoder.decode_frame(data, output)` → returns bytes consumed
/// 7. For animation: repeat steps 5-6 for each frame
pub struct JxlBridgeDecoder {
    inner: Box<DecoderInner>,
}

/// Private inner state holding all jxl crate types.
/// Boxed to keep them opaque to Crubit (prevents jxl.h from being included
/// in the generated C++ header).
struct DecoderInner {
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

enum DecoderState {
    /// Initial state, waiting for header data.
    Initializing { decoder: JxlDecoder<states::Initialized> },
    /// Header has been parsed, ready to decode frames.
    HeaderDecoded { decoder: JxlDecoder<states::WithImageInfo> },
    /// Frame header has been parsed, ready to decode pixels.
    FrameHeaderDecoded { decoder: JxlDecoder<states::WithFrameInfo> },
    /// Consumed / error state.
    Empty,
}

/// Result of attempting to parse a frame header incrementally.
enum FrameHeaderResult {
    /// Frame header fully parsed.
    Complete(JxlDecoder<states::WithFrameInfo>),
    /// Not enough input; the returned decoder can resume when more data arrives.
    NeedsMoreInput(JxlDecoder<states::WithImageInfo>),
}

/// Result of attempting to decode a frame incrementally.
enum FrameDecodeResult {
    /// Frame fully decoded.
    Complete(JxlDecoder<states::WithImageInfo>),
    /// Not enough input; the returned decoder can resume when more data arrives.
    NeedsMoreInput(JxlDecoder<states::WithFrameInfo>),
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
        let jxl_options = options.to_jxl_options();
        Self {
            inner: Box::new(DecoderInner {
                state: DecoderState::Initializing {
                    decoder: JxlDecoder::<states::Initialized>::new(jxl_options),
                },
                basic_info: None,
                pixel_format: None,
                output_color_type: JxlBridgeColorType::default(),
                output_data_type: JxlBridgeDataType::default(),
                jxl_info: None,
                frame_header: None,
                coalescing: options.coalescing,
            }),
        }
    }

    /// Decodes the image header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance its
    /// input span by `bytes_consumed`.
    ///
    /// - `HeaderReady`: header parsed, call `basic_info()` and
    ///   `set_output_format()`.
    /// - `NeedsMoreInput`: provide more data and call again.
    pub fn decode_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        self.inner.decode_header_impl(data)
    }

    /// Decodes the next frame header from the provided input data.
    ///
    /// Returns the feed state result and the number of bytes consumed. The caller should advance its
    /// input span by `bytes_consumed`.
    ///
    /// Requires `set_output_format()` to have been called.
    /// After success, call `frame_header()` to retrieve the header.
    ///
    /// - `NeedsMoreInput`: provide more data and call again.
    /// - `FrameHeaderReady`: frame header parsed, call `frame_header()`.
    pub fn decode_frame_header(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        self.inner.decode_frame_header_impl(data)
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
        self.inner.decode_frame_impl(data, output)
    }

    /// Returns basic image information such as dimensions and animation metadata.
    ///
    /// Must be called after [`decode_header`](Self::decode_header) has successfully
    /// parsed the image header (`HeaderReady`).
    pub fn basic_info(&self) -> StatusOr<JxlBridgeBasicInfo> {
        match &self.inner.basic_info {
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
        self.inner.set_output_format(color_type, data_type)
    }

    /// Returns true if `set_output_format` has been called successfully.
    #[must_use]
    pub fn has_output_format(&self) -> bool {
        self.inner.pixel_format.is_some()
    }

    /// Returns the output color type, or the default if not yet set.
    #[must_use]
    pub fn output_color_type(&self) -> JxlBridgeColorType {
        self.inner.output_color_type
    }

    /// Returns the output data type, or the default if not yet set.
    #[must_use]
    pub fn output_data_type(&self) -> JxlBridgeDataType {
        self.inner.output_data_type
    }

    /// Returns `true` if there are more frames left to decode in the image.
    #[must_use]
    pub fn has_more_frames(&self) -> bool {
        match &self.inner.state {
            DecoderState::HeaderDecoded { decoder, .. } => decoder.has_more_frames(),
            // If we're mid-frame, there's at least one frame being decoded.
            DecoderState::FrameHeaderDecoded { .. } => true,
            _ => false,
        }
    }

    /// Returns the frame header for the next frame to be decoded.
    ///
    /// Must be called after `decode_frame_header()` has successfully parsed a
    /// frame header (i.e. the decoder is in `FrameHeaderDecoded` state).
    /// The returned header is cached; calling this multiple times returns
    /// the same result.
    pub fn frame_header(&mut self) -> StatusOr<JxlBridgeFrameHeader> {
        self.inner.frame_header_impl()
    }
}

impl DecoderInner {
    /// Returns the frame header for the current frame.
    /// Must only be called when the decoder is in `FrameHeaderDecoded` state.
    fn frame_header_impl(&mut self) -> StatusOr<JxlBridgeFrameHeader> {
        // Return cached frame header if already parsed for this frame.
        if let Some(fh) = &self.frame_header {
            return status::ok(fh.clone());
        }

        match &self.state {
            DecoderState::FrameHeaderDecoded { decoder } => {
                let jxl_fh = decoder.frame_header();
                let bridge_fh = JxlBridgeFrameHeader::from_jxl_frame_header(&jxl_fh);
                self.frame_header = Some(bridge_fh.clone());
                status::ok(bridge_fh)
            }
            _ => precondition_err(
                "frame_header can only be called after decode_frame_header \
                 has successfully parsed a frame header",
            ),
        }
    }

    fn set_output_format(
        &mut self,
        color_type: JxlBridgeColorType,
        data_type: JxlBridgeDataType,
    ) -> Status {
        let Some(jxl_info) = self.jxl_info.as_ref() else {
            return precondition_status(
                "Header has not been decoded yet; call decode_header until HeaderReady",
            );
        };

        let num_extra = jxl_info.extra_channels.len();
        let pixel_format = types::build_pixel_format(&color_type, &data_type, num_extra);

        let old_state = mem::replace(&mut self.state, DecoderState::Empty);
        match old_state {
            DecoderState::HeaderDecoded { mut decoder } => {
                decoder.set_pixel_format(pixel_format);
                // Re-read from the decoder to avoid cloning before the move.
                self.pixel_format = Some(decoder.current_pixel_format().clone());
                self.output_color_type = color_type;
                self.output_data_type = data_type;
                self.state = DecoderState::HeaderDecoded { decoder };
                status::ok(())
            }
            other => {
                self.state = other;
                precondition_status("set_output_format can only be called after HeaderReady")
            }
        }
    }

    /// Decodes the image header. Drives the `Initializing` → `HeaderDecoded`
    /// transition.
    fn decode_header_impl(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        let old_state = mem::replace(&mut self.state, DecoderState::Empty);
        let original_len = data.len();

        match old_state {
            DecoderState::Initializing { decoder } => {
                let mut input: &[u8] = data;
                let processing_result = decoder.process(&mut input, None).map_err(to_internal)?;

                match processing_result {
                    ProcessingResult::Complete { result: decoder_with_info } => {
                        let consumed = original_len - input.len();

                        let profile = decoder_with_info.output_color_profile();

                        let info = decoder_with_info.basic_info().clone();
                        let bridge_info = JxlBridgeBasicInfo::from_jxl_info(&info, profile);

                        self.basic_info = Some(bridge_info);
                        self.jxl_info = Some(info);
                        self.state = DecoderState::HeaderDecoded { decoder: decoder_with_info };
                        status::ok(JxlBridgeProcessResult {
                            status: JxlBridgeFeedResult::HeaderReady,
                            consumed,
                        })
                    }
                    ProcessingResult::NeedsMoreInput { fallback, .. } => {
                        let consumed = original_len - input.len();
                        self.state = DecoderState::Initializing { decoder: fallback };
                        status::ok(JxlBridgeProcessResult {
                            status: JxlBridgeFeedResult::NeedsMoreInput,
                            consumed,
                        })
                    }
                }
            }
            DecoderState::HeaderDecoded { decoder } => {
                // Header already decoded.
                self.state = DecoderState::HeaderDecoded { decoder };
                status::ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::HeaderReady,
                    consumed: 0,
                })
            }
            other => {
                self.state = other;
                precondition_err("decode_header called in invalid state")
            }
        }
    }

    /// Decodes the next frame header. Drives the `HeaderDecoded` →
    /// `FrameHeaderDecoded` transition.
    fn decode_frame_header_impl(&mut self, data: &[u8]) -> StatusOr<JxlBridgeProcessResult> {
        if self.pixel_format.is_none() {
            return precondition_err("set_output_format must be called before decode_frame_header");
        }

        let old_state = mem::replace(&mut self.state, DecoderState::Empty);
        let original_len = data.len();

        match old_state {
            DecoderState::HeaderDecoded { decoder } => {
                let mut input: &[u8] = data;
                let header_result = Self::try_drive_frame_header(decoder, &mut input)?;
                let consumed = original_len - input.len();
                match header_result {
                    FrameHeaderResult::Complete(decoder_with_frame) => {
                        self.state =
                            DecoderState::FrameHeaderDecoded { decoder: decoder_with_frame };
                        status::ok(JxlBridgeProcessResult {
                            status: JxlBridgeFeedResult::FrameHeaderReady,
                            consumed,
                        })
                    }
                    FrameHeaderResult::NeedsMoreInput(fallback) => {
                        self.state = DecoderState::HeaderDecoded { decoder: fallback };
                        status::ok(JxlBridgeProcessResult {
                            status: JxlBridgeFeedResult::NeedsMoreInput,
                            consumed,
                        })
                    }
                }
            }
            DecoderState::FrameHeaderDecoded { decoder } => {
                // Frame header already parsed.
                self.state = DecoderState::FrameHeaderDecoded { decoder };
                status::ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::FrameHeaderReady,
                    consumed: 0,
                })
            }
            other => {
                self.state = other;
                precondition_err("decode_frame_header can only be called after header is decoded")
            }
        }
    }

    /// Decodes the next frame into the output buffer.
    fn decode_frame_impl(
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
        let header_result = self.decode_frame_header_impl(data)?;
        let header_consumed = header_result.consumed;
        if header_result.status == JxlBridgeFeedResult::NeedsMoreInput {
            return status::ok(header_result);
        }

        // Use frame header dimensions when not coalescing, otherwise use
        // the image-level dimensions.
        let (width, height) = if self.coalescing {
            (image_size.0, image_size.1)
        } else {
            let fh = self.frame_header_impl()?;
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

        // Extract the decoder from FrameHeaderDecoded state.
        let old_state = mem::replace(&mut self.state, DecoderState::Empty);
        let decoder_with_frame = match old_state {
            DecoderState::FrameHeaderDecoded { decoder } => decoder,
            _ => unreachable!(
                "decode_frame_header_impl succeeded but state is not FrameHeaderDecoded"
            ),
        };

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

        let decode_result =
            Self::drive_frame_decode(decoder_with_frame, &mut input, &mut output_bufs)?;
        match decode_result {
            FrameDecodeResult::Complete(decoder_back) => {
                let frame_consumed = remaining_data.len() - input.len();
                let total_consumed = header_consumed + frame_consumed;
                let has_more = decoder_back.has_more_frames();
                // Clear cached frame header from this frame.
                self.frame_header = None;
                self.state = DecoderState::HeaderDecoded { decoder: decoder_back };
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
            FrameDecodeResult::NeedsMoreInput(decoder) => {
                let frame_consumed = remaining_data.len() - input.len();
                let total_consumed = header_consumed + frame_consumed;
                self.state = DecoderState::FrameHeaderDecoded { decoder };
                status::ok(JxlBridgeProcessResult {
                    status: JxlBridgeFeedResult::NeedsMoreInput,
                    consumed: total_consumed,
                })
            }
        }
    }

    /// Try to parse the frame header, returning the partial decoder if input
    /// is exhausted before the header is complete.
    fn try_drive_frame_header(
        mut decoder: JxlDecoder<states::WithImageInfo>,
        input: &mut &[u8],
    ) -> StatusOr<FrameHeaderResult> {
        loop {
            match decoder.process(input, None).map_err(to_internal)? {
                ProcessingResult::Complete { result } => {
                    return status::ok(FrameHeaderResult::Complete(result));
                }
                ProcessingResult::NeedsMoreInput { fallback, .. } => {
                    if input.is_empty() {
                        return status::ok(FrameHeaderResult::NeedsMoreInput(fallback));
                    }
                    decoder = fallback;
                }
            }
        }
    }

    fn drive_frame_decode(
        mut decoder: JxlDecoder<states::WithFrameInfo>,
        input: &mut &[u8],
        buffers: &mut [JxlOutputBuffer<'_>],
    ) -> StatusOr<FrameDecodeResult> {
        loop {
            match decoder.process(input, buffers, None).map_err(to_internal)? {
                ProcessingResult::Complete { result } => {
                    return status::ok(FrameDecodeResult::Complete(result))
                }
                ProcessingResult::NeedsMoreInput { fallback, .. } => {
                    if input.is_empty() {
                        return status::ok(FrameDecodeResult::NeedsMoreInput(fallback));
                    }
                    decoder = fallback;
                }
            }
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
