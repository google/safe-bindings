use crate::vec_u8::VecU8;
use image::{
    error::{DecodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind},
    ColorType, ExtendedColorType, ImageDecoder, ImageFormat, Limits,
};
use png::text_metadata::{ITXtChunk, ZTXtChunk};
use std::io::{BufRead, Seek, SeekFrom};

// Match the same-named macros in google3/third_party/png/src/pnglibconf.h.
const PNG_USER_CHUNK_MALLOC_MAX: usize = 8_000_000;
pub const PNG_USER_WIDTH_MAX: u32 = 1_000_000;
pub const PNG_USER_HEIGHT_MAX: u32 = 1_000_000;

pub const NUL_LEN: usize = 1;
const COMPRESSION_FLAG_LEN: usize = 1;
const COMPRESSION_METHOD_LEN: usize = 1;

// Match kExifChunkKeyword1 and the other keywords in
// google3/image/codec/pngdecoder.cc. Public so that the C++ tests can name them.
pub const EXIF_CHUNK_KEYWORD_1: &str = "Raw profile type APP1";
pub const EXIF_CHUNK_KEYWORD_2: &str = "Raw profile type exif";
pub const IPTC_CHUNK_KEYWORD_1: &str = "Raw profile type iptc";
pub const IPTC_CHUNK_KEYWORD_2: &str = "Raw profile type 8bim";
pub const XMP_CHUNK_KEYWORD_1: &str = "XML:com.adobe.xmp";
pub const XMP_CHUNK_KEYWORD_2: &str = "Raw profile type xmp";

const EXIF_CHUNK_KEYWORDS: [&str; 2] = [EXIF_CHUNK_KEYWORD_1, EXIF_CHUNK_KEYWORD_2];
const IPTC_CHUNK_KEYWORDS: [&str; 2] = [IPTC_CHUNK_KEYWORD_1, IPTC_CHUNK_KEYWORD_2];
const XMP_CHUNK_KEYWORDS: [&str; 2] = [XMP_CHUNK_KEYWORD_1, XMP_CHUNK_KEYWORD_2];
const EXIF_HEADER: &str = "Exif\x00\x00";

const PNG_SIGNATURE_LEN: u64 = 8;
const CHUNK_LENGTH_LEN: usize = 4;
const CHUNK_TYPE_LEN: usize = 4;
const CHUNK_HEADER_LEN: usize = CHUNK_LENGTH_LEN + CHUNK_TYPE_LEN;
const CHUNK_CRC_LEN: i64 = 4;
const GRAY_TRNS_LEN: usize = 2;
const RGB_TRNS_LEN: usize = 6;
// Matches `PNG_MAX_PALETTE_LENGTH` in google3/third_party/png/src/png.h.
const MAX_TRNS_LEN: usize = 256;
const PLTE_ENTRY_LEN: usize = 3;

#[repr(C)]
#[derive(Default, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct Rgb16 {
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

#[derive(Default)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct PngTextMetadata {
    pub exif: Option<VecU8>,
    pub iptc: Option<VecU8>,
    pub xmp: Option<VecU8>,
}

pub(crate) struct PngDecoder<R: BufRead + Seek> {
    color_type: ColorType,
    reader: png::Reader<R>,
    limits: Limits,
    gray_trns_len: Option<usize>,
    rgb_trns_len: Option<usize>,
}

