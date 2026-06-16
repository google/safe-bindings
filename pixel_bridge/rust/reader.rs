use crate::{
    image::{GenericImageDecoder, ImageDecoder},
    vec_u8::VecU8,
};
use image::{
    codecs::bmp::BmpDecoder as RustBmpDecoder, codecs::gif::GifDecoder as RustGifDecoder,
    codecs::ico::IcoDecoder as RustIcoDecoder, codecs::jpeg::JpegDecoder as RustJpegDecoder,
    codecs::png::PngDecoder as RustPngDecoder, codecs::tiff::TiffDecoder as RustTiffDecoder,
    codecs::webp::WebPDecoder as RustWebPDecoder, ImageFormat, ImageReader as RustImageReader,
};
use png as png_crate;
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

impl ImageReader {
    /// Creates a new `ImageReader` with the provided input buffer.
    pub fn new_in_memory(input: &[u8]) -> Self {
        // We copy the bytes here as this is the most easy case for safety (lifetimes over FFI
        // are hard). If we ever see that this has performance issues, this is a good place to
        // start.
        let bytes_reader = std::io::BufReader::new(std::io::Cursor::new(input.to_vec()));
        Self {
            inner: Some(Box::new(RustImageReader::new(Box::new(bytes_reader)))),
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
            }),
            Err(err) => Err(err.to_string().into()),
        }
    }

    /// Supply the format as which to interpret the read image.
    pub fn set_format(&mut self, format: Format) {
        let Some(ref mut inner) = self.inner else {
            return;
        };
        inner.set_format(format.into())
    }

    /// Read the image.
    ///
    /// Uses the current format to construct the correct reader for the format.
    pub fn into_decoder(self) -> Result<ImageDecoder, VecU8> {
        let Some(inner) = self.inner else {
            return Err("Reader in illegal moved-out state".into());
        };

        // If we already know the format, we do not need to guess.
        let inner = if inner.format().is_some() {
            *inner
        } else {
            match inner.with_guessed_format() {
                Ok(inner) => inner,
                Err(err) => return Err(err.to_string().into()),
            }
        };
        // Delegate to non-inlined helper functions to reduce the stack frame size by 4k.
        match inner.format() {
            Some(ImageFormat::Png) => png(inner),
            Some(ImageFormat::Jpeg) => jpeg(inner),
            Some(ImageFormat::WebP) => webp(inner),
            Some(ImageFormat::Gif) => gif(inner),
            Some(ImageFormat::Tiff) => tiff(inner),
            Some(ImageFormat::Bmp) => bmp(inner),
            Some(ImageFormat::Ico) => ico(inner),
            _ => Err("unsupported image format".to_string()),
        }
        .map_err(|err| err.into())
    }
}

#[inline(never)]
fn png(
    d: RustImageReader<Box<dyn ReadSeek>>,
) -> Result<ImageDecoder, String> {
    let decoder = RustPngDecoder::new(d.into_inner()).map_err(|e| e.to_string())?;
    Ok(ImageDecoder::new(GenericImageDecoder::Png(Box::new(decoder))))
}

#[inline(never)]
fn jpeg(
    d: RustImageReader<Box<dyn ReadSeek>>,
) -> Result<ImageDecoder, String> {
    let decoder = RustJpegDecoder::new(d.into_inner()).map_err(|e| e.to_string())?;
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

pub fn extract_png_text_metadata(
    png_data: &[u8],
) -> Result<crate::vec_u8::VecU8, crate::vec_u8::VecU8> {
    let decoder = png_crate::Decoder::new(std::io::Cursor::new(png_data));
    let reader = decoder.read_info().map_err(|e| crate::vec_u8::VecU8::from(e.to_string()))?;
    let info = reader.info();

    let mut serialized = Vec::new();

    let mut append_entry = |key: &str, text: &str| {
        let key_bytes = key.as_bytes();
        let val_bytes = text.as_bytes();
        serialized.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        serialized.extend_from_slice(key_bytes);
        serialized.extend_from_slice(&(val_bytes.len() as u32).to_le_bytes());
        serialized.extend_from_slice(val_bytes);
    };

    for text_chunk in &info.uncompressed_latin1_text {
        append_entry(&text_chunk.keyword, &text_chunk.text);
    }

    for text_chunk in &info.compressed_latin1_text {
        if let Ok(val_str) = text_chunk.get_text() {
            append_entry(&text_chunk.keyword, &val_str);
        }
    }

    for text_chunk in &info.utf8_text {
        if let Ok(val_str) = text_chunk.get_text() {
            append_entry(&text_chunk.keyword, &val_str);
        }
    }

    Ok(crate::vec_u8::VecU8::from(serialized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_with_null_bytes() {
        let mut buffer = Vec::new();
        {
            let mut encoder = png_crate::Encoder::new(&mut buffer, 1, 1);
            encoder.set_color(png_crate::ColorType::Grayscale);
            encoder.set_depth(png_crate::BitDepth::Eight);

            // Add a text chunk with null bytes in the value
            let key = "Owner";
            let val = "G\0O\0O\0G\0L\0E";
            encoder.add_text_chunk(key.to_string(), val.to_string()).unwrap();

            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0]).unwrap();
        }

        let result = extract_png_text_metadata(&buffer).unwrap();
        let serialized = &*result;

        // Under length-prefixed encoding (4-byte LE length), parse keys & values:
        let mut idx = 0;
        let mut parsed = Vec::new();
        while idx + 4 <= serialized.len() {
            let key_len = u32::from_le_bytes(serialized[idx..idx + 4].try_into().unwrap()) as usize;
            idx += 4;
            assert!(idx + key_len <= serialized.len());
            let key = std::str::from_utf8(&serialized[idx..idx + key_len]).unwrap().to_string();
            idx += key_len;

            assert!(idx + 4 <= serialized.len());
            let val_len = u32::from_le_bytes(serialized[idx..idx + 4].try_into().unwrap()) as usize;
            idx += 4;
            assert!(idx + val_len <= serialized.len());
            let val = std::str::from_utf8(&serialized[idx..idx + val_len]).unwrap().to_string();
            idx += val_len;

            parsed.push((key, val));
        }
        assert_eq!(idx, serialized.len());

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "Owner");
        assert_eq!(parsed[0].1, "G\0O\0O\0G\0L\0E");
    }
}
