//! Owned ELF compression header.
use crate::{make_option_type, make_result_type};
use elf::compression::CompressionHeader as ThirdPartyCompressionHeader;

/// An owned ELF section compression header (`SHT_GROUP` with `SHF_COMPRESSED`),
/// analogous to `elf::compression::CompressionHeader` but fully owned for
/// easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct CompressionHeader {
    /// The compression algorithm used.
    pub ch_type: u32,
    /// Uncompressed size of section data.
    pub ch_size: u64,
    /// Alignment of uncompressed data.
    pub ch_addralign: u64,
}

impl From<ThirdPartyCompressionHeader> for CompressionHeader {
    fn from(third_party: ThirdPartyCompressionHeader) -> Self {
        Self {
            ch_type: third_party.ch_type,
            ch_size: third_party.ch_size,
            ch_addralign: third_party.ch_addralign,
        }
    }
}

make_option_type!(CompressionHeader);
make_result_type!(CompressionHeader);
make_result_type!(OptionCompressionHeader);
