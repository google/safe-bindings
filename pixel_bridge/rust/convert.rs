#![forbid(unsafe_code)]

use crate::image::{ColorType, DownscaleMode, PixelType};

const BITS_PER_BYTE: u8 = 8;
const ROUNDING_DIVISOR_16_TO_8: u32 = 257;
const ROUNDING_HALF_16_TO_8: u32 = 128;

/// `SRGB_LUMA` and `SRGB_LUMA_DIV` in
/// google3/third_party/rust/image/v0_25/src/color.rs.
const SRGB_LUMA_R: u32 = 2126;
const SRGB_LUMA_G: u32 = 7152;
const SRGB_LUMA_B: u32 = 722;
const SRGB_LUMA_DIV: u32 = 10_000;
const SRGB_LUMA_HALF: u32 = SRGB_LUMA_DIV / 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidColorType {
    L,
    La,
    Rgb,
    Rgba,
}

impl ValidColorType {
    fn from_ffi(color: ColorType) -> Result<Self, &'static str> {
        match color {
            ColorType::kL => Ok(Self::L),
            ColorType::kLa => Ok(Self::La),
            ColorType::kRgb => Ok(Self::Rgb),
            ColorType::kRgba => Ok(Self::Rgba),
            _ => Err("Invalid color_type in ConversionOptions"),
        }
    }

    fn channel_count(self) -> usize {
        match self {
            Self::L => 1,
            Self::La => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidPixelType {
    U8,
    U16,
    F32,
}

impl ValidPixelType {
    fn from_ffi(pixel: PixelType) -> Result<Self, &'static str> {
        match pixel {
            PixelType::kUint8 => Ok(Self::U8),
            PixelType::kUint16 => Ok(Self::U16),
            PixelType::kFloat32 => Ok(Self::F32),
            _ => Err("Invalid pixel_type in ConversionOptions"),
        }
    }

    fn sample_bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::F32 => 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidDownscaleMode {
    TruncateHighByte,
    Round,
}

impl ValidDownscaleMode {
    fn from_ffi(mode: DownscaleMode) -> Result<Self, &'static str> {
        match mode {
            DownscaleMode::kTruncateHighByte => Ok(Self::TruncateHighByte),
            DownscaleMode::kRound => Ok(Self::Round),
            _ => Err("Invalid downscale_mode in ConversionOptions"),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ConversionSpec {
    pub width: usize,
    pub height: usize,
    pub src_color: ColorType,
    pub src_pixel: PixelType,
    pub target_color: ColorType,
    pub target_pixel: PixelType,
    pub downscale_mode: DownscaleMode,
    pub row_bytes: usize,
    pub rows_bottom_up: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ValidatedLayout {
    pub width: usize,
    pub height: usize,
    pub src_color: ValidColorType,
    pub src_pixel: ValidPixelType,
    pub target_color: ValidColorType,
    pub target_pixel: ValidPixelType,
    pub downscale_mode: ValidDownscaleMode,
    pub packed_row_bytes: usize,
    pub effective_row_bytes: usize,
    pub rows_bottom_up: bool,
    pub is_identity_packed: bool,
}

struct ValidatedTypes {
    src_color: ValidColorType,
    target_color: ValidColorType,
    src_pixel: ValidPixelType,
    target_pixel: ValidPixelType,
    downscale_mode: ValidDownscaleMode,
}

struct PixelOps<S, L, E> {
    opaque: S,
    luminance: L,
    encode: E,
}

pub(crate) fn validate_target_options(
    target_color: ColorType,
    target_pixel: PixelType,
    downscale_mode: DownscaleMode,
) -> Result<(ValidColorType, ValidPixelType, ValidDownscaleMode), &'static str> {
    let color = ValidColorType::from_ffi(target_color)?;
    let pixel = ValidPixelType::from_ffi(target_pixel)?;
    let mode = ValidDownscaleMode::from_ffi(downscale_mode)?;
    Ok((color, pixel, mode))
}

fn validate_types(spec: &ConversionSpec) -> Result<ValidatedTypes, &'static str> {
    let (target_color, target_pixel, downscale_mode) =
        validate_target_options(spec.target_color, spec.target_pixel, spec.downscale_mode)?;
    let src_color = ValidColorType::from_ffi(spec.src_color)?;
    let src_pixel = ValidPixelType::from_ffi(spec.src_pixel)?;

    if (matches!(src_pixel, ValidPixelType::F32) || matches!(target_pixel, ValidPixelType::F32))
        && !(matches!(src_pixel, ValidPixelType::F32)
            && matches!(target_pixel, ValidPixelType::F32)
            && src_color.channel_count() == target_color.channel_count())
    {
        return Err("Float32 sample conversion is only supported when source and target match");
    }

    Ok(ValidatedTypes { src_color, target_color, src_pixel, target_pixel, downscale_mode })
}

pub(crate) fn validate_packed_samples(spec: &ConversionSpec) -> Result<usize, &'static str> {
    let types = validate_types(spec)?;
    let target_channels = types.target_color.channel_count();
    let target_sample_bytes = types.target_pixel.sample_bytes();

    let row_samples =
        spec.width.checked_mul(target_channels).ok_or("Converted image buffer size overflowed")?;
    let packed_row_bytes = row_samples
        .checked_mul(target_sample_bytes)
        .ok_or("Converted image buffer size overflowed")?;
    let total_samples =
        row_samples.checked_mul(spec.height).ok_or("Converted image buffer size overflowed")?;
    let total_bytes = total_samples
        .checked_mul(target_sample_bytes)
        .ok_or("Converted image buffer size overflowed")?;
    if total_bytes > isize::MAX as usize {
        return Err("Converted image buffer size overflowed");
    }
    if spec.row_bytes != 0 && spec.row_bytes != packed_row_bytes {
        return Err(
            "ReadSamplesConverted requires tightly packed rows; use ReadSamplesConvertedIntoRaw for custom row_bytes",
        );
    }
    Ok(total_samples)
}

pub(crate) fn validate_layout(
    spec: &ConversionSpec,
    dest_len: usize,
) -> Result<ValidatedLayout, &'static str> {
    let types = validate_types(spec)?;
    let src_channels = types.src_color.channel_count();
    let target_channels = types.target_color.channel_count();
    let src_sample_bytes = types.src_pixel.sample_bytes();
    let target_sample_bytes = types.target_pixel.sample_bytes();

    let packed_row_bytes = spec
        .width
        .checked_mul(target_channels)
        .and_then(|samples| samples.checked_mul(target_sample_bytes))
        .ok_or("Destination row size overflowed")?;

    let (effective_row_bytes, dest_needed) = match spec.height {
        0 => (0, 0),
        1 => (packed_row_bytes, packed_row_bytes),
        height => {
            if spec.row_bytes != 0 && spec.row_bytes < packed_row_bytes {
                return Err("row_bytes is smaller than tightly packed destination row size");
            }
            let stride = if spec.row_bytes == 0 { packed_row_bytes } else { spec.row_bytes };
            let needed = if packed_row_bytes == 0 {
                0
            } else {
                height
                    .checked_sub(1)
                    .and_then(|rows| rows.checked_mul(stride))
                    .and_then(|offset| offset.checked_add(packed_row_bytes))
                    .ok_or("Destination buffer size overflowed")?
            };
            (stride, needed)
        }
    };

    if dest_len < dest_needed {
        return Err("Destination buffer is too small for converted image");
    }

    let is_identity_packed = src_channels == target_channels
        && src_sample_bytes == target_sample_bytes
        && (!spec.rows_bottom_up || spec.height <= 1)
        && (spec.height <= 1 || effective_row_bytes == packed_row_bytes);

    Ok(ValidatedLayout {
        width: spec.width,
        height: spec.height,
        src_color: types.src_color,
        src_pixel: types.src_pixel,
        target_color: types.target_color,
        target_pixel: types.target_pixel,
        downscale_mode: types.downscale_mode,
        packed_row_bytes,
        effective_row_bytes,
        rows_bottom_up: spec.rows_bottom_up,
        is_identity_packed,
    })
}

fn dest_row_mut<'a>(
    dest: &'a mut [u8],
    layout: &ValidatedLayout,
    row: usize,
    dest_row_len: usize,
) -> Result<&'a mut [u8], &'static str> {
    let dest_row_index = if layout.rows_bottom_up && layout.height > 1 {
        layout
            .height
            .checked_sub(1)
            .and_then(|last| last.checked_sub(row))
            .ok_or("Destination row index underflowed")?
    } else {
        row
    };
    let dest_start = if layout.height <= 1 {
        0
    } else {
        dest_row_index
            .checked_mul(layout.effective_row_bytes)
            .ok_or("Destination row offset overflowed")?
    };
    dest.get_mut(dest_start..)
        .and_then(|tail| tail.get_mut(..dest_row_len))
        .ok_or("Destination buffer is too small for row")
}

