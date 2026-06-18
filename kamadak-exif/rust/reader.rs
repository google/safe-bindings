use crate::error::Error;
use crate::make_result_type;
use crate::tag::Tag;
use crate::tiff::{Field, In, OptionField, VecField};
use crate::types::VecU8;
use exif::{Exif as KamadakExif, Reader as KamadakReader};
use std::fmt::{Debug, Formatter, Result};
use std::io::Cursor;

///
/// A struct that holds the parsed Exif attributes.
///
/// # Examples
/// ```
/// # use exif_bridge_rs::reader::Reader;
/// # use exif_bridge_rs::tag::Tag;
/// # use exif_bridge_rs::tiff::In;
/// # use exif_bridge_rs::types::VecU8;
/// # let data = std::fs::read("tests/exif.jpg").unwrap();
/// # let data_vecu8 = VecU8::from(data);
/// # let exif = Reader::new().read_from_container(data_vecu8).unwrap();
/// // Get a specific field.
/// let xres = exif.get_field(Tag::XResolution(), In::PRIMARY).unwrap();
/// // Iterate over all fields.
/// for f in exif.fields().into_vec() {
///     println!("{} {} {}", f.get_tag(), f.get_ifd(), f.get_value());
/// }
/// ```
// Default is needed to make the type movable.
// However, there is no way to directly create a "default" value for the original crate's Exif.
// So we are using Option<KamadakExif> as the underlying type, and use Option::None to represent
// the "default" value.
#[derive(Default)]
pub struct Exif {
    pub(crate) inner: Option<KamadakExif>,
    pub(crate) mnote_type: crate::mnote::MakerNoteType,
    pub(crate) mnote_fields: Vec<crate::mnote::MnoteField>,
}

impl Exif {
    pub fn new(inner: Option<KamadakExif>) -> Self {
        let mut mnote_type = crate::mnote::MakerNoteType::None;
        let mut mnote_fields = Vec::new();

        if let Some(ref exif_data) = inner {
            let mut make_str = String::new();
            if let Some(field) =
                exif_data.fields().find(|f| f.tag.number() == exif::Tag::Make.number())
                && let exif::Value::Ascii(ref v) = field.value
                && let Some(first) = v.iter().next()
                && let Ok(s) = std::str::from_utf8(first)
            {
                make_str = s.to_string();
            }

            if let Some(mnote_field) =
                exif_data.fields().find(|f| f.tag.number() == exif::Tag::MakerNote.number())
                && let exif::Value::Undefined(ref mnote_bytes, _) = mnote_field.value
            {
                let endian = exif_data.little_endian();
                let tiff_buf = exif_data.buf();
                let (parsed_type, parsed_fields) =
                    crate::mnote::parse_makernote(&make_str, mnote_bytes, tiff_buf, endian);
                mnote_type = parsed_type;
                mnote_fields = parsed_fields;
            }
        }

        Self { inner, mnote_type, mnote_fields }
    }

    /// Returns the TIFF data as a vector.
    pub fn buf(&self) -> VecU8 {
        match &self.inner {
            Some(exif) => exif.buf().into(),
            None => VecU8::new(),
        }
    }

    /// Returns a vector of Exif fields.
    pub fn fields(&self) -> VecField {
        // Return type of the original function is actually just an opaque type that implements
        // ExactSizeIterator<Item = &Field> trait, so we are just using VecField instead.
        match &self.inner {
            Some(exif) => exif.fields().cloned().map(Field::from).collect::<Vec<Field>>().into(),
            None => VecField::new(),
        }
    }

    /// Returns a vector of MakerNote fields.
    pub fn mnote_fields(&self) -> VecField {
        let mut fields = Vec::new();
        for field in &self.mnote_fields {
            let synthetic_field = exif::Field {
                tag: exif::Tag(exif::Context::Exif, field.tag),
                ifd_num: exif::In(0),
                value: field.value.0.clone(),
            };
            fields.push(Field::from(synthetic_field));
        }
        fields.into()
    }

    /// Returns true if the Exif data (TIFF structure) is in the
    /// little-endian byte order.
    pub fn little_endian(&self) -> bool {
        match &self.inner {
            Some(exif) => exif.little_endian(),
            None => false,
        }
    }

    /// Returns a reference to the Exif field specified by the tag
    /// and the IFD number.
    pub fn get_field(&self, tag: Tag, ifd_num: In) -> OptionField {
        match &self.inner {
            Some(exif) => match exif.get_field(tag.into_inner(), ifd_num.into()) {
                Some(field) => OptionField::from_some(Field::from(field.clone())),
                None => OptionField::from(None),
            },
            None => OptionField::from(None),
        }
    }

    /// Returns the MakerNotes type (0=None, 1=Unknown, 2=Canon, etc.)
    pub fn get_mnote_type(&self) -> u32 {
        self.mnote_type as u32
    }

    /// Returns the MakerNote field specified by the tag number.
    pub fn get_mnote_field(&self, tag_num: u32) -> OptionField {
        for field in &self.mnote_fields {
            if field.tag as u32 == tag_num {
                let synthetic_field = exif::Field {
                    tag: exif::Tag(exif::Context::Exif, tag_num as u16),
                    ifd_num: exif::In(0),
                    value: field.value.0.clone(),
                };
                return OptionField::from_some(Field::from(synthetic_field));
            }
        }
        OptionField::from(None)
    }
}

