//! Crubit-compatible data types for the JXL bridge.

use cpp_std::vector;
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

/// Byte order of multi-byte samples in the output buffer.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JxlBridgeEndianness {
    /// The byte order of the host.
    Native,
    LittleEndian,
    BigEndian,
}

impl Default for JxlBridgeEndianness {
    fn default() -> Self {
        Self::Native
    }
}

impl JxlBridgeEndianness {
    fn to_jxl_endianness(self) -> jxl::api::Endianness {
        match self {
            Self::LittleEndian => jxl::api::Endianness::LittleEndian,
            Self::BigEndian => jxl::api::Endianness::BigEndian,
            _ => jxl::api::Endianness::native(),
        }
    }
}

/// Channel layout of the output pixels, i.e. which channels are written to
/// the output buffer and in what order they are interleaved.
///
/// IMPORTANT: this selects an interleaving layout, *not* a color space. The
/// decoder never converts between color spaces, so the color channels are
/// always emitted in the image's own color space. Use
/// `JxlBridgeDecoder::icc_profile` to find out what that color space is.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum JxlBridgeChannelLayout {
    Grayscale,
    GrayscaleAlpha,
    Rgb,
    Rgba,
    Cmyk,
}

impl Default for JxlBridgeChannelLayout {
    fn default() -> Self {
        Self::Rgb
    }
}

impl JxlBridgeChannelLayout {
    fn to_jxl_color_type(self) -> jxl::api::JxlColorType {
        match self {
            Self::Grayscale => jxl::api::JxlColorType::Grayscale,
            Self::GrayscaleAlpha => jxl::api::JxlColorType::GrayscaleAlpha,
            Self::Rgb => jxl::api::JxlColorType::Rgb,
            Self::Rgba => jxl::api::JxlColorType::Rgba,
            Self::Cmyk => jxl::api::JxlColorType::Cmyk,
            _ => Self::default().to_jxl_color_type(),
        }
    }
}

impl JxlBridgeDataType {
    /// Returns the width of a single sample of this type, in bits.
    fn bits_per_sample(self) -> u32 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::F32 => 32,
            _ => Self::default().bits_per_sample(),
        }
    }
}

/// Describes how a single sample is stored in the caller's output buffer.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeSampleFormat {
    pub data_type: JxlBridgeDataType,
    /// Number of bits the decoded samples are scaled to, which must not exceed
    /// the width of `data_type`. 0 means the full width of `data_type`, i.e. 8
    /// for `U8` and 16 for `U16`.
    ///
    /// Set this to the image's `bits_per_sample` to keep the codestream's own
    /// range, e.g. a 12-bit image then yields values in `0..=4095` stored in
    /// `U16` samples rather than being stretched to `0..=65535`.
    ///
    /// Ignored for `F32`.
    pub bit_depth: u32,
    /// Byte order of multi-byte samples. Ignored for `U8`.
    pub endianness: JxlBridgeEndianness,
}

impl JxlBridgeSampleFormat {
    /// Creates a sample format using the full range of `data_type` and the
    /// host byte order.
    #[must_use]
    pub fn new(data_type: JxlBridgeDataType) -> Self {
        Self { data_type, ..Self::default() }
    }

    /// Returns `bit_depth` if it was set, and the full width of `data_type`
    /// otherwise.
    #[must_use]
    pub fn effective_bit_depth(&self) -> u32 {
        if self.bit_depth == 0 {
            self.data_type.bits_per_sample()
        } else {
            self.bit_depth
        }
    }

    /// Returns an error message if the requested bit depth does not fit in the
    /// output data type.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let max_bit_depth = self.data_type.bits_per_sample();
        let bit_depth = self.effective_bit_depth();
        if bit_depth > max_bit_depth {
            return Err(format!(
                "bit_depth ({bit_depth}) exceeds the {max_bit_depth} bits of the output data type"
            ));
        }
        Ok(())
    }

    fn to_jxl_data_format(&self) -> jxl::api::JxlDataFormat {
        // `validate` rejects anything wider than the data type, so the cast
        // cannot truncate.
        let bit_depth = self.effective_bit_depth() as u8;
        let endianness = self.endianness.to_jxl_endianness();
        match self.data_type {
            JxlBridgeDataType::U8 => jxl::api::JxlDataFormat::U8 { bit_depth },
            JxlBridgeDataType::U16 => jxl::api::JxlDataFormat::U16 { endianness, bit_depth },
            JxlBridgeDataType::F32 => jxl::api::JxlDataFormat::F32 { endianness },
            _ => Self::new(JxlBridgeDataType::default()).to_jxl_data_format(),
        }
    }
}