pub(crate) fn copy_strided_rows(
    src_bytes: &[u8],
    dest: &mut [u8],
    layout: &ValidatedLayout,
) -> Result<(), &'static str> {
    if layout.packed_row_bytes == 0 || layout.height == 0 {
        return Ok(());
    }
    let src_needed = layout
        .packed_row_bytes
        .checked_mul(layout.height)
        .ok_or("Source buffer size overflowed")?;
    if src_bytes.len() < src_needed {
        return Err("Decoded source buffer is smaller than expected");
    }
    if !layout.rows_bottom_up
        && (layout.height == 1 || layout.effective_row_bytes == layout.packed_row_bytes)
    {
        let (Some(src_slice), Some(dest_slice)) =
            (src_bytes.get(..src_needed), dest.get_mut(..src_needed))
        else {
            return Err("Destination buffer is too small for converted image");
        };
        dest_slice.copy_from_slice(src_slice);
        return Ok(());
    }
    for (h, src_row) in
        src_bytes.chunks_exact(layout.packed_row_bytes).take(layout.height).enumerate()
    {
        let dest_row = dest_row_mut(dest, layout, h, layout.packed_row_bytes)?;
        dest_row.copy_from_slice(src_row);
    }
    Ok(())
}

fn srgb_rgb8_to_luma(r: u8, g: u8, b: u8) -> u8 {
    ((SRGB_LUMA_R * u32::from(r)
        + SRGB_LUMA_G * u32::from(g)
        + SRGB_LUMA_B * u32::from(b)
        + SRGB_LUMA_HALF)
        / SRGB_LUMA_DIV) as u8
}