impl<R: BufRead + Seek> PngDecoder<R> {
    pub fn with_limits_and_options(
        mut r: R,
        limits: Limits,
        ignore_checksums: bool,
    ) -> ImageResult<Self> {
        limits.check_support(&image::LimitSupport::default())?;

        let (gray_trns_len, rgb_trns_len) = pre_idat_trns_lens(&mut r);

        let max_bytes = usize::try_from(limits.max_alloc.unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
        let mut decode_options = png::DecodeOptions::default();
        decode_options.set_ignore_checksums(ignore_checksums);
        let mut decoder = png::Decoder::new_with_options(r, decode_options);
        decoder.set_limits(png::Limits { bytes: max_bytes });
        decoder.set_ignore_text_chunk(false);

        let info = decoder.read_header_info().map_err(from_png_error)?;
        limits.check_dimensions(info.width, info.height)?;

        // By default the PNG decoder scales 16 bpc to 8 bpc; EXPAND expands
        // <8-bit samples and palettes while preserving 16-bit samples.
        decoder.set_transformations(png::Transformations::EXPAND);
        let reader = decoder.read_info().map_err(from_png_error)?;
        let (color_type, bits) = reader.output_color_type();
        let color_type = match (color_type, bits) {
            (png::ColorType::Grayscale, png::BitDepth::Eight) => ColorType::L8,
            (png::ColorType::Grayscale, png::BitDepth::Sixteen) => ColorType::L16,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => ColorType::La8,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => ColorType::La16,
            (png::ColorType::Rgb, png::BitDepth::Eight) => ColorType::Rgb8,
            (png::ColorType::Rgb, png::BitDepth::Sixteen) => ColorType::Rgb16,
            (png::ColorType::Rgba, png::BitDepth::Eight) => ColorType::Rgba8,
            (png::ColorType::Rgba, png::BitDepth::Sixteen) => ColorType::Rgba16,
            (png::ColorType::Grayscale, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::L1))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::La1))
            }
            (png::ColorType::Rgb, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::Rgb1))
            }
            (png::ColorType::Rgba, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::Rgba1))
            }
            (png::ColorType::Grayscale, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::L2))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::La2))
            }
            (png::ColorType::Rgb, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::Rgb2))
            }
            (png::ColorType::Rgba, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::Rgba2))
            }
            (png::ColorType::Grayscale, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::L4))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::La4))
            }
            (png::ColorType::Rgb, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::Rgb4))
            }
            (png::ColorType::Rgba, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::Rgba4))
            }
            (png::ColorType::Indexed, bits) => {
                return Err(unsupported_color(ExtendedColorType::Unknown(bits as u8)))
            }
        };

        Ok(Self { color_type, reader, limits, gray_trns_len, rgb_trns_len })
    }

    /// Returns whether the image has a full alpha channel or a `tRNS` chunk
    /// that `png_handle_tRNS` in google3/third_party/png/src/pngrutil.c keeps.
    /// `png::Decoder` accepts `tRNS` chunks up to 256 bytes for greyscale and
    /// RGB images and `tRNS` chunks longer than the indexable palette for
    /// indexed images, promoting their decoded colour type to alpha.
    pub fn has_valid_transparency(&self) -> bool {
        let info = self.reader.info();
        match info.color_type {
            png::ColorType::GrayscaleAlpha | png::ColorType::Rgba => true,
            png::ColorType::Grayscale => {
                info.trns.is_some() && self.gray_trns_len == Some(GRAY_TRNS_LEN)
            }
            png::ColorType::Rgb => info.trns.is_some() && self.rgb_trns_len == Some(RGB_TRNS_LEN),
            png::ColorType::Indexed => {
                let (Some(plte), Some(trns)) = (info.palette.as_deref(), info.trns.as_deref())
                else {
                    return false;
                };
                libpng_keeps_palette_trns(plte, trns, info.bit_depth)
            }
        }
    }

    /// Returns the raw `bKGD` background colour, or `None` if `bKGD` is absent
    /// or `png_handle_bKGD` in google3/third_party/png/src/pngrutil.c drops it.
    pub fn background_color(&self) -> Option<Rgb16> {
        let info = self.reader.info();
        let bkgd = info.bkgd.as_deref()?;
        let bit_depth = info.bit_depth;

        let [r, g, b] = match (info.color_type, bkgd) {
            (png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha, &[hi, lo]) => {
                [checked_sample(u16::from_be_bytes([hi, lo]), bit_depth)?; 3]
            }
            (png::ColorType::Indexed, &[index]) => {
                indexable_palette(info.palette.as_deref()?, bit_depth)
                    .get(usize::from(index))?
                    .map(u16::from)
            }
            (png::ColorType::Rgb | png::ColorType::Rgba, &[r_hi, r_lo, g_hi, g_lo, b_hi, b_lo]) => {
                [
                    checked_sample(u16::from_be_bytes([r_hi, r_lo]), bit_depth)?,
                    checked_sample(u16::from_be_bytes([g_hi, g_lo]), bit_depth)?,
                    checked_sample(u16::from_be_bytes([b_hi, b_lo]), bit_depth)?,
                ]
            }
            _ => return None,
        };
        Some(Rgb16 { r, g, b })
    }

    /// Returns the packed RGBA8 palette of an indexed PNG, an empty vector for
    /// other PNGs, or `None` if `PLTE` is missing.
    pub fn palette(&self) -> Option<VecU8> {
        let info = self.reader.info();
        if info.color_type != png::ColorType::Indexed {
            return Some(Vec::new().into());
        }
        let plte = info.palette.as_deref()?;
        let trns = info.trns.as_deref().unwrap_or_default();
        let trns = if libpng_keeps_palette_trns(plte, trns, info.bit_depth) { trns } else { &[] };
        let entries = indexable_palette(plte, info.bit_depth);
        let mut rgba = Vec::with_capacity(entries.len() * 4);
        for (i, &[r, g, b]) in entries.iter().enumerate() {
            rgba.extend_from_slice(&[r, g, b, trns.get(i).copied().unwrap_or(u8::MAX)]);
        }
        Some(rgba.into())
    }

    /// Extracts the EXIF, IPTC, and XMP segments from the text chunks before
    /// `IDAT` using libpng's chunk rules, then verifies that the rest of the
    /// PNG stream decodes through `IEND`.
    pub fn read_text_metadata(mut self) -> Result<PngTextMetadata, VecU8> {
        let info = self.reader.info();
        // Like the libpng decoder, use only text chunks before IDAT. The row
        // reads below add later ones to `info`, so `metadata` is built first.
        let metadata = PngTextMetadata {
            exif: last_raw_profile(&info.compressed_latin1_text, &EXIF_CHUNK_KEYWORDS),
            iptc: last_raw_profile(&info.compressed_latin1_text, &IPTC_CHUNK_KEYWORDS),
            xmp: last_xmp(&info.utf8_text),
        };
        while self.reader.next_row().map_err(|e| e.to_string())?.is_some() {}
        self.reader.finish().map_err(|e| e.to_string())?;
        Ok(metadata)
    }

    fn metadata_from_all_chunks(
        &self,
        keys: &[&str],
        header: Option<&str>,
    ) -> ImageResult<Option<Vec<u8>>> {
        if let Some(mut itx_chunk) = self
            .reader
            .info()
            .utf8_text
            .iter()
            .find(|chunk| keys.iter().any(|key| chunk.keyword.contains(key)))
            .cloned()
        {
            itx_chunk.decompress_text().map_err(from_png_error)?;
            return itx_chunk
                .get_text()
                .map(|text| Some(text.into_bytes()))
                .map_err(from_png_error);
        }

        if let Some(mut text_chunk) = self
            .reader
            .info()
            .compressed_latin1_text
            .iter()
            .find(|chunk| keys.iter().any(|key| chunk.keyword.contains(key)))
            .cloned()
        {
            text_chunk.decompress_text().map_err(from_png_error)?;
            return text_chunk
                .get_text()
                .map(|text| Some(parse_raw_profile(text.as_ref(), header)))
                .map_err(from_png_error);
        }

        if let Some(text_chunk) = self
            .reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .find(|chunk| keys.iter().any(|key| chunk.keyword.contains(key)))
            .cloned()
        {
            return Ok(Some(text_chunk.text.as_bytes().to_vec()));
        }

        Ok(None)
    }
}

