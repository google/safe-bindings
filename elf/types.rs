//! This module defines common data structures used by both `elf_bytes` and
//! `elf_stream` that are returned across the FFI boundary.

use crate::compression::*;
use crate::section::*;
use crate::string_table::*;
use crate::symbol::*;
use crate::{make_option_type, make_result_type};

/// A pair of a section header table and its associated string table.
/// This is used as a return type for `section_headers_with_strtab` because
/// Crubit FFI cannot handle tuples.
#[derive(Debug, Default)]
pub struct SectionHeaderTableWithStrings {
    /// The section header table.
    pub section_header_table: OptionSectionHeaderTable,
    /// The string table for section names (.shstrtab).
    pub string_table: OptionStringTable,
}
make_option_type!(SectionHeaderTableWithStrings);
make_result_type!(SectionHeaderTableWithStrings);

/// Holds section data along with optional compression information.
#[derive(Debug, Default)]
pub struct SectionData {
    /// The raw data of the section. If the section is compressed, this
    /// contains the compressed data.
    pub data: Vec<u8>,
    /// If the section is compressed, this contains the compression header.
    pub compression: OptionCompressionHeader,
}
make_result_type!(SectionData);

/// A pair of a symbol table and its associated string table.
/// This is used as a return type for methods like `symbol_table` and
/// `dynamic_symbol_table` because Crubit FFI cannot handle tuples.
#[derive(Debug, Default)]
pub struct SymbolTableWithStrings {
    /// The symbol table (.symtab or .dynsym).
    pub symbol_table: SymbolTable,
    /// The string table associated with the symbol table.
    pub string_table: StringTable,
}

make_option_type!(SymbolTableWithStrings);
make_result_type!(SymbolTableWithStrings);
make_result_type!(OptionSymbolTableWithStrings);