fn srgb_rgb16_to_luma(r: u16, g: u16, b: u16) -> u16 {
    ((SRGB_LUMA_R * u32::from(r)
        + SRGB_LUMA_G * u32::from(g)
        + SRGB_LUMA_B * u32::from(b)
        + SRGB_LUMA_HALF)
        / SRGB_LUMA_DIV) as u16
}

fn convert_rows_n<S, L, E, const N: usize, const B: usize>(
    src_pixels: &[S],
    dest: &mut [u8],
    layout: &ValidatedLayout,
    ops: &PixelOps<S, L, E>,
) -> Result<(), &'static str>
where
    S: Copy,
    L: Fn(S, S, S) -> S,
    E: Fn(S) -> [u8; B],
{
    let t = layout.target_color.channel_count();
    let dest_pixel_bytes = t.checked_mul(B).ok_or("Destination pixel size overflowed")?;
    let src_row_len = layout.width.checked_mul(N).ok_or("Source row size overflowed")?;
    let dest_row_len =
        layout.width.checked_mul(dest_pixel_bytes).ok_or("Destination row size overflowed")?;
    let src_needed =
        src_row_len.checked_mul(layout.height).ok_or("Source buffer size overflowed")?;
    if src_pixels.len() < src_needed {
        return Err("Decoded source buffer is smaller than expected");
    }
    if src_row_len == 0 || layout.height == 0 {
        return Ok(());
    }
    let has_alpha = matches!(N, 2 | 4);
    for (h, src_row) in src_pixels.chunks_exact(src_row_len).take(layout.height).enumerate() {
        let dest_row = dest_row_mut(dest, layout, h, dest_row_len)?;
        let (src_pixels_row, _) = src_row.as_chunks::<N>();
        let pixel_pairs = src_pixels_row.iter().zip(dest_row.chunks_exact_mut(dest_pixel_bytes));
        if t <= 2 {
            for (pixel, out) in pixel_pairs {
                let alpha = if has_alpha && let Some(&a) = pixel.last() { a } else { ops.opaque };
                let gray = if let Some(&[r, g, b]) = pixel.first_chunk::<3>() {
                    (ops.luminance)(r, g, b)
                } else if let Some(&g) = pixel.first() {
                    g
                } else {
                    ops.opaque
                };
                if let Some((gray_out, rest)) = out.split_first_chunk_mut::<B>() {
                    *gray_out = (ops.encode)(gray);
                    if t == 2
                        && let Some(alpha_out) = rest.first_chunk_mut::<B>()
                    {
                        *alpha_out = (ops.encode)(alpha);
                    }
                }
            }
        } else {
            for (pixel, out) in pixel_pairs {
                let alpha = if has_alpha && let Some(&a) = pixel.last() { a } else { ops.opaque };
                let (r, g, b) = if let Some(&[r, g, b]) = pixel.first_chunk::<3>() {
                    (r, g, b)
                } else if let Some(&g) = pixel.first() {
                    (g, g, g)
                } else {
                    (ops.opaque, ops.opaque, ops.opaque)
                };
                let (channels_out, _) = out.as_chunks_mut::<B>();
                if let Some(([c0, c1, c2], rest)) = channels_out.split_first_chunk_mut::<3>() {
                    *c0 = (ops.encode)(r);
                    *c1 = (ops.encode)(g);
                    *c2 = (ops.encode)(b);
                    if t == 4
                        && let Some(c3) = rest.first_mut()
                    {
                        *c3 = (ops.encode)(alpha);
                    }
                }
            }
        }
    }
    Ok(())
}

