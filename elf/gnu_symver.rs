//! Owned ELF GNU Symbol Versioning structures.
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use elf::endian::EndianParse;
use std::collections::HashMap;

/// An owned ELF symbol version requirement, analogous to
/// `elf::gnu_symver::SymbolRequirement` but holding owned data (`Vec<u8>`)
/// rather than slices for easier FFI.
#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
pub struct SymbolRequirement {
    /// The name of the required library file.
    pub file: Vec<u8>,
    /// The name of the required symbol.
    pub name: Vec<u8>,
    /// The hash of the symbol name.
    pub hash: u32,
    /// Flags associated with the requirement.
    pub flags: u16,
    /// Whether the symbol is hidden.
    pub hidden: bool,
}

make_option_type!(SymbolRequirement);
make_result_type!(SymbolRequirement);
make_result_type!(OptionSymbolRequirement);

/// An owned ELF symbol version definition, analogous to
/// `elf::gnu_symver::SymbolDefinition` but holding owned data (`Vec<u8>`,
/// `Vec<Vec<u8>>`) rather than slices for easier FFI.
#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
pub struct SymbolDefinition {
    /// The hash of the symbol name.
    pub hash: u32,
    /// Flags associated with the definition.
    pub flags: u16,
    /// A list of names associated with this definition.
    pub names: Vec<Vec<u8>>,
    /// Whether the symbol is hidden.
    pub hidden: bool,
}

impl SymbolDefinition {
    /// Returns the number of names associated with this definition.
    pub fn get_name_count(&self) -> usize {
        self.names.len()
    }

    /// Retrieves the name at the given index.
    pub fn get_name(&self, index: usize) -> anyhow::Result<Vec<u8>> {
        self.names
            .get(index)
            .cloned()
            .ok_or(anyhow!("SymbolDefinition: name index {index} out of bounds"))
    }
}

make_option_type!(SymbolDefinition);
make_result_type!(SymbolDefinition);
make_result_type!(OptionSymbolDefinition);

/// An owned ELF symbol version table, containing requirement and definition
/// entries. This struct holds owned data rather than slices for easier FFI.
///
/// The `requirements` and `definitions` fields are HashMaps where the key is
/// the symbol index from the ELF symbol table. Not all symbols have version
/// requirements or definitions, so this mapping is often sparse. `HashMap` is
/// used because it efficiently stores these sparse index-to-data mappings,
/// and allows for efficient O(1) average-time lookups via `get_requirement`
/// and `get_definition`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolVersionTable {
    requirements: HashMap<usize, SymbolRequirement>,
    definitions: HashMap<usize, SymbolDefinition>,
}

impl SymbolVersionTable {
    /// Creates a new `SymbolVersionTable` from a third-party
    /// `elf::gnu_symver::SymbolVersionTable`, converting borrowed data into
    /// owned data structures (`HashMap`, `Vec<u8>`, `Vec`).
    ///
    /// This method iterates through symbol indices `0..sym_count` and queries
    /// `tp_table` for requirements or definitions corresponding to each index.
    /// If a requirement or definition exists for a given index, it is
    /// inserted into the respective HashMap with the symbol index as the key.
    pub fn new_from_third_party<'a, E: EndianParse>(
        tp_table: &elf::gnu_symver::SymbolVersionTable<'a, E>,
        sym_count: usize,
    ) -> anyhow::Result<Self> {
        let mut requirements = HashMap::new();
        let mut definitions = HashMap::new();
        for i in 0..sym_count {
            if let Some(req) = tp_table.get_requirement(i)? {
                requirements.insert(
                    i,
                    SymbolRequirement {
                        file: req.file.into(),
                        name: req.name.into(),
                        hash: req.hash,
                        flags: req.flags,
                        hidden: req.hidden,
                    },
                );
            }
            if let Some(def) = tp_table.get_definition(i)? {
                let mut names = Vec::new();
                for name_res in def.names {
                    names.push(name_res?.into());
                }
                definitions.insert(
                    i,
                    SymbolDefinition {
                        hash: def.hash,
                        flags: def.flags,
                        hidden: def.hidden,
                        names,
                    },
                );
            }
        }
        Ok(Self { requirements, definitions })
    }

    /// Retrieves the symbol requirement for a given symbol index, if one
    /// exists.
    pub fn get_requirement(&self, sym_idx: usize) -> ResultOptionSymbolRequirement {
        try_crubit!({ Ok(self.requirements.get(&sym_idx).cloned().into()) })
    }

    /// Retrieves the symbol definition for a given symbol index, if one exists.
    pub fn get_definition(&self, sym_idx: usize) -> ResultOptionSymbolDefinition {
        try_crubit!({ Ok(self.definitions.get(&sym_idx).cloned().into()) })
    }
}

make_option_type!(SymbolVersionTable);
make_result_type!(OptionSymbolVersionTable);
