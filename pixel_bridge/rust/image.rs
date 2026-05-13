use crate::{
    make_option_type, make_result_type,
    reader::{Format, ReadSeek},
    vec_u8::VecU8,
    OptionU8,
};
use image::{
    codecs::bmp::BmpDecoder as RustBmpDecoder, codecs::gif::GifDecoder as RustGifDecoder,
    codecs::ico::IcoDecoder as RustIcoDecoder, codecs::jpeg::JpegDecoder as RustJpegDecoder,
    codecs::png::PngDecoder as RustPngDecoder, codecs::tiff::TiffDecoder as RustTiffDecoder,
    codecs::webp::WebPDecoder as RustWebPDecoder, flat::SampleLayout as RustSampleLayout,
    AnimationDecoder, ColorType as RustColorType, ExtendedColorType as RustExtendedColorType,
    Frame as RustFrame, ImageBuffer as RustImageBuffer, ImageDecoder as RustImageDecoder,
    Rgba as RustRgba,
};
use log_rs::warn;
use std::collections::VecDeque;
use std::fmt;

pub(crate) enum GenericImageDecoder {
    Png(Box<RustPngDecoder<Box<dyn ReadSeek>>>),
    Gif(Box<RustGifDecoder<Box<dyn ReadSeek>>>),
    Jpeg(Box<RustJpegDecoder<Box<dyn ReadSeek>>>),
    WebP(Box<RustWebPDecoder<Box<dyn ReadSeek>>>),
    Tiff(Box<RustTiffDecoder<Box<dyn ReadSeek>>>),
    Bmp(Box<RustBmpDecoder<Box<dyn ReadSeek>>>),
    Ico(Box<RustIcoDecoder<Box<dyn ReadSeek>>>),
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

    fn extended_xmp_metadata(&mut self) -> image::ImageResult<Option<(String, Vec<u8>)>> {
        call_generic_decoder_mut!(self, decoder, decoder.extended_xmp_metadata())
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
}

impl std::fmt::Debug for ImageDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageDecoder")
    }
}

// NOTE: b/367916605 - Change once Result is supported by Crubit.
make_result_type!(ImageDecoder);
// NOTE: b/367916605 - Change once Option is supported by Crubit.
make_option_type!(VecU8, OptionVecU8);
// NOTE: b/367916605 - Change once Option is supported by Crubit.
make_option_type!(u64, OptionU64);
// NOTE: b/367916605 - Change once Result is supported by Crubit.
make_result_type!(OptionVecU8);

/// Which Rust integer type backs the value of a pixel.
#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
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
#[derive(Default, Copy, Clone, Debug)]
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

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame").finish()
    }
}

/// A collection of animated image frames.
#[derive(Default)]
pub struct Frames {
    inner: VecDeque<RustFrame>,
}

impl fmt::Debug for Frames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frames").finish()
    }
}

// NOTE: b/367916605 - Change once Result is supported by Crubit.
make_result_type!(Frames);
// NOTE: b/367916605 - Change once Option is supported by Crubit.
make_option_type!(Frame);

impl Frame {
    fn new(frame: RustFrame) -> Self {
        Self { inner: Some(frame) }
    }

    /// Delay of the current frame in ms.
    pub fn curr_delay_ms(&self) -> u64 {
        match self.inner {
            Some(ref frame) => {
                let delay = std::time::Duration::from(frame.delay());
                delay.as_millis() as u64
            }
            // We are in a moved out state, return an arbitrary value.
            None => 0,
        }
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
    pub fn curr_frame_and_advance(&mut self) -> OptionFrame {
        let Some(first) = self.inner.pop_front() else {
            return OptionFrame::default();
        };

        Frame::new(first).into()
    }
}

/// Represents chroma subsampling. The values are packed from `(h, v)` ratios
/// as `(h << 4) | v`.
#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub enum ChromaSubsampling {
    #[default]
    Unknown = 0,
    CS444 = 0x11,
    CS422 = 0x21,
    CS420 = 0x22,
    CS411 = 0x41,
    CS410 = 0x42,
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

fn run_catching_panics<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> T,
{
    // b/372147306 - The image crate can panic on malformed input.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(res) => Ok(res),
        Err(err) => {
            let formatted_error = format_panic(err);
            warn!(every_n_sec = 3, "Rust panic: {}", formatted_error);
            Err(formatted_error)
        }
    }
}

fn run_read_image<F>(f: F) -> Result<(), status_wrapper::StatusWrapper>
where
    F: FnOnce() -> image::ImageResult<()>,
{
    match run_catching_panics(f) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            Err(status::invalid_argument(format!("Failed to decode image: {:?}", err)).into())
        }
        Err(formatted_error) => Err(status::internal(formatted_error).into()),
    }
}