fn dispatch_channels<S, L, E, const B: usize>(
    src_pixels: &[S],
    dest: &mut [u8],
    layout: &ValidatedLayout,
    ops: PixelOps<S, L, E>,
) -> Result<(), &'static str>
where
    S: Copy,
    L: Fn(S, S, S) -> S,
    E: Fn(S) -> [u8; B],
{
    match layout.src_color {
        ValidColorType::L => convert_rows_n::<S, L, E, 1, B>(src_pixels, dest, layout, &ops),
        ValidColorType::La => convert_rows_n::<S, L, E, 2, B>(src_pixels, dest, layout, &ops),
        ValidColorType::Rgb => convert_rows_n::<S, L, E, 3, B>(src_pixels, dest, layout, &ops),
        ValidColorType::Rgba => convert_rows_n::<S, L, E, 4, B>(src_pixels, dest, layout, &ops),
    }
}

pub(crate) fn convert_u8_pixels(
    src_pixels: &[u8],
    dest: &mut [u8],
    layout: &ValidatedLayout,
) -> Result<(), &'static str> {
    match layout.target_pixel {
        ValidPixelType::U8
            if layout.src_color.channel_count() == layout.target_color.channel_count() =>
        {
            copy_strided_rows(src_pixels, dest, layout)
        }
        ValidPixelType::U8 => dispatch_channels(
            src_pixels,
            dest,
            layout,
            PixelOps { opaque: u8::MAX, luminance: srgb_rgb8_to_luma, encode: |v| [v] },
        ),
        ValidPixelType::U16 => dispatch_channels(
            src_pixels,
            dest,
            layout,
            PixelOps {
                opaque: u8::MAX,
                luminance: srgb_rgb8_to_luma,
                encode: |v| ((u16::from(v) << BITS_PER_BYTE) | u16::from(v)).to_ne_bytes(),
            },
        ),
        ValidPixelType::F32 => Err("Float32 sample conversion is not supported"),
    }
}

