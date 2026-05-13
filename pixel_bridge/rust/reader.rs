use crate::{
    image::{GenericImageDecoder, ImageDecoder, ResultImageDecoder},
    make_result_type,
};
use image::{
    codecs::bmp::BmpDecoder as RustBmpDecoder, codecs::gif::GifDecoder as RustGifDecoder,
    codecs::ico::IcoDecoder as RustIcoDecoder, codecs::jpeg::JpegDecoder as RustJpegDecoder,
    codecs::png::PngDecoder as RustPngDecoder, codecs::tiff::TiffDecoder as RustTiffDecoder,
    codecs::webp::WebPDecoder as RustWebPDecoder, ImageFormat, ImageReader as RustImageReader,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek};

/// A helper trait to unify the Read + Seek implementation needed by the `ImageReader`.
pub(crate) trait ReadSeek: Read + BufRead + Seek {}
impl ReadSeek for std::io::BufReader<Cursor<Vec<u8>>> {}
impl ReadSeek for std::io::BufReader<File> {}

/// A multi-format image reader.
///
/// Wraps an input reader to facilitate automatic detection of an image’s format,
/// appropriate decoding method, and dispatches into the set of supported
/// `ImageDecoder` implementations.
#[derive(Default)]
pub struct ImageReader {
    inner: Option<Box<RustImageReader<Box<dyn ReadSeek>>>>,
    jpeg_strict_mode: bool,
    png_ignore_checksums: bool,
}

impl std::fmt::Debug for ImageReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageReader")
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub enum Format {
    // We need a default since this type is otherwise not bridged by Crubit.
    #[default]
    Png,
    Jpeg,
    Gif,
    WebP,
    Tiff,
    Bmp,
    Ico,
}

impl From<Format> for ImageFormat {
    fn from(value: Format) -> ImageFormat {
        match value {
            Format::Png => ImageFormat::Png,
            Format::Jpeg => ImageFormat::Jpeg,
            Format::Gif => ImageFormat::Gif,
            Format::WebP => ImageFormat::WebP,
            Format::Tiff => ImageFormat::Tiff,
            Format::Bmp => ImageFormat::Bmp,
            Format::Ico => ImageFormat::Ico,
        }
    }
}

// A `ResultImageReader` that represents a Crubit-understandable `Result<ImageReader>`.
// NOTE(b/367916605): Swap this with a real `Result<ImageReader>` once available.
make_result_type!(ImageReader);

impl ImageReader {
    /// Creates a new `ImageReader` with the provided input buffer.
    pub fn new_in_memory(input: &[u8]) -> Self {
        // We copy the bytes here as this is the most easy case for safety (lifetimes over FFI
        // are hard). If we ever see that this has performance issues, this is a good place to
        // start.
        let bytes_reader = std::io::BufReader::new(std::io::Cursor::new(input.to_vec()));
        Self {
            inner: Some(Box::new(RustImageReader::new(Box::new(bytes_reader)))),
            jpeg_strict_mode: true,
            png_ignore_checksums: false,
        }
    }

    /// Creates a new `ImageReader` reading from the provided file.
    pub fn new_from_file(path: &[u8]) -> ResultImageReader {
        let filepath = match str::from_utf8(path) {
            Ok(path) => path,
            Err(err) => return ResultImageReader::from_err(err.to_string()),
        };

        match std::fs::File::open(filepath) {
            Ok(file) => Ok(Self {
                inner: Some(Box::new(RustImageReader::new(
                    Box::new(BufReader::new(file)) as Box<dyn ReadSeek>
                ))),
                jpeg_strict_mode: true,
                png_ignore_checksums: false,
            })
            .into(),
            Err(err) => ResultImageReader::from_err(err.to_string()),
        }
    }

    /// Supply the format as which to interpret the read image.
    pub fn set_format(&mut self, format: Format) {
        let Some(ref mut inner) = self.inner else {
            return;
        };
        inner.set_format(format.into())
    }

    /// Supply whether to use strict mode for the JPEG decoder.
    pub fn set_jpeg_strict_mode(&mut self, strict_mode: bool) {
        self.jpeg_strict_mode = strict_mode;
    }

    /// Supply whether to ignore checksums for the PNG decoder.
    pub fn set_png_ignore_checksums(&mut self, ignore_checksums: bool) {
        self.png_ignore_checksums = ignore_checksums;
    }

