use crate::error::Error;
use crate::tag::Tag;
use crate::types::TiffExifDataRs;
use crate::value::Value;
use crubit_annotate::cpp_enum;
use exif::{parse_exif as kamadak_parse_exif, Field as KamadakField, In as KamadakIn};

/// An IFD number.
///
/// The IFDs are indexed from 0.  The 0th IFD is for the primary image
/// and the 1st one is for the thumbnail.  Two associated constants,
/// `In::PRIMARY` and `In::THUMBNAIL`, are defined for them respectively.
///
/// # Examples
/// ```
/// use exif_bridge_rs::In;
/// assert_eq!(In::PRIMARY, 0);
/// assert_eq!(In::THUMBNAIL, 1);
/// ```
#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct In(pub i32);

impl In {
    pub const PRIMARY: In = In(KamadakIn::PRIMARY.0 as i32);
    pub const THUMBNAIL: In = In(KamadakIn::THUMBNAIL.0 as i32);
    pub const UNKNOWN: In = In(-1);
}

impl From<KamadakIn> for In {
    fn from(in_: KamadakIn) -> Self {
        match in_ {
            KamadakIn::PRIMARY => In::PRIMARY,
            KamadakIn::THUMBNAIL => In::THUMBNAIL,
            _ => In::UNKNOWN,
        }
    }
}

impl From<In> for KamadakIn {
    fn from(in_: In) -> Self {
        match in_ {
            In::PRIMARY => KamadakIn::PRIMARY,
            In::THUMBNAIL => KamadakIn::THUMBNAIL,
            // Default to PRIMARY if the IFD is not known.
            _ => KamadakIn::PRIMARY,
        }
    }
}

/// Parse the Exif attributes in the TIFF format.
///
/// Returns a Vec of Exif fields and a bool.
/// The boolean value is true if the data is little endian.
/// If an error occurred, `exif_bridge_rs::Error` is returned.
pub fn parse_exif(data: &[u8]) -> Result<TiffExifDataRs, Error> {
    kamadak_parse_exif(data)
        .map(|(fields, is_little_endian)| TiffExifDataRs {
            fields: fields.into_iter().map(Field::from).collect::<Vec<Field>>().into(),
            is_little_endian,
        })
        .map_err(Error::from)
}

/// A TIFF/Exif field.
#[derive(Debug, Clone, Default)]
pub struct Field {
    pub(crate) inner: Option<KamadakField>,
    pub(crate) tag: Tag,
    pub(crate) ifd_num: In,
}

impl Field {
    /// Constructs a new `Field` with the given tag, IFD number, and value.
    pub fn new(tag: Tag, ifd_num: In, value: Value) -> Self {
        Self {
            tag,
            ifd_num,
            inner: Some(KamadakField {
                tag: tag.into(),
                ifd_num: ifd_num.into(),
                value: value.into(),
            }),
        }
    }

    /// Return tag value as the wrapped type `Tag`.
    pub fn get_tag(&self) -> Tag {
        self.tag
    }

    /// Return IFD number as the wrapped type `In`.
    pub fn get_ifd(&self) -> In {
        self.ifd_num
    }

    /// Return value as the wrapped type `Value`.
    pub fn get_value(&self) -> Value {
        self.inner.as_ref().unwrap().value.clone().into()
    }
}

impl PartialEq for Field {
    fn eq(&self, other: &Field) -> bool {
        self.tag == other.tag
            && self.ifd_num == other.ifd_num
            && match (self.inner.as_ref(), other.inner.as_ref()) {
                (Some(a), Some(b)) => Value::value_eq(&a.value, &b.value),
                (None, None) => true,
                _ => false,
            }
    }
}

impl From<KamadakField> for Field {
    fn from(f: KamadakField) -> Self {
        Self { tag: Tag::from(f.tag), ifd_num: In::from(f.ifd_num), inner: Some(f) }
    }
}

impl From<Field> for KamadakField {
    fn from(f: Field) -> Self {
        f.inner.unwrap()
    }
}

impl AsRef<KamadakField> for Field {
    fn as_ref(&self) -> &KamadakField {
        self.inner.as_ref().unwrap()
    }
}
