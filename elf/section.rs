use crate::{
    constants::ResultSectionType, constants::SectionType, make_option_type, make_result_type,
    try_crubit,
};
use anyhow::anyhow;
use elf::endian::EndianParse;
use elf::section::SectionHeader as ThirdPartySectionHeader;
use elf::section::SectionHeaderTable as ThirdPartySectionHeaderTable;

/// An owned ELF section header, analogous to `elf::section::SectionHeader`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct SectionHeader {
    /// An offset to a string in the `.shstrtab` section that represents the
    /// name of this section.
    pub sh_name: u32,
    /// The type of this section header.
    pub sh_type: SectionType,
    /// Section flags.
    pub sh_flags: u64,
    /// The virtual address of the section in memory, for sections that are
    /// loaded.
    pub sh_addr: u64,
    /// The offset of the section in the file image.
    pub sh_offset: u64,
    /// The size of the section in the file image.
    pub sh_size: u64,
    /// Contains section index of an associated section.
    pub sh_link: u32,
    /// Contains extra information about the section.
    pub sh_info: u32,
    /// Contains the required alignment of the section.
    pub sh_addralign: u64,
    /// Contains the size, in bytes, of each entry, for sections that contain
    /// fixed-size entries.
    pub sh_entsize: u64,
}

impl SectionHeader {
    pub fn get_type(&self) -> ResultSectionType {
        try_crubit!({ Ok(self.sh_type) })
    }
    /// Converts this owned `SectionHeader` back to a third-party
    /// `SectionHeader` struct.
    pub(crate) fn as_third_party(&self) -> ThirdPartySectionHeader {
        ThirdPartySectionHeader {
            sh_name: self.sh_name,
            sh_type: self.sh_type.0,
            sh_flags: self.sh_flags,
            sh_addr: self.sh_addr,
            sh_offset: self.sh_offset,
            sh_size: self.sh_size,
            sh_link: self.sh_link,
            sh_info: self.sh_info,
            sh_addralign: self.sh_addralign,
            sh_entsize: self.sh_entsize,
        }
    }
}

impl From<ThirdPartySectionHeader> for SectionHeader {
    fn from(third_party_section_header: ThirdPartySectionHeader) -> Self {
        Self {
            sh_name: third_party_section_header.sh_name,
            sh_type: SectionType(third_party_section_header.sh_type),
            sh_flags: third_party_section_header.sh_flags,
            sh_addr: third_party_section_header.sh_addr,
            sh_offset: third_party_section_header.sh_offset,
            sh_size: third_party_section_header.sh_size,
            sh_link: third_party_section_header.sh_link,
            sh_info: third_party_section_header.sh_info,
            sh_addralign: third_party_section_header.sh_addralign,
            sh_entsize: third_party_section_header.sh_entsize,
        }
    }
}

make_option_type!(SectionHeader);
make_result_type!(SectionHeader);
make_result_type!(OptionSectionHeader);

/// An owned ELF section header table, analogous to
/// `elf::section::SectionHeaderTable` but holding owned `SectionHeader`
/// structs in a `Vec` for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionHeaderTable {
    entries: Vec<SectionHeader>,
}

impl SectionHeaderTable {
    /// Returns the number of section headers in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Retrieves the section header at the given index.
    pub fn get(&self, index: usize) -> ResultSectionHeader {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("SectionHeaderTable: index {index} out of bounds"))
        })
    }

    /// Returns true if the table contains no section headers.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a, E: EndianParse> From<ThirdPartySectionHeaderTable<'a, E>> for SectionHeaderTable {
    fn from(third_party_section_header_table: ThirdPartySectionHeaderTable<'a, E>) -> Self {
        Self {
            entries: third_party_section_header_table
                .iter()
                .map(|third_party_section_header| third_party_section_header.into())
                .collect(),
        }
    }
}

impl From<&Vec<ThirdPartySectionHeader>> for SectionHeaderTable {
    fn from(shdrs: &Vec<ThirdPartySectionHeader>) -> Self {
        SectionHeaderTable { entries: shdrs.iter().copied().map(|shdr| shdr.into()).collect() }
    }
}

make_option_type!(SectionHeaderTable);
make_result_type!(SectionHeaderTable);
make_result_type!(OptionSectionHeaderTable);