impl<R: BufRead + Seek> ImageDecoder for PngDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.reader.info().size()
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    fn original_color_type(&self) -> ExtendedColorType {
        match (self.reader.info().color_type, self.reader.info().bit_depth) {
            (png::ColorType::Grayscale, png::BitDepth::One) => ExtendedColorType::L1,
            (png::ColorType::Grayscale, png::BitDepth::Two) => ExtendedColorType::L2,
            (png::ColorType::Grayscale, png::BitDepth::Four) => ExtendedColorType::L4,
            (png::ColorType::Grayscale, png::BitDepth::Eight) => ExtendedColorType::L8,
            (png::ColorType::Grayscale, png::BitDepth::Sixteen) => ExtendedColorType::L16,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::One) => ExtendedColorType::La1,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Two) => ExtendedColorType::La2,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Four) => ExtendedColorType::La4,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => ExtendedColorType::La8,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => ExtendedColorType::La16,
            (png::ColorType::Rgb, png::BitDepth::One) => ExtendedColorType::Rgb1,
            (png::ColorType::Rgb, png::BitDepth::Two) => ExtendedColorType::Rgb2,
            (png::ColorType::Rgb, png::BitDepth::Four) => ExtendedColorType::Rgb4,
            (png::ColorType::Rgb, png::BitDepth::Eight) => ExtendedColorType::Rgb8,
            (png::ColorType::Rgb, png::BitDepth::Sixteen) => ExtendedColorType::Rgb16,
            (png::ColorType::Rgba, png::BitDepth::One) => ExtendedColorType::Rgba1,
            (png::ColorType::Rgba, png::BitDepth::Two) => ExtendedColorType::Rgba2,
            (png::ColorType::Rgba, png::BitDepth::Four) => ExtendedColorType::Rgba4,
            (png::ColorType::Rgba, png::BitDepth::Eight) => ExtendedColorType::Rgba8,
            (png::ColorType::Rgba, png::BitDepth::Sixteen) => ExtendedColorType::Rgba16,
            (png::ColorType::Indexed, png::BitDepth::One) => ExtendedColorType::Unknown(1),
            (png::ColorType::Indexed, png::BitDepth::Two) => ExtendedColorType::Unknown(2),
            (png::ColorType::Indexed, png::BitDepth::Four) => ExtendedColorType::Unknown(4),
            (png::ColorType::Indexed, png::BitDepth::Eight) => ExtendedColorType::Unknown(8),
            (png::ColorType::Indexed, png::BitDepth::Sixteen) => ExtendedColorType::Unknown(16),
        }
    }

    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.reader.info().icc_profile.as_ref().map(|x| x.to_vec()))
    }

    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        self.reader.read_all_metadata().map_err(from_png_error)?;
        if let Some(metadata) = self.reader.info().exif_metadata.as_ref().map(|x| x.to_vec()) {
            return Ok(Some(metadata));
        }
        self.metadata_from_all_chunks(&EXIF_CHUNK_KEYWORDS, Some(EXIF_HEADER))
    }

    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        self.reader.read_all_metadata().map_err(from_png_error)?;
        self.metadata_from_all_chunks(&XMP_CHUNK_KEYWORDS, None)
    }

    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        self.reader.read_all_metadata().map_err(from_png_error)?;
        self.metadata_from_all_chunks(&IPTC_CHUNK_KEYWORDS, None)
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.reader.next_frame(buf).map_err(from_png_error)?;
        let bpc = self.color_type().bytes_per_pixel() / self.color_type().channel_count();
        match bpc {
            1 => (),
            2 => buf.as_chunks_mut::<2>().0.iter_mut().for_each(|c| {
                *c = u16::from_be_bytes(*c).to_ne_bytes();
            }),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }

    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&image::LimitSupport::default())?;
        let info = self.reader.info();
        limits.check_dimensions(info.width, info.height)?;
        self.limits = limits;
        Ok(())
    }
}

