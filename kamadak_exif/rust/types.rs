// For common types.
pub type OptionField = Option<crate::tiff::Field>;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct TiffExifDataRs {
    pub fields: Vec<crate::tiff::Field>,
    pub is_little_endian: bool,
}
