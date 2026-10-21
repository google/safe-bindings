use crate::constants::DynamicTag;
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use elf::dynamic::Dyn as ThirdPartyDyn;
use elf::dynamic::DynamicTable as ThirdPartyDynTable;
use elf::endian::EndianParse;

/// An owned ELF dynamic section entry, analogous to `elf::dynamic::Dyn`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)] // for use in C++
pub struct Dyn {
    /// The type of dynamic entry.
    pub d_tag: i64,
    /// The value or address associated with the tag.
    // union of both {d_val, d_ptr}
    pub d_un: u64,
}

impl Dyn {
    /// Returns the tag type of this dynamic entry.
    pub fn d_tag(&self) -> DynamicTag {
        DynamicTag(self.d_tag)
    }

    /// Returns the value of this dynamic entry.
    pub fn d_val(&self) -> u64 {
        self.d_un
    }

    /// Returns the address of this dynamic entry.
    pub fn d_ptr(&self) -> u64 {
        self.d_un
    }
}

impl From<ThirdPartyDyn> for Dyn {
    fn from(third_party_dyn: ThirdPartyDyn) -> Self {
        Self { d_tag: third_party_dyn.d_tag, d_un: third_party_dyn.d_ptr() }
    }
}

// make option type
make_option_type!(Dyn);
// make result type
make_result_type!(Dyn);
// make result of option type
make_result_type!(OptionDyn);

/// An owned ELF dynamic table (`.dynamic`), analogous to
/// `elf::dynamic::DynamicTable` but holding owned `Dyn` structs in a `Vec`
/// for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)] // for use in C++
pub struct DynTable {
    /// The list of dynamic entries.
    pub entries: Vec<Dyn>,
}

impl DynTable {
    /// Returns the number of entries in the dynamic table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Retrieves the dynamic entry at the given index.
    pub fn get(&self, index: usize) -> ResultDyn {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("DynTable: index {index} out of bounds"))
        })
    }

    /// Returns true if the table contains no dynamic entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<E: EndianParse> From<ThirdPartyDynTable<'_, E>> for DynTable {
    fn from(third_party_dyn_table: ThirdPartyDynTable<E>) -> Self {
        Self {
            entries: third_party_dyn_table
                .iter()
                .map(|third_party_dyn| third_party_dyn.into())
                .collect(),
        }
    }
}

// make option type
make_option_type!(DynTable);
// make result type
make_result_type!(DynTable);
// make result of option type
make_result_type!(OptionDynTable);
