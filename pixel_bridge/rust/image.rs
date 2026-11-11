use crate::reader::{Format, ReadSeek};
use crate::vec_u8::VecU8;
use image::codecs::bmp::BmpDecoder as RustBmpDecoder;
use image::codecs::gif::GifDecoder as RustGifDecoder;
use image::codecs::ico::IcoDecoder as RustIcoDecoder;
use image::codecs::jpeg::JpegDecoder as RustJpegDecoder;
use image::codecs::png::PngDecoder as RustPngDecoder;
use image::codecs::tiff::TiffDecoder as RustTiffDecoder;
use image::codecs::webp::WebPDecoder as RustWebPDecoder;
use image::flat::SampleLayout as RustSampleLayout;
use image::{
    AnimationDecoder, ColorType as RustColorType, ExtendedColorType as RustExtendedColorType,
    Frame as RustFrame, ImageDecoder as RustImageDecoder, Rgba as RustRgba,
};

#[cfg(feature = "jxl")]
use jxl_image_rs_integration::JxlDecoder as RustJxlDecoder;
use std::collections::VecDeque;

pub(crate) enum GenericImageDecoder {
    Png(Box<RustPngDecoder<Box<dyn ReadSeek>>>),
    Gif(Box<RustGifDecoder<Box<dyn ReadSeek>>>),
    Jpeg(Box<RustJpegDecoder<Box<dyn ReadSeek>>>),
    WebP(Box<RustWebPDecoder<Box<dyn ReadSeek>>>),
    Tiff(Box<RustTiffDecoder<Box<dyn ReadSeek>>>),
    Bmp(Box<RustBmpDecoder<Box<dyn ReadSeek>>>),
    Ico(Box<RustIcoDecoder<Box<dyn ReadSeek>>>),

    #[cfg(feature = "jxl")]
    Jxl(Box<RustJxlDecoder<Box<dyn ReadSeek>>>),
}

macro_rules! call_generic_decoder {
    ($var:expr, $decoder:ident,$expr:expr) => {
        match $var {
            &GenericImageDecoder::Png(ref $decoder) => $expr,
            &GenericImageDecoder::Gif(ref $decoder) => $expr,
            &GenericImageDecoder::Jpeg(ref $decoder) => $expr,
            &GenericImageDecoder::WebP(ref $decoder) => $expr,
            &GenericImageDecoder::Tiff(ref $decoder) => $expr,
            &GenericImageDecoder::Bmp(ref $decoder) => $expr,
            &GenericImageDecoder::Ico(ref $decoder) => $expr,

            #[cfg(feature = "jxl")]
            &GenericImageDecoder::Jxl(ref $decoder) => $expr,
        }
    };
}