impl From<KamadakExif> for Exif {
    fn from(exif: KamadakExif) -> Self {
        Self::new(Some(exif))
    }
}

impl Debug for Exif {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self.inner {
            Some(exif) => {
                let mut make_str = String::new();
                if let Some(field) =
                    exif.fields().find(|f| f.tag.number() == exif::Tag::Make.number())
                    && let exif::Value::Ascii(ref v) = field.value
                    && let Some(first) = v.iter().next()
                    && let Ok(s) = std::str::from_utf8(first)
                {
                    make_str = s.to_string();
                }

                let mnote_tag_present =
                    exif.fields().any(|f| f.tag.number() == exif::Tag::MakerNote.number());
                f.debug_struct("Exif")
                    .field("buffer", &exif.buf())
                    .field("little_endian", &exif.little_endian())
                    .field("mnote_type", &self.mnote_type)
                    .field("mnote_fields_count", &self.mnote_fields.len())
                    .field("detected_make", &make_str)
                    .field("mnote_tag_present", &mnote_tag_present)
                    .finish()
            }
            None => f.debug_struct("Exif: None").finish(),
        }
    }
}

make_result_type!(Exif, ResultExif, Error);

#[repr(transparent)]
pub struct Reader(KamadakReader);

impl Reader {
    pub fn new() -> Self {
        Self(KamadakReader::new())
    }

    /// Sets the option to continue parsing on non-fatal errors.
    ///
    /// When this option is enabled, the parser will not stop on non-fatal
    /// errors and returns the results as far as they can be parsed.
    /// In such a case, `read_raw` and `read_from_container`
    /// return `KamadakError::PartialResult`.
    ///
    /// `KamadakError::PartialResult` variant contains a partial `Exif` object.
    /// The current bridging mechanism to C++'s `absl::Status` does not easily support
    /// propagating this partial result data, effectively disabling the usefulness of
    /// `continue_on_error` when crossing the FFI boundary.
    ///
    /// This wrapper method is kept in case future improvements allow better propagation of
    /// custom error payloads.
    pub fn continue_on_error(&mut self, continue_on_error: bool) -> &mut Self {
        self.0.continue_on_error(continue_on_error);
        self
    }

    /// Parses the Exif attributes from raw Exif data.
    /// If an error occurred, `exif_bridge_rs::Error` is returned.
    pub fn read_raw(&self, data: VecU8) -> ResultExif {
        match self.0.read_raw(data.into_vec()) {
            Ok(exif) => ResultExif::from(Ok(Exif::from(exif))),
            Err(error) => ResultExif::from(Error::from(error)),
        }
    }

    /// Reads an image file and parses the Exif attributes in it.
    /// If an error occurred, `exif_bridge_rs::Error` is returned.
    ///
    /// Supported formats are:
    /// - TIFF and some RAW image formats based on it
    /// - JPEG
    /// - HEIF and coding-specific variations including HEIC and AVIF
    /// - PNG
    /// - WebP
    // The original function requires a generic parameter implementing io::BufRead + io::Seek.
    // std::fs::File could be used but I couldn't figure out a way to make Crubit accept
    // std::fs::File, so we are just using VecU8 for now, and leave reading file to the C++ side.
    // This makes the method largely similar to read_raw(), except for some minor preprocessing
    // at the start based on file type.
    pub fn read_from_container(&self, data: VecU8) -> ResultExif {
        let mut reader = Cursor::new(data.into_vec());
        match self.0.read_from_container(&mut reader) {
            Ok(exif) => ResultExif::from_ok(Exif::from(exif)),
            Err(error) => ResultExif::from(Error::from(error)),
        }
    }
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod rust_tests {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_mock_makernote() {
        let data = vec![
            // TIFF Header (Offset 0)
            b'M', b'M', 0, 42, 0, 0, 0, 8, // IFD 0 (Offset 8)
            0, 2, // Count: 2
            // Entry 1: Make (0x010f), Type: ASCII (2), Count: 6, Offset: 38
            0x01, 0x0f, 0, 2, 0, 0, 0, 6, 0, 0, 0, 38,
            // Entry 2: MakerNote (0x927c), Type: UNDEFINED (7), Count: 18, Offset: 44
            0x92, 0x7c, 0, 7, 0, 0, 0, 18, 0, 0, 0, 44, // Next IFD Offset (Offset 34)
            0, 0, 0, 0, // Make string "Canon\0" (Offset 38)
            b'C', b'a', b'n', b'o', b'n', b'\0',
            // MakerNote payload (Offset 44) - Canon MakerNote (no header, direct IFD)
            0, 1, // Count: 1
            // Sub-tag: 3, Type: SHORT (3), Count: 1, Value: 42 (0x002a)
            0x00, 0x03, 0, 3, 0, 0, 0, 1, 0, 42, 0, 0, // Next IFD Offset (Offset 58)
            0, 0, 0, 0,
        ];

        let exif = exif::Reader::new().read_raw(data).unwrap();
        let parsed_exif = Exif::from(exif);
        assert_eq!(parsed_exif.mnote_type, crate::mnote::MakerNoteType::Canon);
        assert_eq!(parsed_exif.mnote_fields.len(), 1);
        assert_eq!(parsed_exif.mnote_fields[0].tag, 3);
    }
}
