use crate::{
    image::{GenericImageDecoder, ImageDecoder},
    png::{PngDecoder as RustPngDecoder, PNG_USER_HEIGHT_MAX, PNG_USER_WIDTH_MAX},
    vec_u8::VecU8,
};
use image::{
    codecs::bmp::BmpDecoder as RustBmpDecoder, codecs::gif::GifDecoder as RustGifDecoder,
    codecs::ico::IcoDecoder as RustIcoDecoder, codecs::jpeg::JpegDecoder as RustJpegDecoder,
    codecs::tiff::TiffDecoder as RustTiffDecoder, codecs::webp::WebPDecoder as RustWebPDecoder,
    ImageFormat, ImageReader as RustImageReader,
};
#[cfg(feature = "jxl")]
use jxl::api::{check_signature as check_jxl_signature, ProcessingResult as JxlProcessingResult};
#[cfg(feature = "jxl")]
use jxl_image_rs_integration::JxlDecoder as RustJxlDecoder;
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
    format: Option<Format>,
}

#[cfg(debug_assertions)]
impl std::fmt::Debug for ImageReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageReader")
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(debug_assertions, derive(Debug))]
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
    Jxl,
}

fn has_jxl_signature(reader: &mut dyn ReadSeek) -> std::io::Result<bool> {
    #[cfg(feature = "jxl")]
    {
        Ok(matches!(
            check_jxl_signature(reader.fill_buf()?),
            JxlProcessingResult::Complete { result: Some(_) }
        ))
    }
    #[cfg(not(feature = "jxl"))]
    {
        let _ = reader;
        Ok(false)
    }
}

impl ImageReader {
    /// Creates a new `ImageReader` with the provided input buffer.
    pub fn new_in_memory(input: &[u8]) -> Self {
        // We copy the bytes here as this is the most easy case for safety (lifetimes over FFI
        // are hard). If we ever see that this has performance issues, this is a good place to
        // start.
        let bytes_reader = std::io::BufReader::new(std::io::Cursor::new(input.to_vec()));
        Self {
            inner: Some(Box::new(RustImageReader::new(Box::new(bytes_reader)))),
            format: None,
        }
    }

    /// Creates a new `ImageReader` reading from the provided file.
    pub fn new_from_file(path: &[u8]) -> Result<ImageReader, VecU8> {
        let filepath = match std::str::from_utf8(path) {
            Ok(path) => path,
            Err(err) => return Err(err.to_string().into()),
        };

        match std::fs::File::open(filepath) {
            Ok(file) => Ok(Self {
                inner: Some(Box::new(RustImageReader::new(
                    Box::new(BufReader::new(file)) as Box<dyn ReadSeek>
                ))),
                format: None,
            }),
            Err(err) => Err(err.to_string().into()),
        }
    }

    /// Supply the format as which to interpret the read image.
    pub fn set_format(&mut self, format: Format) {
        self.format = Some(format);
    }

    /// Read the image.
    ///
    /// Uses the current format to construct the correct reader for the format.
    pub fn into_decoder(self) -> Result<ImageDecoder, VecU8> {
        let Some(inner) = self.inner else {
            return Err("Reader in illegal moved-out state".into());
        };

        // If we already know the format, we do not need to guess.
        let (mut inner, format) = if let Some(format) = self.format {
            (*inner, Some(format))
        } else {
            let inner = match inner.with_guessed_format() {
                Ok(inner) => inner,
                Err(err) => return Err(err.to_string().into()),
            };
            let format = match inner.format() {
                Some(ImageFormat::Png) => Some(Format::Png),
                Some(ImageFormat::Jpeg) => Some(Format::Jpeg),
                Some(ImageFormat::WebP) => Some(Format::WebP),
                Some(ImageFormat::Gif) => Some(Format::Gif),
                Some(ImageFormat::Tiff) => Some(Format::Tiff),
                Some(ImageFormat::Bmp) => Some(Format::Bmp),
                Some(ImageFormat::Ico) => Some(Format::Ico),
                _ => None,
            };
            (inner, format)
        };

        let format = match format {
            Some(format) => Some(format),
            None => {
                let mut reader = inner.into_inner();
                let is_jxl =
                    has_jxl_signature(&mut *reader).map_err(|e| VecU8::from(e.to_string()))?;
                inner = RustImageReader::new(reader);
                if is_jxl {
                    Some(Format::Jxl)
                } else {
                    None
                }
            }
        };

        // Delegate to non-inlined helper functions to reduce the stack frame size by 4k.
        match format {
            Some(Format::Png) => png(inner),
            Some(Format::Jpeg) => jpeg(inner),
            Some(Format::WebP) => webp(inner),
            Some(Format::Gif) => gif(inner),
            Some(Format::Tiff) => tiff(inner),
            Some(Format::Bmp) => bmp(inner),
            Some(Format::Ico) => ico(inner),
            Some(Format::Jxl) => jxl(inner),
            None => Err("unsupported image format".to_string()),
        }
        .map_err(|err| err.into())
    }
}

#[inline(never)]
fn png(
    d: RustImageReader<Box<dyn ReadSeek>>,
) -> Result<ImageDecoder, String> {
    let decoder = RustPngDecoder::new(d.into_inner()).map_err(|e|
    // e.to_string())?;
    Ok(ImageDecoder::new(GenericImageDecoder::Png(Box::new(decoder))))
}

#[inline(never)]
fn jpeg(
    d: RustImageReader<Box<dyn ReadSeek>>,
) -> Result<ImageDecoder, String> {
    let decoder = RustJpegDecoder::new(d.into_inner()).map_err(|e|
    // e.to_string())?;
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

/// Maximum number of directory entries we accept in an ICO file.
///
/// The ICO format allows up to `u16::MAX` (65535) entries. We enforce a much smaller cap here to
/// bound the work the decoder performs on untrusted input.
const MAX_ICO_ENTRIES: u16 = 16;

#[inline(never)]
fn ico(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    let mut reader = d.into_inner();
    let start = reader.stream_position().map_err(|e| e.to_string())?;

    // ICONDIR header layout: bytes [0..2] = reserved, [2..4] = type,
    // [4..6] = number of directory entries (little-endian u16).
    let mut header = [0u8; 6];
    reader.read_exact(&mut header).map_err(|e| e.to_string())?;
    let count = u16::from_le_bytes([header[4], header[5]]);

    reader.seek(std::io::SeekFrom::Start(start)).map_err(|e| e.to_string())?;

    if count > MAX_ICO_ENTRIES {
        return Err(format!("ICO file has too many entries: {count} (max {MAX_ICO_ENTRIES})"));
    }

    Ok(ImageDecoder::new(GenericImageDecoder::Ico(Box::new(
        RustIcoDecoder::new(reader).map_err(|e| e.to_string())?,
    ))))
}

#[inline(never)]
fn jxl(d: RustImageReader<Box<dyn ReadSeek>>) -> Result<ImageDecoder, String> {
    #[cfg(feature = "jxl")]
    {
        Ok(ImageDecoder::new(GenericImageDecoder::Jxl(Box::new(
            RustJxlDecoder::new(d.into_inner()).map_err(|e| e.to_string())?,
        ))))
    }
    #[cfg(not(feature = "jxl"))]
    {
        let _ = d;
        Err("JXL support is not enabled".to_string())
    }
}