macro_rules! call_generic_decoder_mut {
    ($var:expr, $decoder:ident,$expr:expr) => {
        match $var {
            &mut GenericImageDecoder::Png(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::Gif(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::Jpeg(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::WebP(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::Tiff(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::Bmp(ref mut $decoder) => $expr,
            &mut GenericImageDecoder::Ico(ref mut $decoder) => $expr,

            #[cfg(feature = "jxl")]
            &mut GenericImageDecoder::Jxl(ref mut $decoder) => $expr,
        }
    };
}

macro_rules! call_generic_decoder_owned {
    ($var:expr, $decoder:ident,$expr:expr) => {
        match $var {
            GenericImageDecoder::Png($decoder) => $expr,
            GenericImageDecoder::Gif($decoder) => $expr,
            GenericImageDecoder::Jpeg($decoder) => $expr,
            GenericImageDecoder::WebP($decoder) => $expr,
            GenericImageDecoder::Tiff($decoder) => $expr,
            GenericImageDecoder::Bmp($decoder) => $expr,
            GenericImageDecoder::Ico($decoder) => $expr,

            #[cfg(feature = "jxl")]
            GenericImageDecoder::Jxl($decoder) => $expr,
        }
    };
}

impl RustImageDecoder for GenericImageDecoder {
    fn dimensions(&self) -> (u32, u32) {
        call_generic_decoder!(self, decoder, decoder.dimensions())
    }

    fn color_type(&self) -> RustColorType {
        call_generic_decoder!(self, decoder, decoder.color_type())
    }

    fn original_color_type(&self) -> RustExtendedColorType {
        call_generic_decoder!(self, decoder, decoder.original_color_type())
    }

    fn icc_profile(&mut self) -> image::ImageResult<Option<Vec<u8>>> {
        call_generic_decoder_mut!(self, decoder, decoder.icc_profile())
    }

    fn exif_metadata(&mut self) -> image::ImageResult<Option<Vec<u8>>> {
        call_generic_decoder_mut!(self, decoder, decoder.exif_metadata())
    }

    fn xmp_metadata(&mut self) -> image::ImageResult<Option<Vec<u8>>> {
        call_generic_decoder_mut!(self, decoder, decoder.xmp_metadata())
    }

    fn iptc_metadata(&mut self) -> image::ImageResult<Option<Vec<u8>>> {
        call_generic_decoder_mut!(self, decoder, decoder.iptc_metadata())
    }

    fn set_limits(&mut self, limits: image::Limits) -> image::ImageResult<()> {
        call_generic_decoder_mut!(self, decoder, decoder.set_limits(limits.clone()))
    }

    fn total_bytes(&self) -> u64 {
        call_generic_decoder!(self, decoder, decoder.total_bytes())
    }

    fn read_image(self, buf: &mut [u8]) -> image::ImageResult<()>
    where
        Self: Sized,
    {
        call_generic_decoder_owned!(self, decoder, decoder.read_image(buf))
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> image::ImageResult<()> {
        (*self).read_image(buf)
    }
}

/// A flat buffer over a (multi channel) image.
///
/// Its representation allows grouping by color planes instead of by pixel as
/// long as the strides of each extent are constant.
#[derive(Default)]
pub struct ImageDecoder {
    inner: Option<GenericImageDecoder>,
    sample_layout: Option<RustSampleLayout>,
    background_subs: Option<(RustRgba<u8>, RustRgba<u16>)>,
    pub(crate) should_premultiply: bool,
    max_alloc: Option<u64>,
}

impl std::fmt::Debug for ImageDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageDecoder")
    }
}

pub type Status = Result<(), VecU8>;

#[inline(always)]
fn ok() -> Status {
    Ok(())
}

#[inline(always)]
fn internal_error(message: impl Into<String>) -> Status {
    Err(message.into().into())
}

#[inline(always)]
fn invalid_argument_error(message: impl Into<String>) -> Status {
    Err(message.into().into())
}

/// Which Rust integer type backs the value of a pixel.
#[repr(C)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub enum PixelType {
    // We need a default since this type is otherwise not bridged by Crubit.
    #[default]
    U8,
    U16,
    F32,
}

impl From<RustColorType> for PixelType {
    fn from(value: RustColorType) -> Self {
        match value {
            RustColorType::L8 | RustColorType::La8 | RustColorType::Rgb8 | RustColorType::Rgba8 => {
                PixelType::U8
            }
            RustColorType::L16
            | RustColorType::La16
            | RustColorType::Rgb16
            | RustColorType::Rgba16 => PixelType::U16,
            RustColorType::Rgb32F | RustColorType::Rgba32F => PixelType::F32,
            // Match arm exists because the enum is non-exhaustive. All currently mentioned
            // options are listed here.
            _ => PixelType::default(),
        }
    }
}

/// An enumeration over supported color types.
#[repr(C)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub enum ColorType {
    // We need a default since this type is otherwise not bridged by Crubit.
    #[default]
    L,
    La,
    Rgb,
    Rgba,
}

/// Information about strides (offset to the next sample).
pub struct Strides {
    /// Add this to an index to get to the next sample in x-direction.
    pub width: usize,
    /// Add this to an index to get to the next sample in y-direction.
    pub height: usize,
    /// Add this to an index to get to the sample in the next channel.
    pub channels: usize,
}

impl From<RustColorType> for ColorType {
    fn from(value: RustColorType) -> Self {
        match value {
            RustColorType::L8 | RustColorType::L16 => ColorType::L,
            RustColorType::La8 | RustColorType::La16 => ColorType::La,
            RustColorType::Rgb8 | RustColorType::Rgb16 | RustColorType::Rgb32F => ColorType::Rgb,
            RustColorType::Rgba8 | RustColorType::Rgba16 | RustColorType::Rgba32F => {
                ColorType::Rgba
            }
            // Match arm exists because the enum is non-exhaustive. All currently mentioned
            // options are listed here.
            _ => ColorType::default(),
        }
    }
}

// A single frame of an animated image.
#[derive(Default)]
pub struct Frame {
    inner: Option<RustFrame>,
}

#[cfg(debug_assertions)]
impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame").finish()
    }
}

/// A collection of animated image frames.
#[derive(Default)]
pub struct Frames {
    inner: VecDeque<RustFrame>,
}

#[cfg(debug_assertions)]
impl std::fmt::Debug for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frames").finish()
    }
}

impl Frame {
    fn new(frame: RustFrame) -> Self {
        Self { inner: Some(frame) }
    }

