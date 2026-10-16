use crate::constants::{ResultSymbolBind, ResultSymbolType, ResultSymbolVisibility};
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use elf::abi;
use elf::endian::EndianParse;
use elf::symbol::Symbol as ThirdPartySymbol;
use elf::symbol::SymbolTable as ThirdPartySymbolTable;

// Needs Symbol and SymbolTable
// needs result, option, and result<option>

/// An owned ELF symbol table entry, analogous to `elf::symbol::Symbol`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct Symbol {
    /// An index into the object file's symbol string table, which holds the
    /// character representations of the symbol names.
    pub st_name: u32,
    /// The section header index in relation to which the symbol is defined.
    pub st_shndx: u16,
    /// Contains symbol type and binding attributes.
    pub(super) st_info: u8,
    /// Contains symbol visibility.
    pub(super) st_other: u8,
    /// The value of the associated symbol.
    pub st_value: u64,
    /// The size of the symbol, if any.
    pub st_size: u64,
}

impl Symbol {
    /// Returns true if the symbol is undefined (SHN_UNDEF).
    pub fn is_undefined(&self) -> bool {
        self.st_shndx == abi::SHN_UNDEF
    }

    /// Extracts the symbol type from `st_info`.
    pub fn st_symtype(&self) -> ResultSymbolType {
        try_crubit!({ (self.st_info & 0xf).try_into() })
    }

    /// Extracts the symbol binding from `st_info`.
    pub fn st_bind(&self) -> ResultSymbolBind {
        try_crubit!({ (self.st_info >> 4).try_into() })
    }

    /// Extracts the symbol visibility from `st_other`.
    pub fn st_vis(&self) -> ResultSymbolVisibility {
        try_crubit!({ (self.st_other & 0x3).try_into() })
    }
}

impl From<ThirdPartySymbol> for Symbol {
    fn from(third_party_symbol: ThirdPartySymbol) -> Self {
        Self {
            st_name: third_party_symbol.st_name,
            st_shndx: third_party_symbol.st_shndx,
            st_info: (third_party_symbol.st_bind() << 4) | third_party_symbol.st_symtype(),
            st_other: third_party_symbol.st_vis(),
            st_value: third_party_symbol.st_value,
            st_size: third_party_symbol.st_size,
        }
    }
}

make_option_type!(Symbol);
make_result_type!(Symbol);
make_result_type!(OptionSymbol);

/// An owned ELF symbol table, analogous to `elf::symbol::SymbolTable`
/// but holding owned `Symbol` structs in a `Vec` for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SymbolTable {
    pub(crate) entries: Vec<Symbol>,
}

impl SymbolTable {
    /// Returns the number of symbols in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Retrieves the symbol at the given index.
    pub fn get(&self, index: usize) -> ResultSymbol {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("SybolTable: index {index} out of bounds"))
        })
    }

    /// Returns true if the table contains no symbols.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<E: EndianParse> From<ThirdPartySymbolTable<'_, E>> for SymbolTable {
    fn from(third_party_symbol_table: ThirdPartySymbolTable<E>) -> Self {
        Self {
            entries: third_party_symbol_table
                .iter()
                .map(|third_party_symbol| third_party_symbol.into())
                .collect(),
        }
    }
}

make_option_type!(SymbolTable);
make_result_type!(SymbolTable);
make_result_type!(OptionSymbolTable);