fn unsupported_color(ect: ExtendedColorType) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        ImageFormat::Png.into(),
        UnsupportedErrorKind::Color(ect),
    ))
}

fn from_png_error(err: png::DecodingError) -> ImageError {
    match err {
        png::DecodingError::IoError(err) => ImageError::IoError(err),
        err @ png::DecodingError::Format(_) => {
            ImageError::Decoding(DecodingError::new(ImageFormat::Png.into(), err))
        }
        err @ png::DecodingError::LimitsExceeded => ImageError::Limits(
            image::error::LimitError::from_kind(image::error::LimitErrorKind::InsufficientMemory),
        ),
        png::DecodingError::Parameter(err) => {
            ImageError::Parameter(image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::Generic(err.to_string()),
            ))
        }
    }
}

/// Returns the byte lengths of the first `tRNS` chunks before `IDAT` that
/// `png::Decoder` accepts for greyscale (`2..=256`) and RGB (`6..=256`) PNGs.
fn pre_idat_trns_lens<R: BufRead + Seek>(r: &mut R) -> (Option<usize>, Option<usize>) {
    let mut gray_len = None;
    let mut rgb_len = None;
    if r.seek(SeekFrom::Start(PNG_SIGNATURE_LEN)).is_ok() {
        let mut header = [0u8; CHUNK_HEADER_LEN];
        while r.read_exact(&mut header).is_ok() {
            let (len_bytes, chunk_type) = header.split_first_chunk::<CHUNK_LENGTH_LEN>().unwrap();
            if chunk_type == b"IDAT" {
                break;
            }
            let len_u32 = u32::from_be_bytes(*len_bytes);
            if r.seek(SeekFrom::Current(i64::from(len_u32) + CHUNK_CRC_LEN)).is_err() {
                break;
            }
            if chunk_type == b"tRNS"
                && let Ok(len) = usize::try_from(len_u32)
            {
                if gray_len.is_none() && (GRAY_TRNS_LEN..=MAX_TRNS_LEN).contains(&len) {
                    gray_len = Some(len);
                }
                if rgb_len.is_none() && (RGB_TRNS_LEN..=MAX_TRNS_LEN).contains(&len) {
                    rgb_len = Some(len);
                }
            }
        }
    }
    let _ = r.seek(SeekFrom::Start(0));
    (gray_len, rgb_len)
}

/// Returns `sample`, or `None` if it exceeds the maximum value for `bit_depth`,
/// in which case `png_handle_bKGD` in google3/third_party/png/src/pngrutil.c
/// drops the chunk.
fn checked_sample(sample: u16, bit_depth: png::BitDepth) -> Option<u16> {
    match bit_depth {
        png::BitDepth::Sixteen => Some(sample),
        png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four | png::BitDepth::Eight => {
            let max = (1u16 << (bit_depth as u8)) - 1;
            (sample <= max).then_some(sample)
        }
    }
}

/// Returns the `PLTE` entries that `bit_depth` can index, matching the
/// truncation in `png_handle_PLTE` in google3/third_party/png/src/pngrutil.c.
fn indexable_palette(plte: &[u8], bit_depth: png::BitDepth) -> &[[u8; PLTE_ENTRY_LEN]] {
    let entries = plte.as_chunks::<PLTE_ENTRY_LEN>().0;
    let max_entries = 2usize.pow(u32::from(bit_depth as u8));
    &entries[..entries.len().min(max_entries)]
}

/// Returns whether `png_handle_tRNS` in google3/third_party/png/src/pngrutil.c
/// keeps the `tRNS` of an indexed PNG. It drops an empty chunk and one longer
/// than the palette, which `png_handle_PLTE` first truncates to the entries
/// that `bit_depth` can index.
fn libpng_keeps_palette_trns(plte: &[u8], trns: &[u8], bit_depth: png::BitDepth) -> bool {
    !trns.is_empty() && trns.len() <= indexable_palette(plte, bit_depth).len()
}

/// Returns the decoded raw profile of the last `zTXt` chunk with one of
/// `keywords` that libpng would have kept.
fn last_raw_profile(chunks: &[ZTXtChunk], keywords: &[&str]) -> Option<VecU8> {
    chunks
        .iter()
        .rev()
        .filter(|chunk| keywords.contains(&chunk.keyword.as_str()))
        .find_map(ztxt_text)
        .map(|text| VecU8::from(hex_decode(&text)))
}

/// Returns the XMP segment of the last `iTXt` chunk with an XMP keyword that
/// the libpng decoder would read.
fn last_xmp(chunks: &[ITXtChunk]) -> Option<VecU8> {
    chunks
        .iter()
        .rev()
        .filter(|chunk| XMP_CHUNK_KEYWORDS.contains(&chunk.keyword.as_str()))
        .find_map(itxt_xmp)
        .map(VecU8::from)
}

