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
#[repr(transparent)]
pub struct Exif(Option<KamadakExif>);

impl Exif {
    /// Returns the TIFF data as a vector.
    pub fn buf(&self) -> VecU8 {
        match &self.0 {
            Some(exif) => exif.buf().into(),
            None => VecU8::new(),
        }
    }

    /// Returns a vector of Exif fields.
    pub fn fields(&self) -> VecField {
        // Return type of the original function is actually just an opaque type that implements
        // ExactSizeIterator<Item = &Field> trait, so we are just using VecField instead.
        match &self.0 {
            Some(exif) => exif.fields().cloned().map(Field::from).collect::<Vec<Field>>().into(),
            None => VecField::new(),
        }
    }

    /// Returns true if the Exif data (TIFF structure) is in the
    /// little-endian byte order.
    pub fn little_endian(&self) -> bool {
        match &self.0 {
            Some(exif) => exif.little_endian(),
            None => false,
        }
    }

    /// Returns a reference to the Exif field specified by the tag
    /// and the IFD number.
    pub fn get_field(&self, tag: Tag, ifd_num: In) -> OptionField {
        match &self.0 {
            Some(exif) => match exif.get_field(tag.into_inner(), ifd_num.into()) {
                Some(field) => OptionField::from_some(Field::from(field.clone())),
                None => OptionField::from(None),
            },
            None => OptionField::from(None),
        }
    }
}

impl From<KamadakExif> for Exif {
    fn from(exif: KamadakExif) -> Self {
        Self(Some(exif))
    }
}

impl Debug for Exif {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self.0 {
            Some(exif) => f
                .debug_struct("Exif")
                .field("buffer", &exif.buf())
                .field("little_endian", &exif.little_endian())
                .finish(),
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
            Ok(exif) => ResultExif::from(Ok(Exif(Some(exif)))),
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
            Ok(exif) => ResultExif::from_ok(Exif(Some(exif))),
            Err(error) => ResultExif::from(Error::from(error)),
        }
    }
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}
