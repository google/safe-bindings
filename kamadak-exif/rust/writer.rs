use crate::error::Error;
use crate::make_result_type;
use crate::tiff::{Field, In};
use crate::types::VecU8;
use exif::experimental::Writer as KamadakWriter;
use std::io::Cursor;

/// The `Writer` struct is used to encode and write Exif data.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use exif_bridge_rs::types::{VecVecU8, VecU8};
/// use exif_bridge_rs::value::Value;
/// use exif_bridge_rs::tag::Tag;
/// use exif_bridge_rs::tiff::{Field, In};
/// use exif_bridge_rs::writer::Writer;
/// let image_desc = Field::new({
///     tag: Tag::ImageDescription(),
///     ifd_num: In::PRIMARY,
///     value: Value::Ascii(VecVecU8::from(vec![b"Sample".to_vec()])),
/// });
/// let mut writer = Writer::new();
/// let mut buf = VecU8::new();
/// writer.push_field(&image_desc);
/// writer.write(&mut buf, false).into()?;
/// const EXPECTED: &[u8] =
///     b"\x4d\x4d\x00\x2a\x00\x00\x00\x08\
///       \x00\x01\x01\x0e\x00\x02\x00\x00\x00\x07\x00\x00\x00\x1a\
///       \x00\x00\x00\x00\
///       Sample\0";
/// assert_eq!(buf.as_slice(), EXPECTED);
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct Writer<'a>(pub(crate) KamadakWriter<'a>);

make_result_type!(VecU8, ResultVecU8, Error);

impl<'a> Writer<'a> {
    /// Constructs an empty `Writer`.
    pub fn new() -> Self {
        Self(KamadakWriter::new())
    }

    /// Appends a field to be written.
    ///
    /// The fields can be appended in any order.
    /// Duplicate fields must not be appended.
    ///
    /// The following fields are ignored and synthesized when needed:
    /// ExifIFDPointer, GPSInfoIFDPointer, InteropIFDPointer,
    /// StripOffsets, StripByteCounts, TileOffsets, TileByteCounts,
    /// JPEGInterchangeFormat, and JPEGInterchangeFormatLength.
    pub fn push_field(&mut self, field: &'a Field) {
        self.0.push_field(&field.inner)
    }

    /// Sets TIFF strips for the specified IFD.
    /// If this method is called multiple times, the last one is used.
    pub fn set_strips(&mut self, strips: &'a [&'a [u8]], ifd_num: In) {
        self.0.set_strips(strips, ifd_num.into())
    }

    /// Sets TIFF strips for the specified IFD.
    /// If this method is called multiple times, the last one is used.
    pub fn set_tiles(&mut self, tiles: &'a [&'a [u8]], ifd_num: In) {
        self.0.set_tiles(tiles, ifd_num.into())
    }

    /// Sets JPEG data for the specified IFD.
    /// If this method is called multiple times, the last one is used.
    pub fn set_jpeg(&mut self, jpeg: &'a [u8], ifd_num: In) {
        self.0.set_jpeg(jpeg, ifd_num.into())
    }

    /// Encodes Exif data and returns it in `VecU8`.
    pub fn write(&mut self, little_endian: bool) -> ResultVecU8 {
        let mut out = VecU8::new();
        let mut w = Cursor::new(out.as_mut_vec());
        match self.0.write(&mut w, little_endian) {
            Ok(_) => ResultVecU8::from_ok(out),
            Err(e) => ResultVecU8::from(Error::from(e)),
        }
    }
}

impl<'a> Default for Writer<'a> {
    fn default() -> Self {
        Self::new()
    }
}