/// Returns the text of a `zTXt` chunk up to its first NUL, or `None` if libpng
/// would have dropped the chunk, or relabelled it as `tEXt` for being empty.
fn ztxt_text(chunk: &ZTXtChunk) -> Option<Vec<u8>> {
    let mut chunk = chunk.clone();
    // Bounds the work; `fits_chunk_malloc_max()` below is the exact check.
    chunk.decompress_text_with_limit(PNG_USER_CHUNK_MALLOC_MAX).ok()?;
    // The crate decodes zTXt as ISO-8859-1, so every char maps back to a byte.
    let mut text: Vec<u8> =
        chunk.get_text().ok()?.chars().filter_map(|c| u8::try_from(c).ok()).collect();
    let field_lens = [chunk.keyword.len(), NUL_LEN, COMPRESSION_METHOD_LEN, text.len()];
    if !fits_chunk_malloc_max(&field_lens) {
        return None;
    }
    truncate_at_nul(&mut text);
    (!text.is_empty()).then_some(text)
}

/// Returns the text of an uncompressed `iTXt` chunk up to its first NUL, or
/// `None` if libpng would have dropped the chunk.
fn itxt_xmp(chunk: &ITXtChunk) -> Option<Vec<u8>> {
    if chunk.compressed {
        return None;
    }
    let mut text = chunk.get_text().ok()?.into_bytes();
    let field_lens = [
        chunk.keyword.len(),
        NUL_LEN,
        COMPRESSION_FLAG_LEN,
        COMPRESSION_METHOD_LEN,
        chunk.language_tag.len(),
        NUL_LEN,
        chunk.translated_keyword.len(),
        NUL_LEN,
        text.len(),
    ];
    if !fits_chunk_malloc_max(&field_lens) {
        return None;
    }
    truncate_at_nul(&mut text);
    Some(text)
}

/// libpng drops a text chunk if its fields, with the text decompressed, plus a
/// NUL terminator exceed `PNG_USER_CHUNK_MALLOC_MAX`.
fn fits_chunk_malloc_max(field_lens: &[usize]) -> bool {
    let allocation_len =
        field_lens.iter().fold(NUL_LEN, |len, &field_len| len.saturating_add(field_len));
    allocation_len <= PNG_USER_CHUNK_MALLOC_MAX
}

/// libpng keeps text only up to its first NUL, because `png_set_text_2()`
/// copies `strlen()` bytes of it; the png crate keeps every byte.
fn truncate_at_nul(text: &mut Vec<u8>) {
    if let Some(nul) = text.iter().position(|&byte| byte == 0) {
        text.truncate(nul);
    }
}

