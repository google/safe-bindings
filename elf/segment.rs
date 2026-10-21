//! Owned ELF program header table structures, for safe use across FFI.
//!
//! These structs are analogous to `elf::segment::ProgramHeader` and
//! `elf::segment::SegmentTable` but hold owned data rather than references,
//! making them suitable for returning to C++.
use crate::constants::{ProgramHeaderType, ResultProgramHeaderType};
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use elf::endian::EndianParse;
use elf::segment::ProgramHeader as ThirdPartyProgramHeader;
use elf::segment::SegmentTable as ThirdPartySegmentTable;

/// An owned ELF program header, analogous to `elf::segment::ProgramHeader`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct ProgramHeader {
    /// The type of this segment.
    pub p_type: ProgramHeaderType,
    /// The offset of the segment in the file image.
    pub p_offset: u64,
    /// The virtual address of the segment in memory.
    pub p_vaddr: u64,
    /// The physical address of the segment in memory.
    pub p_paddr: u64,
    /// The size of the segment in the file image.
    pub p_filesz: u64,
    /// The size of the segment in memory.
    pub p_memsz: u64,
    /// Segment flags.
    pub p_flags: u32,
    /// The required alignment of the segment in memory and in the file.
    pub p_align: u64,
}

impl ProgramHeader {
    /// Returns the type of this segment as a `ProgramHeaderType`.
    pub fn get_type(&self) -> ResultProgramHeaderType {
        try_crubit!({ Ok(self.p_type) })
    }

    /// Converts this owned ProgramHeader back to a third-party ProgramHeader
    /// struct, which is needed to call methods on the underlying
    /// `elf::ElfBytes` that expect it (e.g., `segment_data`).
    pub(crate) fn as_third_party(&self) -> ThirdPartyProgramHeader {
        ThirdPartyProgramHeader {
            p_type: self.p_type.0,
            p_offset: self.p_offset,
            p_vaddr: self.p_vaddr,
            p_paddr: self.p_paddr,
            p_filesz: self.p_filesz,
            p_memsz: self.p_memsz,
            p_flags: self.p_flags,
            p_align: self.p_align,
        }
    }
}

impl From<ThirdPartyProgramHeader> for ProgramHeader {
    fn from(third_party: ThirdPartyProgramHeader) -> Self {
        Self {
            p_type: ProgramHeaderType(third_party.p_type),
            p_offset: third_party.p_offset,
            p_vaddr: third_party.p_vaddr,
            p_paddr: third_party.p_paddr,
            p_filesz: third_party.p_filesz,
            p_memsz: third_party.p_memsz,
            p_flags: third_party.p_flags,
            p_align: third_party.p_align,
        }
    }
}

make_option_type!(ProgramHeader);
make_result_type!(ProgramHeader);
make_result_type!(OptionProgramHeader);

/// An owned ELF segment table (program header table), analogous to
/// `elf::segment::SegmentTable` but holding owned `ProgramHeader` structs in
/// a `Vec` for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct SegmentTable {
    entries: Vec<ProgramHeader>,
}

impl SegmentTable {
    /// Returns the number of segments in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Retrieves the program header for the segment at the given index.
    ///
    /// Returns `Ok(ProgramHeader)` if the index is valid, or an error if
    /// `index` is out of bounds.
    pub fn get(&self, index: usize) -> ResultProgramHeader {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("SegmentTable: index {index} out of bounds"))
        })
    }

    /// Returns true if the table contains no segments.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a, E: EndianParse> From<ThirdPartySegmentTable<'a, E>> for SegmentTable {
    fn from(third_party: ThirdPartySegmentTable<'a, E>) -> Self {
        Self { entries: third_party.iter().map(|entry| entry.into()).collect() }
    }
}

impl From<&Vec<ThirdPartyProgramHeader>> for SegmentTable {
    fn from(phdrs: &Vec<ThirdPartyProgramHeader>) -> Self {
        Self { entries: phdrs.iter().copied().map(|phdr| phdr.into()).collect() }
    }
}

make_option_type!(SegmentTable);
make_result_type!(SegmentTable);
make_result_type!(OptionSegmentTable);