    /// Delay of the current frame in ms.
    pub fn curr_delay_ms(&self) -> u64 {
        run_catching_panics(|| match self.inner {
            Some(ref frame) => {
                let delay = std::time::Duration::from(frame.delay());
                delay.as_millis() as u64
            }
            // We are in a moved out state, return an arbitrary value.
            None => 0,
        })
        .unwrap_or(0)
    }

    /// Get a reference to the underlying data. The reference will be living as
    /// long as the object itself.
    pub fn image_ref(&self) -> *const [u8] {
        const EMPTY: &[u8] = &[];
        self.inner
            .as_ref()
            .map_or(EMPTY as *const _, |inner| inner.buffer().as_raw() as &[u8] as *const _)
    }
}

impl Frames {
    fn new(frames: Vec<RustFrame>) -> Self {
        Self {
            // As per Rust docs: Creating a `VecDeque` from a `Vec` is O(1) and guaranteed
            // to not reallocate.
            inner: VecDeque::from(frames),
        }
    }

    /// Returns the bytes of the current image and advances to the next Frame.
    pub fn curr_frame_and_advance(&mut self) -> Option<Frame> {
        let first = self.inner.pop_front()?;
        Some(Frame::new(first))
    }
}

fn format_panic(err: Box<dyn std::any::Any + Send>) -> String {
    let backtrace = std::backtrace::Backtrace::capture();
    let msg = if let Some(msg) = err.downcast_ref::<&str>() {
        *msg
    } else if let Some(msg) = err.downcast_ref::<String>() {
        msg.as_str()
    } else {
        "Unknown panic payload"
    };
    // This error message needs to start with "Rust panic caught", as
    // that is how we identify it on the C++ side.
    format!("Rust panic caught: {}\nBacktrace:\n{}", msg, backtrace)
}

pub(crate) fn run_catching_panics<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> T,
{
    // b/491846166 - The image crate can panic on malformed input.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(res) => Ok(res),
        Err(err) => {
            let formatted_error = format_panic(err);
            Err(formatted_error)
        }
    }
}