/// Type of an extra (non-color) channel, mirroring the JPEG XL extra channel
/// types.
#[open_enum(allow_alias)]
#[cpp_enum(kind = "enum class")]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JxlBridgeExtraChannelType {
    Alpha,
    Depth,
    SpotColor,
    SelectionMask,
    Black,
    Cfa,
    Thermal,
    Reserved0,
    Reserved1,
    Reserved2,
    Reserved3,
    Reserved4,
    Reserved5,
    Reserved6,
    Reserved7,
    Unknown,
    Optional,
}

impl Default for JxlBridgeExtraChannelType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl JxlBridgeExtraChannelType {
    fn from_jxl(ec_type: jxl::headers::extra_channels::ExtraChannel) -> Self {
        use jxl::headers::extra_channels::ExtraChannel;
        match ec_type {
            ExtraChannel::Alpha => Self::Alpha,
            ExtraChannel::Depth => Self::Depth,
            ExtraChannel::SpotColor => Self::SpotColor,
            ExtraChannel::SelectionMask => Self::SelectionMask,
            ExtraChannel::Black => Self::Black,
            ExtraChannel::CFA => Self::Cfa,
            ExtraChannel::Thermal => Self::Thermal,
            ExtraChannel::Reserved0 => Self::Reserved0,
            ExtraChannel::Reserved1 => Self::Reserved1,
            ExtraChannel::Reserved2 => Self::Reserved2,
            ExtraChannel::Reserved3 => Self::Reserved3,
            ExtraChannel::Reserved4 => Self::Reserved4,
            ExtraChannel::Reserved5 => Self::Reserved5,
            ExtraChannel::Reserved6 => Self::Reserved6,
            ExtraChannel::Reserved7 => Self::Reserved7,
            ExtraChannel::Unknown => Self::Unknown,
            ExtraChannel::Optional => Self::Optional,
        }
    }
}

/// Describes a single extra (non-color) channel of the image.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeExtraChannel {
    pub channel_type: JxlBridgeExtraChannelType,
    /// For an `Alpha` channel, true if the color channels are already
    /// premultiplied by this alpha channel.
    pub alpha_associated: bool,
}

/// Basic image information returned after decoding the header.
#[derive(Debug, Clone, Default)]
pub struct JxlBridgeBasicInfo {
    pub width: u32,
    pub height: u32,
    pub num_color_channels: u32,
    pub extra_channels: vector<JxlBridgeExtraChannel>,
    pub bits_per_sample: u32,
    pub is_float: bool,
    pub has_animation: bool,
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
            extra_channels: info
                .extra_channels
                .iter()
                .map(|ec| JxlBridgeExtraChannel {
                    channel_type: JxlBridgeExtraChannelType::from_jxl(ec.ec_type),
                    alpha_associated: ec.alpha_associated,
                })
                .collect(),
            bits_per_sample: info.bit_depth.bits_per_sample(),
            is_float,
            has_animation: info.animation.is_some(),
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
    /// `JxlBridgeDecoder::set_pixel_layout()` before feeding more data.
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
    channel_layout: &JxlBridgeChannelLayout,
    sample_format: &JxlBridgeSampleFormat,
    num_extra_channels: usize,
) -> jxl::api::JxlPixelFormat {
    let jxl_color_type = channel_layout.to_jxl_color_type();
    let jxl_data_format = sample_format.to_jxl_data_format();
    jxl::api::JxlPixelFormat {
        color_type: jxl_color_type,
        color_data_format: Some(jxl_data_format),
        // Ignore extra channels by default (alpha is interleaved into the
        // color channels by the channel layout).
        extra_channel_format: vec![None; num_extra_channels],
    }
}