pub(crate) fn convert_u16_pixels(
    src_pixels: &[u16],
    dest: &mut [u8],
    layout: &ValidatedLayout,
) -> Result<(), &'static str> {
    match layout.target_pixel {
        ValidPixelType::U16
            if layout.src_color.channel_count() == layout.target_color.channel_count() =>
        {
            let src_bytes = bytemuck::try_cast_slice::<u16, u8>(src_pixels)
                .map_err(|_| "Source u16 slice byte cast failed")?;
            copy_strided_rows(src_bytes, dest, layout)
        }
        ValidPixelType::U16 => dispatch_channels(
            src_pixels,
            dest,
            layout,
            PixelOps { opaque: u16::MAX, luminance: srgb_rgb16_to_luma, encode: u16::to_ne_bytes },
        ),
        ValidPixelType::U8 => match layout.downscale_mode {
            ValidDownscaleMode::TruncateHighByte => dispatch_channels(
                src_pixels,
                dest,
                layout,
                PixelOps {
                    opaque: u16::MAX,
                    luminance: srgb_rgb16_to_luma,
                    encode: |v| [(v >> BITS_PER_BYTE) as u8],
                },
            ),
            ValidDownscaleMode::Round => dispatch_channels(
                src_pixels,
                dest,
                layout,
                PixelOps {
                    opaque: u16::MAX,
                    luminance: srgb_rgb16_to_luma,
                    encode: |v| {
                        [((u32::from(v) + ROUNDING_HALF_16_TO_8) / ROUNDING_DIVISOR_16_TO_8) as u8]
                    },
                },
            ),
        },
        ValidPixelType::F32 => Err("Float32 sample conversion is not supported"),
    }
}

#[cfg(test)]
mod tests {
        use super::*;

    fn packed_u8_spec(
        width: usize,
        height: usize,
        src_color: ColorType,
        target_color: ColorType,
    ) -> ConversionSpec {
        ConversionSpec {
            width,
            height,
            src_color,
            src_pixel: PixelType::kUint8,
            target_color,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        }
    }

    #[test]
    fn gray_expands_to_rgb_by_replicating_luminance() {
        let mut rgb = [0u8; 6];
        let spec = packed_u8_spec(2, 1, ColorType::kL, ColorType::kRgb);
        let layout = validate_layout(&spec, rgb.len()).unwrap();
        convert_u8_pixels(&[10, 200], &mut rgb, &layout).unwrap();
        assert_eq!(rgb, [10, 10, 10, 200, 200, 200]);
    }

    #[test]
    fn gray_expands_to_rgba_with_opaque_alpha() {
        let mut rgba = [0u8; 4];
        let spec = packed_u8_spec(1, 1, ColorType::kL, ColorType::kRgba);
        let layout = validate_layout(&spec, rgba.len()).unwrap();
        convert_u8_pixels(&[42], &mut rgba, &layout).unwrap();
        assert_eq!(rgba, [42, 42, 42, u8::MAX]);
    }

    #[test]
    fn gray_alpha_expands_to_rgba_preserving_alpha() {
        let mut rgba = [0u8; 4];
        let spec = packed_u8_spec(1, 1, ColorType::kLa, ColorType::kRgba);
        let layout = validate_layout(&spec, rgba.len()).unwrap();
        convert_u8_pixels(&[42, 128], &mut rgba, &layout).unwrap();
        assert_eq!(rgba, [42, 42, 42, 128]);
    }