fn run_fallible_result<F>(f: F) -> Result<Frames, VecU8>
where
    F: FnOnce() -> Result<Frames, VecU8>,
{
    match run_catching_panics(f) {
        Ok(res) => res,
        Err(formatted_error) => Err(formatted_error.into()),
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum DestinationAlignment {
    Aligned,
    Unaligned,
}

fn checked_row_major_packed(channels: u8, width: u32, height: u32) -> Option<RustSampleLayout> {
    let width_usize = usize::try_from(width).ok()?;
    usize::from(channels).checked_mul(width_usize)?;
    Some(RustSampleLayout::row_major_packed(channels, width, height))
}

fn blend_pixels_in_place<P>(samples: &mut [P::Subpixel], background: P)
where
    P: image::Pixel,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    for pixel_slice in samples.chunks_exact_mut(channels) {
        let pixel = P::from_slice_mut(pixel_slice);
        let mut blended = background;
        blended.blend(pixel);
        *pixel = blended;
    }
}

impl ImageDecoder {
    /// Creates a new `ImageDecoder`.
    pub(crate) fn new(rust_decoder: GenericImageDecoder) -> Self {
        let (width, height) = rust_decoder.dimensions();
        Self {
            sample_layout: match &rust_decoder.color_type() {
                RustColorType::L16 | RustColorType::L8 => {
                    checked_row_major_packed(1, width, height)
                }
                RustColorType::La16 | RustColorType::La8 => {
                    checked_row_major_packed(2, width, height)
                }
                RustColorType::Rgb32F | RustColorType::Rgb16 | RustColorType::Rgb8 => {
                    checked_row_major_packed(3, width, height)
                }
                RustColorType::Rgba32F | RustColorType::Rgba16 | RustColorType::Rgba8 => {
                    checked_row_major_packed(4, width, height)
                }
                _ => None,
            },
            inner: Some(rust_decoder),
            background_subs: None,
            should_premultiply: false,
            max_alloc: None,
        }
    }

    fn insufficient_memory_message() -> String {
        format!(
            "Failed to decode image: {:?}",
            image::ImageError::Limits(image::error::LimitError::from_kind(
                image::error::LimitErrorKind::InsufficientMemory,
            ))
        )
    }

    fn compute_required_bytes(
        inner: &GenericImageDecoder,
        alignment: DestinationAlignment,
    ) -> Result<(usize, u128), Status> {
        let (width, height) = inner.dimensions();
        let color_type = inner.color_type();
        let bytes_per_pixel = u128::from(color_type.bytes_per_pixel());
        let pixel_count = u128::from(width) * u128::from(height);
        let output_bytes_u128 = pixel_count * bytes_per_pixel;

        let Ok(output_bytes_usize) = usize::try_from(output_bytes_u128) else {
            return Err(invalid_argument_error(Self::insufficient_memory_message()));
        };
        if output_bytes_usize > isize::MAX as usize {
            return Err(invalid_argument_error(Self::insufficient_memory_message()));
        }

        let required_u128 = match inner {
            GenericImageDecoder::Tiff(tiff_decoder) => {
                let tiff_bpp = match tiff_decoder.original_color_type() {
                    RustExtendedColorType::Cmyk8 => 4u128,
                    RustExtendedColorType::Cmyk16 => 8u128,
                    _ => bytes_per_pixel,
                };
                output_bytes_u128.max(pixel_count * tiff_bpp)
            }
            GenericImageDecoder::WebP(webp_decoder) if webp_decoder.has_animation() => {
                output_bytes_u128.max(pixel_count * 4)
            }

            #[cfg(feature = "jxl")]
            GenericImageDecoder::Jxl(_) => {
                let channel_count = usize::from(color_type.channel_count()).max(1);
                let bytes_per_sample = usize::from(color_type.bytes_per_pixel()) / channel_count;
                if alignment == DestinationAlignment::Unaligned && bytes_per_sample > 1 {
                    output_bytes_u128 * 2
                } else {
                    output_bytes_u128
                }
            }
            _ => {
                let _ = alignment;
                output_bytes_u128
            }
        };

        Ok((output_bytes_usize, required_u128))
    }

    fn validate_read_limits(&self, alignment: DestinationAlignment) -> Result<usize, Status> {
        let Some(ref inner) = self.inner else {
            return Err(internal_error("ImageDecoder is in invalid, moved out state"));
        };
        let (output_bytes, required_bytes) = Self::compute_required_bytes(inner, alignment)?;
        if let Some(max_alloc) = self.max_alloc
            && required_bytes > u128::from(max_alloc)
        {
            return Err(invalid_argument_error(Self::insufficient_memory_message()));
        }
        Ok(output_bytes)
    }

    /// Validates that the decoder has not been moved from and that the output
    /// image fits within the configured memory limits and platform bounds.
    pub fn check_read_limits(&self) -> Status {
        match run_catching_panics(|| {
            match self.validate_read_limits(DestinationAlignment::Aligned) {
                Ok(_) => ok(),
                Err(status) => status,
            }
        }) {
            Ok(status) => status,
            Err(formatted_error) => internal_error(formatted_error),
        }
    }

    fn decode_and_postprocess(self, byte_buf: &mut [u8]) -> Status {
        match run_catching_panics(move || {
            let Some(ref inner_ref) = self.inner else {
                return internal_error("ImageDecoder is in invalid, moved out state");
            };
            let color_type = inner_ref.color_type();
            let channel_count = usize::from(color_type.channel_count()).max(1);
            let bytes_per_sample = usize::from(color_type.bytes_per_pixel()) / channel_count;
            let alignment = if bytes_per_sample > 1
                && bytes_per_sample.is_power_of_two()
                && byte_buf.as_ptr().align_offset(bytes_per_sample) != 0
            {
                DestinationAlignment::Unaligned
            } else {
                DestinationAlignment::Aligned
            };

            let expected_bytes = match self.validate_read_limits(alignment) {
                Ok(expected) => expected,
                Err(status) => return status,
            };
            if byte_buf.len() != expected_bytes {
                return invalid_argument_error(format!(
                    "Failed to decode image: output buffer length ({}) does not match expected byte size ({expected_bytes})",
                    byte_buf.len(),
                ));
            }

            let Some(inner) = self.inner else {
                return internal_error("ImageDecoder is in invalid, moved out state");
            };
            if let Err(err) = inner.read_image(byte_buf) {
                return invalid_argument_error(format!("Failed to decode image: {:?}", err));
            }
            if let Some(background_subs) = self.background_subs
                && let Err(status) =
                    Self::perform_color_correction(byte_buf, background_subs, color_type)
            {
                return status;
            }
            if self.should_premultiply
                && let Err(status) = Self::premultiply_alpha_bytes(byte_buf, color_type)
            {
                return status;
            }
            ok()
        }) {
            Ok(status) => status,
            Err(formatted_error) => internal_error(formatted_error),
        }
    }

    /// Read the raw u8 image data into `buf`.
    pub fn read_u8_slice(self, buf: &mut [u8]) -> Status {
        self.decode_and_postprocess(buf)
    }

    /// Read the raw u16 image data into `buf`.
    pub fn read_u16_slice(self, buf: &mut [u16]) -> Status {
        let byte_buf: &mut [u8] = bytemuck::cast_slice_mut(buf);
        self.decode_and_postprocess(byte_buf)
    }

    /// Read the raw f32 image data into `buf`.
    pub fn read_f32_slice(self, buf: &mut [f32]) -> Status {
        let byte_buf: &mut [u8] = bytemuck::cast_slice_mut(buf);
        self.decode_and_postprocess(byte_buf)
    }

    /// Returns the total byte size of the decoded image, or 0 if moved from.
    pub fn total_bytes(&self) -> u64 {
        run_catching_panics(|| self.inner.as_ref().map_or(0, |inner| inner.total_bytes()))
            .unwrap_or(0)
    }

    /// Returns the width of the image data produced by this decoder.
    pub fn width(&self) -> usize {
        run_catching_panics(|| self.inner.as_ref().map_or(0, |inner| inner.dimensions().0 as usize))
            .unwrap_or(0)
    }

    /// Returns the height of the image data produced by this decoder.
    pub fn height(&self) -> usize {
        run_catching_panics(|| self.inner.as_ref().map_or(0, |inner| inner.dimensions().1 as usize))
            .unwrap_or(0)
    }

    /// Returns the color type of the image data produced by this decoder.
    pub fn color_type(&self) -> ColorType {
        run_catching_panics(|| {
            self.inner
                .as_ref()
                .map_or_else(ColorType::default, |inner| ColorType::from(inner.color_type()))
        })
        .unwrap_or_default()
    }

    /// Returns the PixelType of the image data produced by this decoder.
    pub fn pixel_type(&self) -> PixelType {
        run_catching_panics(|| {
            self.inner
                .as_ref()
                .map_or_else(PixelType::default, |inner| PixelType::from(inner.color_type()))
        })
        .unwrap_or_default()
    }

    /// Returns the format of the image.
    pub fn format(&self) -> Format {
        // If `self.inner` is `None`, we moved out of the decoder and all methods will throw
        // errors. We default to Png in this case, which is an arbitrary choice.
        let Some(inner) = &self.inner else {
            return Format::Png;
        };
        match inner {
            GenericImageDecoder::Png(_) => Format::Png,
            GenericImageDecoder::Gif(_) => Format::Gif,
            GenericImageDecoder::Jpeg(_) => Format::Jpeg,
            GenericImageDecoder::WebP(_) => Format::WebP,
            GenericImageDecoder::Tiff(_) => Format::Tiff,
            GenericImageDecoder::Bmp(_) => Format::Bmp,
            GenericImageDecoder::Ico(_) => Format::Ico,

            #[cfg(feature = "jxl")]
            GenericImageDecoder::Jxl(_) => Format::Jxl,
        }
    }

    /// Whether the image is originally a CMYK image (this class will convert
    /// it and return an RGB anyways).
    pub fn is_cmyk(&self) -> bool {
        run_catching_panics(|| {
            self.inner.as_ref().is_some_and(|inner| {
                // The zune_image lib returns YCCK for a lot of images that are CMYK in
                // libjpeg_turbo. For YCCK, it returns `ExtendedColorType::Unknown(253)`.
                matches!(
                    inner.original_color_type(),
                    RustExtendedColorType::Cmyk8 | RustExtendedColorType::Cmyk16
                )
            })
        })
        .unwrap_or(false)
    }

    /// Whether the image originally used an indexed color palette.
    ///
    /// Note: This is currently only relevant for PNG images, where the `image` crate uses
    /// `ExtendedColorType::Unknown(u8)` to represent indexed color types.
    pub fn has_palette(&self) -> bool {
        run_catching_panics(|| match self.format() {
            Format::Png => self.inner.as_ref().is_some_and(|inner| {
                matches!(inner.original_color_type(), RustExtendedColorType::Unknown(_))
            }),
            _ => false,
        })
        .unwrap_or(false)
    }

    /// Returns the bit depth of an image. Note that this only indicates the depth as
    /// encoded and may not necessarily indicate the bit depth at which an image would
    /// be decoded at via the decoder (e.g. if a codec's support for that depth is
    /// unimplemented). It also does not necessarily indicate whether an image is decodable.
    /// Returns `None` if this cannot be determined for a given image (does not mean the
    /// image is invalid).
    pub fn bit_depth(&self) -> Option<u8> {
        run_catching_panics(|| {
            self.inner.as_ref().and_then(|inner| {
                let color_type = inner.original_color_type();
                let channel_count = color_type.channel_count();
                if channel_count == 0 {
                    return None;
                }
                let bits_per_pixel = color_type.bits_per_pixel();
                // Perform division before casting to u8 to prevent overflow.
                let bit_depth = bits_per_pixel / (channel_count as u16);
                u8::try_from(bit_depth).ok()
            })
        })
        .unwrap_or(None)
    }

    /// Returns the strides of the image data produced by this decoder.
    pub fn strides(&self) -> Strides {
        let (channels, width, height) =
            self.sample_layout.as_ref().map_or((0, 0, 0), |layout| layout.strides_cwh());
        Strides { channels, width, height }
    }

    /// Whether this decoder is able to return a series of animated frames.
    pub fn is_animated(&self) -> bool {
        matches!(self.inner, Some(GenericImageDecoder::Gif(_) | GenericImageDecoder::WebP(_)))
    }

    fn collect_bounded_frames<I>(frames_iter: I, max_alloc: Option<u64>) -> Result<Frames, VecU8>
    where
        I: Iterator<Item = image::ImageResult<RustFrame>>,
    {
        let mut collected_frames: Vec<RustFrame> = Vec::new();
        let mut accumulated_bytes: u128 = 0;
        for frame_result in frames_iter {
            let frame = frame_result.map_err(|err| VecU8::from(err.to_string()))?;
            let frame_bytes = frame.buffer().as_raw().len() as u128;
            accumulated_bytes = accumulated_bytes.saturating_add(frame_bytes);
            if let Some(limit) = max_alloc
                && accumulated_bytes > u128::from(limit)
            {
                return Err(VecU8::from(Self::insufficient_memory_message()));
            }
            collected_frames.push(frame);
        }
        Ok(Frames::new(collected_frames))
    }

    /// Returns all animated frames of the image. This only returns data for decoders, where
    /// `is_animated` returns true.
    pub fn all_frames(self) -> Result<Frames, VecU8> {
        let max_alloc = self.max_alloc;
        run_fallible_result(move || {
            let Some(inner) = self.inner else {
                return Err(VecU8::from("Decoder is in invalid, moved out state"));
            };
            if !matches!(inner, GenericImageDecoder::Gif(_) | GenericImageDecoder::WebP(_)) {
                return Err(VecU8::from(
                    "Trying to get animated data on a codec that does not support animation",
                ));
            }
            let (width, height) = inner.dimensions();
            let canvas_bytes = u128::from(width) * u128::from(height) * 4;
            if usize::try_from(canvas_bytes).is_err() || canvas_bytes > isize::MAX as u128 {
                return Err(VecU8::from(Self::insufficient_memory_message()));
            }
            if let Some(limit) = max_alloc
                && canvas_bytes > u128::from(limit)
            {
                return Err(VecU8::from(Self::insufficient_memory_message()));
            }

            match inner {
                GenericImageDecoder::Gif(mut gif_decoder) => {
                    if let Some(limit) = max_alloc {
                        let mut full_limits = image::Limits::default();
                        full_limits.max_alloc = Some(limit);
                        gif_decoder.set_limits(full_limits).map_err(|err| {
                            VecU8::from(format!("Failed to decode image: {:?}", err))
                        })?;
                    }
                    Self::collect_bounded_frames(gif_decoder.into_frames(), max_alloc)
                }
                GenericImageDecoder::WebP(webp_decoder) => {
                    Self::collect_bounded_frames(webp_decoder.into_frames(), max_alloc)
                }
                _ => Err(VecU8::from(
                    "Trying to get animated data on a codec that does not support animation",
                )),
            }
        })
    }

    fn get_metadata<F>(&mut self, metadata_fn: F) -> Result<Option<VecU8>, VecU8>
    where
        F: FnOnce(&mut GenericImageDecoder) -> image::ImageResult<Option<Vec<u8>>>,
    {
        let max_alloc = self.max_alloc;
        match run_catching_panics(|| {
            let Some(ref mut inner) = self.inner else {
                return Err(VecU8::from("Reader in illegal moved-out state"));
            };

            match metadata_fn(inner) {
                Ok(Some(metadata)) => {
                    if let Some(limit) = max_alloc
                        && (metadata.len() as u64) > limit
                    {
                        return Err(VecU8::from(Self::insufficient_memory_message()));
                    }
                    Ok(Some(metadata.into()))
                }
                Ok(None) => Ok(None),
                Err(err) => Err(VecU8::from(err.to_string())),
            }
        }) {
            Ok(result) => result,
            Err(formatted_error) => Err(formatted_error.into()),
        }
    }

    /// Returns the ICC color profile embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    pub fn icc_profile(&mut self) -> Result<Option<VecU8>, VecU8> {
        self.get_metadata(|inner| inner.icc_profile())
    }

    /// Returns the Exif metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    pub fn exif_metadata(&mut self) -> Result<Option<VecU8>, VecU8> {
        self.get_metadata(|inner| inner.exif_metadata())
    }

    /// Returns the XMP metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    pub fn xmp_metadata(&mut self) -> Result<Option<VecU8>, VecU8> {
        self.get_metadata(|inner| inner.xmp_metadata())
    }

    /// Returns the IPTC metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    pub fn iptc_metadata(&mut self) -> Result<Option<VecU8>, VecU8> {
        self.get_metadata(|inner| inner.iptc_metadata())
    }

    /// Sets the background color for transparent pixels if available. Since this
    /// can decode to multitude of different bit-depths, this allows overriding for
    /// 8-bit and 16-bit respectively. This leaves the input untouched if no alpha
    /// channel is present.
    pub fn set_background_sustitution(&mut self, rgba8: u32, rgba16: u64) {
        let bytes_u8 = rgba8.to_le_bytes();
        let color_u8 = RustRgba([bytes_u8[0], bytes_u8[1], bytes_u8[2], bytes_u8[3]]);

        let bytes_u16 = rgba16.to_le_bytes();
        let color_u16 = RustRgba([
            u16::from_le_bytes([bytes_u16[0], bytes_u16[1]]),
            u16::from_le_bytes([bytes_u16[2], bytes_u16[3]]),
            u16::from_le_bytes([bytes_u16[4], bytes_u16[5]]),
            u16::from_le_bytes([bytes_u16[6], bytes_u16[7]]),
        ]);
        self.background_subs = Some((color_u8, color_u16));
    }

    /// Sets the maximum allocation limit in bytes for decoding operations.
    pub fn set_limits(&mut self, max_alloc: u64) -> Status {
        self.max_alloc = Some(max_alloc);
        match run_catching_panics(|| {
            let Some(inner) = &mut self.inner else {
                return internal_error("ImageDecoder is in invalid, moved out state");
            };

            let (width, height) = inner.dimensions();
            let bytes_per_pixel = u64::from(inner.color_type().bytes_per_pixel()).max(1);
            let output_bytes = u128::from(width) * u128::from(height) * u128::from(bytes_per_pixel);

            let mut limits = image::Limits::default();
            match inner {
                GenericImageDecoder::Gif(_) => {
                    let remaining = u128::from(max_alloc).saturating_sub(output_bytes);
                    limits.max_alloc = Some(u64::try_from(remaining).unwrap_or(0));
                }
                GenericImageDecoder::Jpeg(_) => {
                    limits.max_alloc = Some(max_alloc);
                    if width == 0 && height == 0 {
                        let max_dim = (max_alloc / bytes_per_pixel).min(u64::from(u32::MAX)) as u32;
                        limits.max_image_width = Some(max_dim);
                        limits.max_image_height = Some(max_dim);
                    } else if height == 0 {
                        let denom = u128::from(width) * u128::from(bytes_per_pixel);
                        let max_height =
                            (u128::from(max_alloc) / denom).min(u128::from(u32::MAX)) as u32;
                        limits.max_image_height = Some(max_height);
                    } else if width == 0 {
                        let denom = u128::from(height) * u128::from(bytes_per_pixel);
                        let max_width =
                            (u128::from(max_alloc) / denom).min(u128::from(u32::MAX)) as u32;
                        limits.max_image_width = Some(max_width);
                    }
                }
                _ => {
                    limits.max_alloc = Some(max_alloc);
                }
            }

            if let Err(err) = inner.set_limits(limits) {
                return invalid_argument_error(format!("Failed to set limits: {:?}", err));
            }
            ok()
        }) {
            Ok(status) => status,
            Err(formatted_error) => internal_error(formatted_error),
        }
    }

    fn for_each_u16_pixel_chunk_mut<F>(
        buf: &mut [u8],
        channels: usize,
        mut callback: F,
    ) -> Result<(), Status>
    where
        F: FnMut(&mut [u16]),
    {
        let bytes_per_pixel = channels * std::mem::size_of::<u16>();
        if !buf.len().is_multiple_of(bytes_per_pixel) {
            return Err(invalid_argument_error(format!(
                "Pixel buffer length ({}) is not a multiple of {bytes_per_pixel}",
                buf.len(),
            )));
        }
        if buf.is_empty() {
            return Ok(());
        }
        match bytemuck::try_cast_slice_mut::<u8, u16>(buf) {
            Ok(aligned_slice) => {
                callback(aligned_slice);
                Ok(())
            }
            Err(bytemuck::PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                const CHUNK_U16S: usize = 256;
                debug_assert!(
                    CHUNK_U16S.is_multiple_of(channels),
                    "256 u16 samples must be divisible by pixel channel count ({channels})"
                );
                let mut scratch = [0u16; CHUNK_U16S];
                for byte_chunk in buf.chunks_mut(CHUNK_U16S * std::mem::size_of::<u16>()) {
                    let u16_len = byte_chunk.len() / std::mem::size_of::<u16>();
                    let scratch_slice = &mut scratch[..u16_len];
                    for (dst, src) in scratch_slice.iter_mut().zip(byte_chunk.chunks_exact(2)) {
                        *dst = u16::from_ne_bytes([src[0], src[1]]);
                    }
                    callback(scratch_slice);
                    for (dst, src) in byte_chunk.chunks_exact_mut(2).zip(scratch_slice.iter()) {
                        dst.copy_from_slice(&src.to_ne_bytes());
                    }
                }
                Ok(())
            }
            Err(err) => {
                Err(invalid_argument_error(format!("Invalid u16 pixel buffer layout: {err}")))
            }
        }
    }

    fn for_each_f32_pixel_chunk_mut<F>(
        buf: &mut [u8],
        channels: usize,
        mut callback: F,
    ) -> Result<(), Status>
    where
        F: FnMut(&mut [f32]),
    {
        let bytes_per_pixel = channels * std::mem::size_of::<f32>();
        if !buf.len().is_multiple_of(bytes_per_pixel) {
            return Err(invalid_argument_error(format!(
                "Pixel buffer length ({}) is not a multiple of {bytes_per_pixel}",
                buf.len(),
            )));
        }
        if buf.is_empty() {
            return Ok(());
        }
        match bytemuck::try_cast_slice_mut::<u8, f32>(buf) {
            Ok(aligned_slice) => {
                callback(aligned_slice);
                Ok(())
            }
            Err(bytemuck::PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                const CHUNK_F32S: usize = 128;
                debug_assert!(
                    CHUNK_F32S.is_multiple_of(channels),
                    "128 f32 samples must be divisible by pixel channel count ({channels})"
                );
                let mut scratch = [0.0f32; CHUNK_F32S];
                for byte_chunk in buf.chunks_mut(CHUNK_F32S * std::mem::size_of::<f32>()) {
                    let f32_len = byte_chunk.len() / std::mem::size_of::<f32>();
                    let scratch_slice = &mut scratch[..f32_len];
                    for (dst, src) in scratch_slice.iter_mut().zip(byte_chunk.chunks_exact(4)) {
                        *dst = f32::from_ne_bytes([src[0], src[1], src[2], src[3]]);
                    }
                    callback(scratch_slice);
                    for (dst, src) in byte_chunk.chunks_exact_mut(4).zip(scratch_slice.iter()) {
                        dst.copy_from_slice(&src.to_ne_bytes());
                    }
                }
                Ok(())
            }
            Err(err) => {
                Err(invalid_argument_error(format!("Invalid f32 pixel buffer layout: {err}")))
            }
        }
    }

    /// Performs background alpha blending on decoded pixel bytes.
    fn perform_color_correction(
        buf: &mut [u8],
        background_subs: (RustRgba<u8>, RustRgba<u16>),
        color_type: RustColorType,
    ) -> Result<(), Status> {
        let (color_u8, color_u16) = background_subs;
        match color_type {
            RustColorType::Rgba8 => {
                blend_pixels_in_place(buf, color_u8);
                Ok(())
            }
            RustColorType::Rgba16 => Self::for_each_u16_pixel_chunk_mut(buf, 4, |u16_chunk| {
                blend_pixels_in_place(u16_chunk, color_u16);
            }),
            RustColorType::La8 => {
                let gray = image::LumaA([color_u8.0[1], color_u8.0[3]]);
                blend_pixels_in_place(buf, gray);
                Ok(())
            }
            RustColorType::La16 => {
                let gray = image::LumaA([color_u16.0[1], color_u16.0[3]]);
                Self::for_each_u16_pixel_chunk_mut(buf, 2, |u16_chunk| {
                    blend_pixels_in_place(u16_chunk, gray);
                })
            }
            _ => Ok(()),
        }
    }

    fn premultiply_alpha_bytes(buf: &mut [u8], color_type: RustColorType) -> Result<(), Status> {
        match color_type {
            RustColorType::Rgba8 => {
                pic_scale_safe::premultiply_rgba8(buf);
                Ok(())
            }
            RustColorType::La8 => {
                pic_scale_safe::premultiply_la8(buf);
                Ok(())
            }
            RustColorType::Rgba16 => Self::for_each_u16_pixel_chunk_mut(buf, 4, |u16_chunk| {
                pic_scale_safe::premultiply_rgba16(u16_chunk, 16);
            }),
            RustColorType::La16 => Self::for_each_u16_pixel_chunk_mut(buf, 2, |u16_chunk| {
                pic_scale_safe::premultiply_la16(u16_chunk, 16);
            }),
            RustColorType::Rgba32F => Self::for_each_f32_pixel_chunk_mut(buf, 4, |f32_chunk| {
                pic_scale_safe::premultiply_rgba_f32(f32_chunk);
            }),
            _ => Ok(()),
        }
    }
}
