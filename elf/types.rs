//! This module defines common data structures used by both `elf_bytes` and
//! `elf_stream` that are returned across the FFI boundary.

use crate::compression::*;
use crate::section::*;
use crate::string_table::*;
use crate::symbol::*;

/// A pair of a section header table and its associated string table.
#[derive(Debug, Default)]
pub struct SectionHeaderTableWithStrings {
    /// The section header table.
    pub section_header_table: Option<SectionHeaderTable>,
    /// The string table for section names (.shstrtab).
    pub string_table: Option<StringTable>,
}

/// Holds section data along with optional compression information.
#[derive(Debug, Default)]
pub struct SectionData {
    /// The raw data of the section. If the section is compressed, this
    /// contains the compressed data.
    pub data: Vec<u8>,
    /// If the section is compressed, this contains the compression header.
    pub compression: Option<CompressionHeader>,
}

/// A pair of a symbol table and its associated string table.
#[derive(Debug, Default)]
pub struct SymbolTableWithStrings {
    /// The symbol table (.symtab or .dynsym).
    pub symbol_table: SymbolTable,
    /// The string table associated with the symbol table.
    pub string_table: StringTable,
}