    #[test]
    fn rgba_converts_to_luma_matching_image_crate() {
        let src = [100u8, 150, 200, 77, 255, 255, 254, 255];
        let mut luma = [0u8; 2];
        let spec = packed_u8_spec(2, 1, ColorType::kRgba, ColorType::kL);
        let layout = validate_layout(&spec, luma.len()).unwrap();
        convert_u8_pixels(&src, &mut luma, &layout).unwrap();

        let ref_img = image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(2, 1, src.to_vec()).unwrap(),
        )
        .to_luma8();
        assert_eq!(luma.as_slice(), ref_img.as_raw().as_slice());
    }

    #[test]
    fn rgba_converts_to_luma_alpha_preserving_alpha() {
        let mut luma_alpha = [0u8; 2];
        let spec = packed_u8_spec(1, 1, ColorType::kRgba, ColorType::kLa);
        let layout = validate_layout(&spec, luma_alpha.len()).unwrap();
        convert_u8_pixels(&[100, 150, 200, 77], &mut luma_alpha, &layout).unwrap();
        let expected_luma = ((2126 * 100 + 7152 * 150 + 722 * 200 + 5_000) / 10_000) as u8;
        assert_eq!(luma_alpha, [expected_luma, 77]);
    }

    #[test]
    fn rgb16_converts_to_luma16_with_half_up_rounding() {
        let src: [u16; 3] = [65535, 65535, 65534];
        let mut dest = [0u8; 2];
        let spec = ConversionSpec {
            width: 1,
            height: 1,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kL,
            target_pixel: PixelType::kUint16,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        convert_u16_pixels(&src, &mut dest, &layout).unwrap();
        assert_eq!(u16::from_ne_bytes(dest), 65535);
    }

    #[test]
    fn u16_truncate_high_byte_downscale_retains_upper_byte() {
        let src: [u16; 3] = [0x00FF, 0x12FE, 0xFFFF];
        let mut truncated = [0u8; 3];
        let spec = ConversionSpec {
            width: 1,
            height: 1,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, truncated.len()).unwrap();
        convert_u16_pixels(&src, &mut truncated, &layout).unwrap();
        assert_eq!(truncated, [0x00, 0x12, 0xFF]);
    }

    #[test]
    fn u16_round_downscale_matches_image_crate_rounding() {
        let src: [u16; 3] = [0x00FF, 0x12FE, 0xFFFF];
        let mut rounded = [0u8; 3];
        let spec = ConversionSpec {
            width: 1,
            height: 1,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kRound,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, rounded.len()).unwrap();
        convert_u16_pixels(&src, &mut rounded, &layout).unwrap();
        assert_eq!(rounded, [0x01, 0x13, 0xFF]);
    }

    #[test]
    fn u8_to_u16_supports_odd_row_stride() {
        let mut dest = [0xAAu8; 11];
        let spec = ConversionSpec {
            width: 1,
            height: 2,
            src_color: ColorType::kL,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kLa,
            target_pixel: PixelType::kUint16,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 7,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u8_pixels(&[0x12, 0x34], &mut dest, &layout).unwrap();
        assert_eq!(u16::from_ne_bytes([dest[0], dest[1]]), 0x1212);
        assert_eq!(u16::from_ne_bytes([dest[2], dest[3]]), u16::MAX);
        assert_eq!(&dest[4..7], &[0xAA, 0xAA, 0xAA]);
        assert_eq!(u16::from_ne_bytes([dest[7], dest[8]]), 0x3434);
        assert_eq!(u16::from_ne_bytes([dest[9], dest[10]]), u16::MAX);
    }

    #[test]
    fn validate_layout_rejects_undersized_row_bytes() {
        let spec = ConversionSpec {
            width: 4,
            height: 2,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: false,
        };
        assert!(validate_layout(&spec, 64).is_err());
    }

    #[test]
    fn validate_layout_ignores_row_bytes_for_single_row_image() {
        let spec = ConversionSpec {
            width: 2,
            height: 1,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 1,
            rows_bottom_up: true,
        };
        let layout = validate_layout(&spec, 6).unwrap();
        assert!(layout.is_identity_packed);
        assert_eq!(layout.packed_row_bytes, 6);
    }

    #[test]
    fn validate_layout_rejects_undersized_dest_buffer() {
        let spec = packed_u8_spec(4, 2, ColorType::kRgb, ColorType::kRgb);
        assert!(validate_layout(&spec, 23).is_err());
    }

    #[test]
    fn validate_layout_rejects_mismatched_f32_conversion() {
        let spec = ConversionSpec {
            width: 1,
            height: 1,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kFloat32,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        assert!(validate_layout(&spec, 16).is_err());
    }

    #[test]
    fn validate_layout_rejects_dimension_overflow() {
        let spec = ConversionSpec {
            width: usize::MAX,
            height: 2,
            src_color: ColorType::kRgba,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kRgba,
            target_pixel: PixelType::kUint16,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        assert!(validate_layout(&spec, 64).is_err());
    }

    #[test]
    fn validate_rejects_out_of_range_ffi_enum_values() {
        let bad_color = packed_u8_spec(1, 1, ColorType::kRgb, ColorType(99));
        assert_eq!(
            validate_layout(&bad_color, 4).err(),
            Some("Invalid color_type in ConversionOptions")
        );
        assert_eq!(
            validate_packed_samples(&bad_color),
            Err("Invalid color_type in ConversionOptions")
        );

        let mut bad_pixel = packed_u8_spec(1, 1, ColorType::kRgb, ColorType::kRgb);
        bad_pixel.target_pixel = PixelType(-1);
        assert_eq!(
            validate_layout(&bad_pixel, 4).err(),
            Some("Invalid pixel_type in ConversionOptions")
        );
        assert_eq!(
            validate_packed_samples(&bad_pixel),
            Err("Invalid pixel_type in ConversionOptions")
        );

        let mut bad_mode = packed_u8_spec(1, 1, ColorType::kRgb, ColorType::kRgb);
        bad_mode.downscale_mode = DownscaleMode(42);
        assert_eq!(
            validate_layout(&bad_mode, 4).err(),
            Some("Invalid downscale_mode in ConversionOptions")
        );
        assert_eq!(
            validate_packed_samples(&bad_mode),
            Err("Invalid downscale_mode in ConversionOptions")
        );
    }

    #[test]
    fn validate_packed_samples_enforces_packed_rows_and_isize_max() {
        let mut non_packed = packed_u8_spec(2, 2, ColorType::kRgb, ColorType::kRgb);
        non_packed.row_bytes = 8;
        assert!(validate_packed_samples(&non_packed).is_err());

        let mut exact_packed = packed_u8_spec(2, 2, ColorType::kRgb, ColorType::kRgb);
        exact_packed.row_bytes = 6;
        assert_eq!(validate_packed_samples(&exact_packed), Ok(12));

        let oversized = ConversionSpec {
            width: (isize::MAX as usize / 4) + 1,
            height: 1,
            src_color: ColorType::kRgba,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgba,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 0,
            rows_bottom_up: false,
        };
        assert_eq!(
            validate_packed_samples(&oversized),
            Err("Converted image buffer size overflowed")
        );
    }

    #[test]
    fn u8_identity_with_row_padding_copies_rows_preserving_padding() {
        let mut dest = [0xEEu8; 9];
        let spec = ConversionSpec {
            width: 1,
            height: 2,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u8_pixels(&[1, 2, 3, 4, 5, 6], &mut dest, &layout).unwrap();
        assert_eq!(dest, [1, 2, 3, 0xEE, 0xEE, 4, 5, 6, 0xEE]);
    }

    #[test]
    fn u16_identity_with_odd_row_stride_copies_rows_preserving_padding() {
        let mut dest = [0xCCu8; 9];
        let spec = ConversionSpec {
            width: 2,
            height: 2,
            src_color: ColorType::kL,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kL,
            target_pixel: PixelType::kUint16,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: false,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u16_pixels(&[0x1122, 0x3344, 0x5566, 0x7788], &mut dest, &layout).unwrap();
        assert_eq!(u16::from_ne_bytes([dest[0], dest[1]]), 0x1122);
        assert_eq!(u16::from_ne_bytes([dest[2], dest[3]]), 0x3344);
        assert_eq!(dest[4], 0xCC);
        assert_eq!(u16::from_ne_bytes([dest[5], dest[6]]), 0x5566);
        assert_eq!(u16::from_ne_bytes([dest[7], dest[8]]), 0x7788);
    }

    #[test]
    fn convert_rows_bottom_up_writes_rows_in_reverse_order() {
        let mut dest = [0xEEu8; 7];
        let spec = ConversionSpec {
            width: 1,
            height: 2,
            src_color: ColorType::kL,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 4,
            rows_bottom_up: true,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u8_pixels(&[0x11, 0x22], &mut dest, &layout).unwrap();
        assert_eq!(dest, [0x22, 0x22, 0x22, 0xEE, 0x11, 0x11, 0x11]);
    }

    #[test]
    fn u8_identity_bottom_up_with_row_padding_reverses_rows() {
        let mut dest = [0xEEu8; 8];
        let spec = ConversionSpec {
            width: 1,
            height: 2,
            src_color: ColorType::kRgb,
            src_pixel: PixelType::kUint8,
            target_color: ColorType::kRgb,
            target_pixel: PixelType::kUint8,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: true,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u8_pixels(&[1, 2, 3, 4, 5, 6], &mut dest, &layout).unwrap();
        assert_eq!(dest, [4, 5, 6, 0xEE, 0xEE, 1, 2, 3]);
    }

    #[test]
    fn u16_identity_bottom_up_with_odd_row_stride_reverses_rows() {
        let mut dest = [0xEEu8; 9];
        let spec = ConversionSpec {
            width: 2,
            height: 2,
            src_color: ColorType::kL,
            src_pixel: PixelType::kUint16,
            target_color: ColorType::kL,
            target_pixel: PixelType::kUint16,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: true,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        convert_u16_pixels(&[0x1122, 0x3344, 0x5566, 0x7788], &mut dest, &layout).unwrap();
        assert_eq!(u16::from_ne_bytes([dest[0], dest[1]]), 0x5566);
        assert_eq!(u16::from_ne_bytes([dest[2], dest[3]]), 0x7788);
        assert_eq!(dest[4], 0xEE);
        assert_eq!(u16::from_ne_bytes([dest[5], dest[6]]), 0x1122);
        assert_eq!(u16::from_ne_bytes([dest[7], dest[8]]), 0x3344);
    }

    #[test]
    fn f32_identity_bottom_up_reverses_rows() {
        let src_f32: [f32; 2] = [1.5, -2.25];
        let src_bytes: &[u8] = bytemuck::cast_slice(&src_f32);
        let mut dest = [0xDDu8; 9];
        let spec = ConversionSpec {
            width: 1,
            height: 2,
            src_color: ColorType::kL,
            src_pixel: PixelType::kFloat32,
            target_color: ColorType::kL,
            target_pixel: PixelType::kFloat32,
            downscale_mode: DownscaleMode::kTruncateHighByte,
            row_bytes: 5,
            rows_bottom_up: true,
        };
        let layout = validate_layout(&spec, dest.len()).unwrap();
        assert!(!layout.is_identity_packed);
        copy_strided_rows(src_bytes, &mut dest, &layout).unwrap();
        assert_eq!(f32::from_ne_bytes([dest[0], dest[1], dest[2], dest[3]]), -2.25);
        assert_eq!(dest[4], 0xDD);
        assert_eq!(f32::from_ne_bytes([dest[5], dest[6], dest[7], dest[8]]), 1.5);
    }
}
