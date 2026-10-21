//! Owned ELF relocation sections.
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use elf::endian::EndianParse;
use elf::file::Class;
use elf::relocation::{
    Rel as ThirdPartyRel, RelIterator as ThirdPartyRelIterator, Rela as ThirdPartyRela,
    RelaIterator as ThirdPartyRelaIterator,
};

/// An owned ELF `Rel` relocation entry, analogous to `elf::relocation::Rel`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct Rel {
    /// The location at which to apply the relocation action.
    pub r_offset: u64,
    /// The symbol table index with respect to which the relocation is made.
    pub r_sym: u32,
    /// The type of relocation to apply.
    pub r_type: u32,
}

impl From<ThirdPartyRel> for Rel {
    fn from(rel: ThirdPartyRel) -> Self {
        Self { r_offset: rel.r_offset, r_sym: rel.r_sym, r_type: rel.r_type }
    }
}
make_option_type!(Rel);
make_result_type!(Rel);
make_result_type!(OptionRel);

/// An owned table of ELF `Rel` relocation entries, analogous to iterating over
/// `elf::relocation::RelIterator` but holding owned `Rel` structs in a `Vec`
/// for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelTable {
    entries: Vec<Rel>,
}

impl RelTable {
    /// Creates a new `RelTable` by parsing `.rel` section data.
    pub fn new<E: EndianParse>(endian: E, class: Class, data: &[u8]) -> Self {
        let entries = ThirdPartyRelIterator::new(endian, class, data).map(|r| r.into()).collect();
        Self { entries }
    }

    /// Returns the number of relocation entries in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the table contains no relocation entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves the relocation entry at the given index.
    pub fn get(&self, index: usize) -> ResultRel {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("RelTable: index {index} out of bounds"))
        })
    }
}
make_option_type!(RelTable);
make_result_type!(RelTable);
make_result_type!(OptionRelTable);

/// An owned ELF `Rela` relocation entry, analogous to `elf::relocation::Rela`
/// but fully owned for easier FFI. This is a relocation entry with an
/// explicit addend.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct Rela {
    /// The location at which to apply the relocation action.
    pub r_offset: u64,
    /// The symbol table index with respect to which the relocation is made.
    pub r_sym: u32,
    /// The type of relocation to apply.
    pub r_type: u32,
    /// The explicit addend to use in the relocation calculation.
    pub r_addend: i64,
}

impl From<ThirdPartyRela> for Rela {
    fn from(rela: ThirdPartyRela) -> Self {
        Self {
            r_offset: rela.r_offset,
            r_sym: rela.r_sym,
            r_type: rela.r_type,
            r_addend: rela.r_addend,
        }
    }
}
make_option_type!(Rela);
make_result_type!(Rela);
make_result_type!(OptionRela);

/// An owned table of ELF `Rela` relocation entries, analogous to iterating over
/// `elf::relocation::RelaIterator` but holding owned `Rela` structs in a `Vec`
/// for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelaTable {
    entries: Vec<Rela>,
}

impl RelaTable {
    /// Creates a new `RelaTable` by parsing `.rela` section data.
    pub fn new<E: EndianParse>(endian: E, class: Class, data: &[u8]) -> Self {
        let entries = ThirdPartyRelaIterator::new(endian, class, data).map(|r| r.into()).collect();
        Self { entries }
    }
    /// Returns the number of relocation entries in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the table contains no relocation entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves the relocation entry at the given index.
    pub fn get(&self, index: usize) -> ResultRela {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("RelaTable: index {index} out of bounds"))
        })
    }
}
make_option_type!(RelaTable);
make_result_type!(RelaTable);
make_result_type!(OptionRelaTable);