    /// Read the image.
    ///
    /// Uses the current format to construct the correct reader for the format.
    pub fn into_decoder(self) -> ResultImageDecoder {
        let Some(inner) = self.inner else {
            return ResultImageDecoder::from_err("Reader in illegal moved-out state");
        };

        // If we already know the format, we do not need to guess.
        let inner = if inner.format().is_some() {
            *inner
        } else {
            match inner.with_guessed_format() {
                Ok(inner) => inner,
                Err(err) => return ResultImageDecoder::from_err(err.to_string()),
            }
        };
        // Delegate to non-inlined helper functions to reduce the stack frame size by 4k.
        // See WorksWithSmallStack in photos/editing/twix/codec/decode_image_safe_test.cc.
        match inner.format() {
            Some(ImageFormat::Png) => png(inner, self.png_ignore_checksums),
            Some(ImageFormat::Jpeg) => jpeg(inner, self.jpeg_strict_mode),
            Some(ImageFormat::WebP) => webp(inner),
            Some(ImageFormat::Gif) => gif(inner),
            Some(ImageFormat::Tiff) => tiff(inner),
            Some(ImageFormat::Bmp) => bmp(inner),
            Some(ImageFormat::Ico) => ico(inner),
            _ => Err("unsupported image format".to_string()),
        }
        .into()
    }
}

#[inline(never)]
fn png(
    d: RustImageReader<Box<dyn ReadSeek>>,
    ignore_checksums: bool,
) -> Result<ImageDecoder, String> {
    let decoder = RustPngDecoder::with_limits_and_options(
        d.into_inner(),
        image::Limits::no_limits(),
        ignore_checksums,
    )
    .map_err(|e| e.to_string())?;
    Ok(ImageDecoder::new(GenericImageDecoder::Png(Box::new(decoder))))
}

#[inline(never)]
fn jpeg(d: RustImageReader<Box<dyn ReadSeek>>, strict_mode: bool) -> Result<ImageDecoder, String> {
    let mut decoder = RustJpegDecoder::new(d.into_inner()).map_err(|e| e.to_string())?;
    decoder.set_strict_mode(strict_mode);
    Ok(ImageDecoder::new(GenericImageDecoder::Jpeg(Box::new(decoder))))
}

#[inline(never)]
fn webp(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    Ok(ImageDecoder::new(GenericImageDecoder::WebP(Box::new(
        RustWebPDecoder::new(d.into_inner()).map_err(|e| e.to_string())?,
    ))))
}

#[inline(never)]
fn gif(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    Ok(ImageDecoder::new(GenericImageDecoder::Gif(Box::new(
        RustGifDecoder::new(d.into_inner()).map_err(|e| e.to_string())?,
    ))))
}

#[inline(never)]
fn tiff(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    let mut reader = d.into_inner();
    let mut should_premultiply = false;

    // Check if we should premultiply based on ExtraSamples tag
    {
        let tiff_decoder_result = tiff::decoder::Decoder::new(&mut *reader);
        if let Ok(mut decoder) = tiff_decoder_result
            && let Ok(Some(extra_samples)) =
                decoder.find_tag_unsigned_vec::<u16>(tiff::tags::Tag::ExtraSamples)
            && extra_samples.first() == Some(&2)
        {
            // 2 = Unassociated Alpha
            should_premultiply = true;
        }
    }

    // Seek back to start before passing to image crate decoder
    reader.seek(std::io::SeekFrom::Start(0)).map_err(|e| e.to_string())?;

    let mut image_decoder = ImageDecoder::new(GenericImageDecoder::Tiff(Box::new(
        RustTiffDecoder::new(reader).map_err(|e| e.to_string())?,
    )));
    image_decoder.should_premultiply = should_premultiply;
    Ok(image_decoder)
}

#[inline(never)]
fn bmp(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    Ok(ImageDecoder::new(GenericImageDecoder::Bmp(Box::new(
        RustBmpDecoder::new(d.into_inner()).map_err(|e| e.to_string())?,
    ))))
}

#[inline(never)]
fn ico(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    Ok(ImageDecoder::new(GenericImageDecoder::Ico(Box::new(
        RustIcoDecoder::new(d.into_inner()).map_err(|e| e.to_string())?,
    ))))
}