fn run_fallible_result<F>(f: F) -> ResultFrames
where
    F: FnOnce() -> ResultFrames,
{
    match run_catching_panics(f) {
        Ok(res) => res,
        Err(formatted_error) => ResultFrames::from_err(formatted_error),
    }
}

impl ImageDecoder {
    /// Creates a new `ImageDecoder`.
    pub(crate) fn new(rust_decoder: GenericImageDecoder) -> Self {
        let (width, height) = rust_decoder.dimensions();
        Self {
            sample_layout: match &rust_decoder.color_type() {
                RustColorType::L16 | RustColorType::L8 => {
                    Some(RustSampleLayout::row_major_packed(1, width, height))
                }
                RustColorType::La16 | RustColorType::La8 => {
                    Some(RustSampleLayout::row_major_packed(2, width, height))
                }
                RustColorType::Rgb32F | RustColorType::Rgb16 | RustColorType::Rgb8 => {
                    Some(RustSampleLayout::row_major_packed(3, width, height))
                }
                RustColorType::Rgba32F | RustColorType::Rgba16 | RustColorType::Rgba8 => {
                    Some(RustSampleLayout::row_major_packed(4, width, height))
                }
                _ => None,
            },
            inner: Some(rust_decoder),
            background_subs: None,
            should_premultiply: false,
        }
    }

    /// Read the raw u8 image data into `buf`.
    pub fn read_u8_slice(self, buf: &mut [u8]) -> status_wrapper::StatusWrapper {
        let Some(inner) = self.inner else {
            return status::internal("ImageDecoder is in invalid, moved out state").into();
        };
        let (width, height) = inner.dimensions();
        let color_type = inner.color_type();

        match run_read_image(|| inner.read_image(buf)) {
            Ok(()) => {
                if let Some(background_subs) = self.background_subs {
                    Self::perform_color_correction(buf, background_subs, color_type, width, height);
                }
                if self.should_premultiply {
                    match color_type {
                        RustColorType::Rgba8 => pic_scale_safe::premultiply_rgba8(buf),
                        RustColorType::La8 => pic_scale_safe::premultiply_la8(buf),
                        _ => {}
                    }
                }
                status::OkStatus().into()
            }
            Err(status) => status,
        }
    }

    /// Read the raw u16 image data into `buf`.
    pub fn read_u16_slice(self, buf: &mut [u16]) -> status_wrapper::StatusWrapper {
        let Some(inner) = self.inner else {
            return status::internal("ImageDecoder is in invalid, moved out state").into();
        };
        let (width, height) = inner.dimensions();
        let color_type = inner.color_type();
        let byte_buf: &mut [u8] = bytemuck::cast_slice_mut(buf);

        match run_read_image(|| inner.read_image(byte_buf)) {
            Ok(()) => {
                if let Some(background_subs) = self.background_subs {
                    Self::perform_color_correction(
                        byte_buf,
                        background_subs,
                        color_type,
                        width,
                        height,
                    );
                }
                if self.should_premultiply {
                    match color_type {
                        RustColorType::Rgba16 => pic_scale_safe::premultiply_rgba16(buf, 16),
                        RustColorType::La16 => pic_scale_safe::premultiply_la16(buf, 16),
                        _ => {}
                    }
                }
                status::OkStatus().into()
            }
            Err(status) => status,
        }
    }

    /// Read the raw f32 image data into `buf`.
    pub fn read_f32_slice(self, buf: &mut [f32]) -> status_wrapper::StatusWrapper {
        let Some(inner) = self.inner else {
            return status::internal("ImageDecoder is in invalid, moved out state").into();
        };
        let color_type = inner.color_type();
        let byte_buf: &mut [u8] = bytemuck::cast_slice_mut(buf);

        match run_read_image(|| inner.read_image(byte_buf)) {
            Ok(()) => {
                if self.should_premultiply && color_type == RustColorType::Rgba32F {
                    pic_scale_safe::premultiply_rgba_f32(buf);
                }
                status::OkStatus().into()
            }
            Err(status) => status,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.inner.as_ref().map_or(0, |inner| inner.total_bytes())
    }

    /// Returns the width of the image data produced by this decoder.
    pub fn width(&self) -> usize {
        self.inner.as_ref().map_or(0, |inner| inner.dimensions().0 as usize)
    }

    /// Returns the height of the image data produced by this decoder.
    pub fn height(&self) -> usize {
        self.inner.as_ref().map_or(0, |inner| inner.dimensions().1 as usize)
    }

    /// Returns the color type of the image data produced by this decoder.
    pub fn color_type(&self) -> ColorType {
        self.inner.as_ref().map_or(Default::default(), |inner| ColorType::from(inner.color_type()))
    }

    /// Returns the PixelType of the image data produced by this decoder.
    pub fn pixel_type(&self) -> PixelType {
        self.inner.as_ref().map_or(Default::default(), |inner| PixelType::from(inner.color_type()))
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
        }
    }