/// Decodes an ImageMagick raw profile: an empty line, a name line, a line with
/// the decoded size, then hex digits with interspersed newlines. Returns an
/// empty segment for malformed input.
///
/// Unlike `GetHexDecodedSegment()` in the libpng decoder, this rejects letters
/// past `f` and missing header newlines, which that function misreads.
fn hex_decode(text: &[u8]) -> Vec<u8> {
    let mut lines = text.splitn(4, |&byte| byte == b'\n');
    let (Some(_), Some(_), Some(size_line), Some(hex)) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    else {
        return Vec::new();
    };
    // Matches the limit in `GetHexDecodedSegment()` in google3/image/codec/pngdecoder.cc.
    const MAX_DECLARED_SIZE: i32 = 1 << 22;
    // Like `SimpleAtoi()`, also trims vertical tabs, which `is_ascii_whitespace()` excludes.
    let size = std::str::from_utf8(size_line).ok().and_then(|line| {
        line.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b').parse::<i32>().ok()
    });
    if !size.is_some_and(|size| size <= MAX_DECLARED_SIZE) {
        return Vec::new();
    }

    let mut segment = Vec::new();
    let mut index = 0;
    while index + 1 < hex.len() {
        if hex[index] == b'\n' {
            index += 1;
            continue;
        }
        let (Some(high), Some(low)) = (hex_digit(hex[index]), hex_digit(hex[index + 1])) else {
            return Vec::new();
        };
        segment.push((high << 4) | low);
        index += 2;
    }
    segment
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_raw_profile(buffer: &str, header: Option<&str>) -> Vec<u8> {
    let parse = || {
        let mut parts = buffer.splitn(4, '\n');
        let size_str = parts.nth(2)?;
        let body = parts.next()?;
        let size: usize = size_str.trim().parse().ok()?;
        if size > (1 << 22) {
            return None;
        }

        let header_bytes = header.map(|h| h.as_bytes());
        let mut payload = Vec::new();
        let mut hex_stream = body.bytes().filter(|&b| b != b'\n');

        while let (Some(high), Some(low)) = (hex_stream.next(), hex_stream.next()) {
            let decode = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
            let byte = (decode(high)? << 4) | decode(low)?;
            payload.push(byte);
            if let Some(h) = header_bytes
                && payload == h
            {
                payload.clear();
            }
        }

        if hex_stream.next().is_some() {
            return None;
        }

        Some(payload)
    };

    parse().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    const EXIF_CHUNK_KEYWORD: &str = EXIF_CHUNK_KEYWORDS[1];
    const XMP_CHUNK_KEYWORD: &str = XMP_CHUNK_KEYWORDS[0];
    const LANGUAGE_TAG: &str = "en";
    const TRANSLATED_KEYWORD: &str = "XMP";

    // Text lengths at which libpng's allocation, the fields before the text
    // plus its NUL terminator, is exactly `PNG_USER_CHUNK_MALLOC_MAX`.
    const MAX_EXIF_ZTXT_TEXT_LEN: usize = PNG_USER_CHUNK_MALLOC_MAX
        - (EXIF_CHUNK_KEYWORD.len() + NUL_LEN + COMPRESSION_METHOD_LEN)
        - NUL_LEN;

    const MAX_TRANSLATED_XMP_ITXT_TEXT_LEN: usize = PNG_USER_CHUNK_MALLOC_MAX
        - (XMP_CHUNK_KEYWORD.len()
            + NUL_LEN
            + COMPRESSION_FLAG_LEN
            + COMPRESSION_METHOD_LEN
            + LANGUAGE_TAG.len()
            + NUL_LEN
            + TRANSLATED_KEYWORD.len()
            + NUL_LEN)
        - NUL_LEN;

    /// Returns a `zTXt` chunk holding compressed text, as the decoder returns
    /// it, so that `ztxt_text` has to decompress it within the limit.
    fn ztxt(keyword: &str, text: impl Into<String>) -> ZTXtChunk {
        let mut chunk = ZTXtChunk::new(keyword, text);
        chunk.compress_text().unwrap();
        chunk
    }

    fn compressed_itxt(keyword: &str, text: impl Into<String>) -> ITXtChunk {
        let mut chunk = ITXtChunk::new(keyword, text);
        chunk.compressed = true;
        chunk.compress_text().unwrap();
        chunk
    }

    fn translated_xmp_itxt(text_len: usize) -> ITXtChunk {
        let mut chunk = ITXtChunk::new(XMP_CHUNK_KEYWORD, "a".repeat(text_len));
        chunk.language_tag = LANGUAGE_TAG.to_string();
        chunk.translated_keyword = TRANSLATED_KEYWORD.to_string();
        chunk
    }

    #[gtest]
    fn hex_decode_decodes_digits_of_either_case() {
        expect_eq!(hex_decode(b"\nexif\n3\n0aFf7C"), vec![0x0a, 0xff, 0x7c]);
    }

    #[gtest]
    fn hex_decode_ignores_first_two_lines() {
        expect_eq!(hex_decode(b"junk\nmore junk\n1\n01"), vec![0x01]);
    }

    #[gtest]
    fn hex_decode_skips_newlines_between_bytes() {
        expect_eq!(hex_decode(b"\nexif\n3\n\n0a\nff\n\n7c\n"), vec![0x0a, 0xff, 0x7c]);
    }

    #[gtest]
    fn hex_decode_drops_unpaired_final_digit() {
        expect_eq!(hex_decode(b"\nexif\n1\n01a"), vec![0x01]);
    }

    #[gtest]
    fn hex_decode_rejects_newline_paired_with_digit() {
        // As in the libpng decoder, a newline is only skipped where a byte
        // starts, so even one after an unpaired final digit is rejected.
        expect_that!(hex_decode(b"\nexif\n1\n0\n1"), is_empty());
        expect_that!(hex_decode(b"\nexif\n1\n01a\n"), is_empty());
    }

    #[gtest]
    fn hex_decode_rejects_non_hex_digit() {
        // The libpng decoder reads 'g' to 'z' as the digits 16 to 35.
        expect_that!(hex_decode(b"\nexif\n1\n0g"), is_empty());
    }

    #[gtest]
    fn hex_decode_rejects_profile_missing_header_line() {
        for profile in ["0102", "1\n00", "ab\ncd\n5"] {
            expect_that!(hex_decode(profile.as_bytes()), is_empty(), "profile {:?}", profile);
        }
    }

    #[gtest]
    fn hex_decode_checks_declared_size() {
        for (size_line, accepted) in [
            ("4194304", true),
            ("4194305", false),
            ("-1", true),
            ("\t +1 \r\x0c", true),
            ("\x0b1\x0b", true),
            ("1 2", false),
            ("", false),
            ("2147483648", false),
        ] {
            let expected: &[u8] = if accepted { &[0] } else { &[] };
            expect_eq!(
                hex_decode(format!("\nexif\n{size_line}\n00").as_bytes()),
                expected,
                "size line {:?}",
                size_line
            );
        }
        expect_that!(hex_decode(b"\nexif\n\xff\n00"), is_empty());
    }

    #[gtest]
    fn hex_digit_accepts_only_hexadecimal_digits() {
        for byte in u8::MIN..=u8::MAX {
            expect_eq!(
                hex_digit(byte).map(u32::from),
                char::from(byte).to_digit(16),
                "byte {:#04x}",
                byte
            );
        }
    }

    const MAX_SAMPLES_BELOW_16_BITS: [(png::BitDepth, u16); 4] = [
        (png::BitDepth::One, 1),
        (png::BitDepth::Two, 3),
        (png::BitDepth::Four, 15),
        (png::BitDepth::Eight, 255),
    ];

    #[gtest]
    fn checked_sample_keeps_any_16_bit_sample() {
        for sample in [0x0000_u16, 0x00ff, 0x12ab, 0xffff] {
            expect_eq!(
                checked_sample(sample, png::BitDepth::Sixteen),
                Some(sample),
                "sample {:#06x}",
                sample
            );
        }
    }

    #[gtest]
    fn checked_sample_keeps_samples_up_to_maximum_for_bit_depth() {
        for (bit_depth, max) in MAX_SAMPLES_BELOW_16_BITS {
            for sample in 0..=max {
                expect_eq!(checked_sample(sample, bit_depth), Some(sample), "{:?}", bit_depth);
            }
        }
    }

    #[gtest]
    fn checked_sample_rejects_sample_over_maximum_for_bit_depth() {
        for (bit_depth, max) in MAX_SAMPLES_BELOW_16_BITS {
            expect_that!(checked_sample(max + 1, bit_depth), none(), "{:?}", bit_depth);
            expect_that!(checked_sample(u16::MAX, bit_depth), none(), "{:?}", bit_depth);
        }
    }

    /// A 1-bit image indexes only the first two of these four entries.
    const FOUR_ENTRY_PLTE: [u8; 4 * PLTE_ENTRY_LEN] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

    #[gtest]
    fn indexable_palette_truncates_to_bit_depth() {
        expect_eq!(
            indexable_palette(&FOUR_ENTRY_PLTE, png::BitDepth::One),
            &[[1, 2, 3], [4, 5, 6]]
        );
        expect_eq!(
            indexable_palette(&FOUR_ENTRY_PLTE, png::BitDepth::Two),
            &[[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]
        );
        expect_eq!(
            indexable_palette(&FOUR_ENTRY_PLTE, png::BitDepth::Eight),
            &[[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]
        );
    }

    #[gtest]
    fn libpng_keeps_palette_trns_keeps_trns_up_to_kept_palette_len() {
        expect_that!(
            libpng_keeps_palette_trns(&FOUR_ENTRY_PLTE, &[7, 8], png::BitDepth::One),
            eq(true)
        );
        expect_that!(
            libpng_keeps_palette_trns(&FOUR_ENTRY_PLTE, &[7, 8, 9, 10], png::BitDepth::Eight),
            eq(true)
        );
    }

    #[gtest]
    fn libpng_keeps_palette_trns_drops_trns_longer_than_kept_palette() {
        expect_that!(
            libpng_keeps_palette_trns(&FOUR_ENTRY_PLTE, &[7, 8, 9], png::BitDepth::One),
            eq(false)
        );
        expect_that!(
            libpng_keeps_palette_trns(&FOUR_ENTRY_PLTE, &[7, 8, 9, 10, 11], png::BitDepth::Eight),
            eq(false)
        );
    }

    #[gtest]
    fn libpng_keeps_palette_trns_drops_empty_trns() {
        expect_that!(
            libpng_keeps_palette_trns(&FOUR_ENTRY_PLTE, &[], png::BitDepth::Eight),
            eq(false)
        );
    }

    #[gtest]
    fn truncate_at_nul_drops_everything_from_first_nul() {
        let truncated = |text: &[u8]| {
            let mut text = text.to_vec();
            truncate_at_nul(&mut text);
            text
        };
        expect_eq!(truncated(b"abc"), b"abc");
        expect_eq!(truncated(b"ab\0c\0d"), b"ab");
        expect_that!(truncated(b"\0abc"), is_empty());
    }

    #[gtest]
    fn ztxt_text_maps_latin1_text_back_to_bytes() {
        expect_eq!(ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "caf\u{e9}")), Some(b"caf\xe9".to_vec()));
    }

    #[gtest]
    fn ztxt_text_stops_at_first_nul() {
        expect_eq!(ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "ab\0cd")), Some(b"ab".to_vec()));
    }

    #[gtest]
    fn ztxt_text_drops_empty_text() {
        // libpng relabels a zTXt chunk with empty text as tEXt.
        expect_that!(ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "")), none());
        expect_that!(ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "\0ab")), none());
    }

    #[gtest]
    fn ztxt_text_keeps_text_at_libpng_limit() {
        let text = ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "a".repeat(MAX_EXIF_ZTXT_TEXT_LEN)));
        expect_eq!(text.map(|text| text.len()), Some(MAX_EXIF_ZTXT_TEXT_LEN));
    }

    #[gtest]
    fn ztxt_text_drops_text_over_libpng_limit() {
        let text = ztxt_text(&ztxt(EXIF_CHUNK_KEYWORD, "a".repeat(MAX_EXIF_ZTXT_TEXT_LEN + 1)));
        expect_that!(text.map(|text| text.len()), none());
    }

    #[gtest]
    fn itxt_xmp_stops_at_first_nul() {
        expect_eq!(
            itxt_xmp(&ITXtChunk::new(XMP_CHUNK_KEYWORD, "<x/>\0junk")),
            Some(b"<x/>".to_vec())
        );
    }

    #[gtest]
    fn itxt_xmp_keeps_empty_uncompressed_text() {
        expect_that!(itxt_xmp(&ITXtChunk::new(XMP_CHUNK_KEYWORD, "")), some(is_empty()));
    }

    #[gtest]
    fn itxt_xmp_ignores_compressed_text() {
        expect_that!(itxt_xmp(&compressed_itxt(XMP_CHUNK_KEYWORD, "<x/>")), none());
        // Unlike the libpng decoder, which reads it as empty XMP once libpng has
        // relabelled it as uncompressed.
        expect_that!(itxt_xmp(&compressed_itxt(XMP_CHUNK_KEYWORD, "")), none());
    }

    #[gtest]
    fn itxt_xmp_keeps_text_at_libpng_limit() {
        let text = itxt_xmp(&translated_xmp_itxt(MAX_TRANSLATED_XMP_ITXT_TEXT_LEN));
        expect_eq!(text.map(|text| text.len()), Some(MAX_TRANSLATED_XMP_ITXT_TEXT_LEN));
    }

    #[gtest]
    fn itxt_xmp_drops_text_over_libpng_limit() {
        let text = itxt_xmp(&translated_xmp_itxt(MAX_TRANSLATED_XMP_ITXT_TEXT_LEN + 1));
        expect_that!(text.map(|text| text.len()), none());
    }

    #[gtest]
    fn last_raw_profile_decodes_last_chunk_with_matching_keyword() {
        let chunks = [
            ztxt("Raw profile type APP1", "\nexif\n1\n01"),
            ztxt("Raw profile type exif", "\nexif\n1\n02"),
            ztxt("Raw profile type iptc", "\niptc\n1\n03"),
        ];
        expect_eq!(
            last_raw_profile(&chunks, &EXIF_CHUNK_KEYWORDS).map(|v| v.as_slice().to_vec()),
            Some(vec![0x02])
        );
        expect_eq!(
            last_raw_profile(&chunks, &IPTC_CHUNK_KEYWORDS).map(|v| v.as_slice().to_vec()),
            Some(vec![0x03])
        );
    }

    #[gtest]
    fn last_raw_profile_falls_back_over_chunk_libpng_dropped() {
        let chunks = [ztxt(EXIF_CHUNK_KEYWORD, "\nexif\n1\n01"), ztxt(EXIF_CHUNK_KEYWORD, "")];
        expect_eq!(
            last_raw_profile(&chunks, &EXIF_CHUNK_KEYWORDS).map(|v| v.as_slice().to_vec()),
            Some(vec![0x01])
        );
    }

    #[gtest]
    fn last_raw_profile_keeps_last_chunk_even_if_malformed() {
        let chunks =
            [ztxt(EXIF_CHUNK_KEYWORD, "\nexif\n1\n01"), ztxt(EXIF_CHUNK_KEYWORD, "\nexif\n1\nzz")];
        expect_that!(
            last_raw_profile(&chunks, &EXIF_CHUNK_KEYWORDS).map(|v| v.as_slice().to_vec()),
            some(is_empty())
        );
    }

    #[gtest]
    fn last_raw_profile_returns_none_without_matching_chunk() {
        expect_that!(
            last_raw_profile(&[], &EXIF_CHUNK_KEYWORDS).map(|v| v.as_slice().to_vec()),
            none()
        );
        expect_that!(
            last_raw_profile(
                &[ztxt("Raw profile type iptc", "\niptc\n1\n03")],
                &EXIF_CHUNK_KEYWORDS
            )
            .map(|v| v.as_slice().to_vec()),
            none()
        );
    }

    #[gtest]
    fn last_xmp_returns_last_uncompressed_chunk_with_xmp_keyword() {
        let chunks = [
            ITXtChunk::new("XML:com.adobe.xmp", "<a/>"),
            ITXtChunk::new("Raw profile type xmp", "<b/>"),
            ITXtChunk::new("Comment", "<c/>"),
        ];
        expect_eq!(last_xmp(&chunks).map(|v| v.as_slice().to_vec()), Some(b"<b/>".to_vec()));
    }

    #[gtest]
    fn last_xmp_falls_back_over_compressed_chunk() {
        let chunks =
            [ITXtChunk::new(XMP_CHUNK_KEYWORD, "<a/>"), compressed_itxt(XMP_CHUNK_KEYWORD, "<b/>")];
        expect_eq!(last_xmp(&chunks).map(|v| v.as_slice().to_vec()), Some(b"<a/>".to_vec()));
    }

    #[gtest]
    fn last_xmp_returns_none_without_xmp_chunk() {
        expect_that!(
            last_xmp(&[ITXtChunk::new("Comment", "<a/>")]).map(|v| v.as_slice().to_vec()),
            none()
        );
    }
}
