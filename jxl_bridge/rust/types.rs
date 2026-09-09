//! Crubit-compatible data types for the JXL bridge.

use crubit_annotate::cpp_enum;
use open_enum::open_enum;

/// Pixel data type for output buffers.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum JxlBridgeDataType {
    U8,
    U16,
    F32,
}

impl Default for JxlBridgeDataType {
    fn default() -> Self {
        Self::U8
    }
}

/// Color type for output pixels.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum JxlBridgeColorType {
    Grayscale,
    GrayscaleAlpha,
    Rgb,
    Rgba,
}

impl Default for JxlBridgeColorType {
    fn default() -> Self {
        Self::Rgb
    }
}

impl JxlBridgeColorType {
    fn to_jxl_color_type(self) -> jxl::api::JxlColorType {
        match self {
            Self::Grayscale => jxl::api::JxlColorType::Grayscale,
            Self::GrayscaleAlpha => jxl::api::JxlColorType::GrayscaleAlpha,
            Self::Rgb => jxl::api::JxlColorType::Rgb,
            Self::Rgba => jxl::api::JxlColorType::Rgba,
            _ => Self::default().to_jxl_color_type(),
        }
    }
}

impl JxlBridgeDataType {
    fn to_jxl_data_format(self) -> jxl::api::JxlDataFormat {
        match self {
            Self::U8 => jxl::api::JxlDataFormat::U8 { bit_depth: 8 },
            Self::U16 => jxl::api::JxlDataFormat::U16 {
                endianness: jxl::api::Endianness::native(),
                bit_depth: 16,
            },
            Self::F32 => jxl::api::JxlDataFormat::f32(),
            _ => Self::default().to_jxl_data_format(),
        }
    }
}

/// Basic image information returned after decoding the header.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeBasicInfo {
    pub width: u32,
    pub height: u32,
    pub num_color_channels: u32,
    pub has_alpha: bool,
    pub bits_per_sample: u32,
    pub is_float: bool,
    pub has_animation: bool,
    pub num_extra_channels: u32,
    /// True if the image uses the original (embedded) color profile.
    pub uses_original_profile: bool,
}

impl JxlBridgeBasicInfo {
    pub(crate) fn from_jxl_info(
        info: &jxl::api::JxlBasicInfo,
        output_color_profile: &jxl::api::JxlColorProfile,
    ) -> Self {
        let is_float = matches!(info.bit_depth, jxl::api::JxlBitDepth::Float { .. });
        // num_color_channels is set to a default here; callers that need the
        // actual value (e.g. the decoder) must overwrite it from the output
        // color profile after header decoding.
        Self {
            width: info.size.0 as u32,
            height: info.size.1 as u32,
            num_color_channels: output_color_profile.channels() as u32,
            has_alpha: info
                .extra_channels
                .iter()
                .any(|ec| ec.ec_type == jxl::headers::extra_channels::ExtraChannel::Alpha),
            bits_per_sample: info.bit_depth.bits_per_sample(),
            is_float,
            has_animation: info.animation.is_some(),
            num_extra_channels: info.extra_channels.len() as u32,
            uses_original_profile: info.uses_original_profile,
        }
    }
}

/// Per-frame information returned after the frame header has been parsed.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeFrameHeader {
    /// Frame name (empty string if unnamed).
    pub name: String,
    /// Duration in seconds. 0.0 for still images or the last frame.
    pub duration_seconds: f64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

impl JxlBridgeFrameHeader {
    pub(crate) fn from_jxl_frame_header(fh: &jxl::api::JxlFrameHeader) -> Self {
        Self {
            name: fh.name.clone(),
            duration_seconds: fh.duration.unwrap_or(0.0),
            width: fh.size.0 as u32,
            height: fh.size.1 as u32,
        }
    }
}