    /// Whether the image is originally a CMYK image (this class will convert
    /// it and return an RGB anyways).
    pub fn is_cmyk(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| {
            // The zune_image lib returns YCCK for a lot of images that are CMYK in libjpeg_turbo.
            // For YCCK, it returns `ExtendedColorType::Unknown(253)`.
            matches!(
                inner.original_color_type(),
                RustExtendedColorType::Cmyk8 | RustExtendedColorType::Cmyk16
            )
        })
    }

    /// Whether the image originally used an indexed color palette.
    ///
    /// Note: This is currently only relevant for PNG images, where the `image` crate uses
    /// `ExtendedColorType::Unknown(u8)` to represent indexed color types.
    pub fn has_palette(&self) -> bool {
        match self.format() {
            Format::Png => self.inner.as_ref().is_some_and(|inner| {
                matches!(inner.original_color_type(), RustExtendedColorType::Unknown(_))
            }),
            _ => false,
        }
    }

    /// Returns the bit depth of an image. Note that this only indicates the depth as
    /// encoded and may not necessarily indicate the bit depth at which an image would
    /// be decoded at via the decoder (e.g. if a codec's support for that depth is
    /// unimplemented). It also does not necessarily indicate whether an image is decodable.
    /// Returns `None` if this cannot be determined for a given image (does not mean the
    /// image is invalid).
    pub fn bit_depth(&self) -> OptionU8 {
        self.inner
            .as_ref()
            .and_then(|inner| {
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
            .into()
    }

    /// Returns the chroma subsampling of the image data produced by this decoder.
    ///
    /// This is only supported for JPEG images.
    pub fn chroma_subsampling(&self) -> ChromaSubsampling {
        match &self.inner {
            Some(GenericImageDecoder::Jpeg(decoder)) => match decoder.chroma_subsampling() {
                Ok(Some((1, 1))) => ChromaSubsampling::CS444,
                Ok(Some((2, 1))) => ChromaSubsampling::CS422,
                Ok(Some((2, 2))) => ChromaSubsampling::CS420,
                Ok(Some((4, 1))) => ChromaSubsampling::CS411,
                Ok(Some((4, 2))) => ChromaSubsampling::CS410,
                _ => ChromaSubsampling::Unknown,
            },
            _ => ChromaSubsampling::Unknown,
        }
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

    /// Returns all animated frames of the image. This only returns data for decoders, where
    /// `is_animated` returns true.
    pub fn all_frames(self) -> ResultFrames {
        run_fallible_result(move || match self.inner {
            Some(GenericImageDecoder::Gif(gif_decoder)) => {
                match gif_decoder.into_frames().collect_frames() {
                    Ok(frames) => ResultFrames::from_ok(Frames::new(frames)),
                    Err(err) => ResultFrames::from_err(err.to_string()),
                }
            }
            Some(GenericImageDecoder::WebP(webp_decoder)) => {
                match webp_decoder.into_frames().collect_frames() {
                    Ok(frames) => ResultFrames::from_ok(Frames::new(frames)),
                    Err(err) => ResultFrames::from_err(err.to_string()),
                }
            }
            Some(_) => ResultFrames::from_err(
                "Trying to get animated data on a codec that does not support animation",
            ),
            None => ResultFrames::from_err("Decoder is in invalid, moved out state"),
        })
    }

    fn get_metadata<F>(&mut self, metadata_fn: F) -> ResultOptionVecU8
    where
        F: FnOnce(&mut GenericImageDecoder) -> image::ImageResult<Option<Vec<u8>>>,
    {
        let Some(ref mut inner) = self.inner else {
            return ResultOptionVecU8::from_err("Reader in illegal moved-out state");
        };

        match metadata_fn(inner) {
            Ok(Some(metadata)) => {
                ResultOptionVecU8::from_ok(OptionVecU8::from_some(metadata.into()))
            }
            Ok(None) => ResultOptionVecU8::from_ok(None.into()),
            Err(err) => ResultOptionVecU8::from_err(err.to_string()),
        }
    }

    /// Returns the ICC color profile embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    pub fn icc_profile(&mut self) -> ResultOptionVecU8 {
        self.get_metadata(|inner| inner.icc_profile())
    }

    /// Returns the Exif metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    /// NOTE: b/462340924 - Does not always detect Exif metadata in comparison to Twix
    /// implementation.
    pub fn exif_metadata(&mut self) -> ResultOptionVecU8 {
        self.get_metadata(|inner| inner.exif_metadata())
    }

    /// Returns the XMP metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    /// NOTE: b/462340924 - Does not always detect XMP metadata in comparison to Twix
    /// implementation.
    pub fn xmp_metadata(&mut self) -> ResultOptionVecU8 {
        self.get_metadata(|inner| inner.xmp_metadata())
    }

    /// Returns the Extended XMP GUID embedded in the image, or Ok(None) if the
    /// image does not have one.
    pub fn extended_xmp_guid(&mut self) -> ResultOptionVecU8 {
        let Some(ref mut inner) = self.inner else {
            return ResultOptionVecU8::from_err("Reader in illegal moved-out state");
        };

        match inner.extended_xmp_metadata() {
            Ok(Some((guid, _))) => ResultOptionVecU8::from_ok(OptionVecU8::from_some(guid.into())),
            Ok(None) => ResultOptionVecU8::from_ok(None.into()),
            Err(err) => ResultOptionVecU8::from_err(err.to_string()),
        }
    }

    /// Returns the Extended XMP metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    pub fn extended_xmp_metadata(&mut self) -> ResultOptionVecU8 {
        let Some(ref mut inner) = self.inner else {
            return ResultOptionVecU8::from_err("Reader in illegal moved-out state");
        };

        match inner.extended_xmp_metadata() {
            Ok(Some((_, metadata))) => {
                ResultOptionVecU8::from_ok(OptionVecU8::from_some(metadata.into()))
            }
            Ok(None) => ResultOptionVecU8::from_ok(None.into()),
            Err(err) => ResultOptionVecU8::from_err(err.to_string()),
        }
    }

    /// Returns the IPTC metadata embedded in the image, or Ok(None) if the
    /// image does not have one.
    /// For formats that don’t support embedded profiles this function should
    /// always return Ok(None).
    /// NOTE: b/462340924 - Does not always detect IPTC metadata in comparison to Twix
    /// implementation, and report different bytes as well.
    pub fn iptc_metadata(&mut self) -> ResultOptionVecU8 {
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

    pub fn set_limits(&mut self, max_alloc: u64) -> status_wrapper::StatusWrapper {
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(max_alloc);

        if let Some(inner) = &mut self.inner {
            if let Err(e) = inner.set_limits(limits) {
                return status::invalid_argument(format!("Failed to set limits: {:?}", e)).into();
            }
        }
        status::OkStatus().into()
    }

    /// Performs the actual color correction.
    fn perform_color_correction(
        buf: &mut [u8],
        background_subs: (RustRgba<u8>, RustRgba<u16>),
        color_type: RustColorType,
        width: u32,
        height: u32,
    ) {
        let (color_u8, color_u16) = background_subs;
        match color_type {
            RustColorType::Rgba8 => {
                Self::perform_color_correction_inner(buf, color_u8, width, height);
            }
            RustColorType::Rgba16 => {
                let buf: &mut [u16] = bytemuck::cast_slice_mut(buf);
                Self::perform_color_correction_inner(buf, color_u16, width, height);
            }
            RustColorType::La8 => {
                // Unknown context for gray=green but this is the current behavior for thumbnailer:
                // google3/third_party/picasa/yt/ytPNGFile.cpp;rcl=614000061;l=809
                let gray = image::LumaA([color_u8.0[1], color_u8.0[3]]);
                Self::perform_color_correction_inner(buf, gray, width, height);
            }
            RustColorType::La16 => {
                let gray = image::LumaA([color_u16.0[1], color_u16.0[3]]);
                let buf: &mut [u16] = bytemuck::cast_slice_mut(buf);
                Self::perform_color_correction_inner(buf, gray, width, height);
            }
            _ => {}
        }
    }

    fn perform_color_correction_inner<P, C>(buf: C, corrected_color: P, width: u32, height: u32)
    where
        P: image::Pixel,
        C: std::ops::Deref<Target = [P::Subpixel]> + std::ops::DerefMut,
    {
        let Some(mut image_buffer) = RustImageBuffer::<P, C>::from_raw(width, height, buf) else {
            return;
        };
        for pixel in image_buffer.pixels_mut() {
            let mut blended = corrected_color;
            blended.blend(pixel);
            *pixel = blended;
        }
    }
}