/// Decoder options for controlling JXL decoding behavior.
#[derive(Debug, Clone)]
pub struct JxlBridgeDecoderOptions {
    /// If true, apply EXIF orientation to the decoded image. Default: true.
    // NOTE: make public once jxl-rs supports it.
    adjust_orientation: bool,
    /// If true, render spot colors. Default: true.
    pub render_spot_colors: bool,
    /// If true, coalesce animation frames. Default: true.
    // NOTE: make public once jxl-rs supports it.
    pub(crate) coalescing: bool,
    /// If true, skip the preview image. Default: true.
    pub skip_preview: bool,
    /// Use high precision mode for decoding. Default: false.
    pub high_precision: bool,
    /// If true, premultiply RGB by alpha. Default: false.
    pub premultiply_output: bool,
    /// If true, only parse frame headers/TOC and skip pixel decoding. Default: false.
    pub scan_frames_only: bool,
    /// Maximum number of samples to decode. 0 means no limit. Default: 0.
    pub sample_limit: u64,
    /// Desired intensity target for HDR tone mapping. 0 means auto. Default: 0.
    // NOTE: make public once jxl-rs supports it.
    desired_intensity_target: f32,
}

impl Default for JxlBridgeDecoderOptions {
    fn default() -> Self {
        Self {
            adjust_orientation: true,
            render_spot_colors: true,
            coalescing: true,
            skip_preview: true,
            high_precision: false,
            premultiply_output: false,
            scan_frames_only: false,
            sample_limit: 0,
            desired_intensity_target: 0.0,
        }
    }
}

impl JxlBridgeDecoderOptions {
    /// Creates options with all defaults.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn to_jxl_options(&self) -> jxl::api::JxlDecoderOptions {
        let mut opts = jxl::api::JxlDecoderOptions::default();
        opts.adjust_orientation = self.adjust_orientation;
        opts.render_spot_colors = self.render_spot_colors;
        opts.coalescing = self.coalescing;
        opts.skip_preview = self.skip_preview;
        opts.high_precision = self.high_precision;
        opts.premultiply_output = self.premultiply_output;
        opts.scan_frames_only = self.scan_frames_only;
        if self.sample_limit > 0 {
            opts.sample_limit = Some(usize::try_from(self.sample_limit).unwrap_or(usize::MAX));
        }
        if self.desired_intensity_target > 0.0 {
            opts.desired_intensity_target = Some(self.desired_intensity_target);
        }
        opts
    }
}

/// Result of feeding data to the streaming decoder.
/// Indicates what happened after processing the latest chunk.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JxlBridgeFeedResult {
    /// The decoder needs more input data. Call `feed_data` again.
    NeedsMoreInput,
    /// The image header has been parsed. Call
    /// `JxlBridgeDecoder::basic_info()` to retrieve it, then
    /// `JxlBridgeDecoder::set_output_format()` before feeding more data.
    HeaderReady,
    /// The frame header has been parsed. Call
    /// `JxlBridgeDecoder::frame_header()` to retrieve it, then
    /// call `decode_frame()` to decode the pixel data.
    FrameHeaderReady,
    /// A frame has been fully decoded into the output buffer.
    FrameReady,
    /// Decoding is complete. No more frames.
    Done,
}

impl Default for JxlBridgeFeedResult {
    fn default() -> Self {
        Self::NeedsMoreInput
    }
}

/// Result of a decode operation, containing the status and the number of bytes
/// consumed from the input.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeProcessResult {
    /// The decoder status after processing.
    pub status: JxlBridgeFeedResult,
    /// Number of bytes consumed from the input data.
    pub consumed: usize,
}

/// Build the JxlPixelFormat from bridge types.
pub(crate) fn build_pixel_format(
    color_type: &JxlBridgeColorType,
    data_type: &JxlBridgeDataType,
    num_extra_channels: usize,
) -> jxl::api::JxlPixelFormat {
    let jxl_color_type = color_type.to_jxl_color_type();
    let jxl_data_format = data_type.to_jxl_data_format();
    jxl::api::JxlPixelFormat {
        color_type: jxl_color_type,
        color_data_format: Some(jxl_data_format),
        // Ignore extra channels by default (alpha is interleaved via color_type).
        extra_channel_format: vec![None; num_extra_channels],
    }
}
