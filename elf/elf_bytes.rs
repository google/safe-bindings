//! This module provides an owned, FFI-friendly wrapper around the `elf` crate.
//!
//! The primary entry point is the `ElfBytes` struct, which parses ELF data
//! provided from C++ via a byte slice and provides methods to access ELF
//! structures like headers, sections, segments, and symbol tables.
//!
//! ## Design
//!
//! The underlying `elf` crate operates on borrowed data (`&[u8]`) and returns
//! lazy-parsing iterators or structs holding references to the input slice.
//! This is efficient in Rust but incompatible with FFI, as lifetimes cannot be
//! safely managed across the C++/Rust boundary.
//!
//! This wrapper solves this by:
//! 1. Copying the input byte slice into an owned `Vec<u8>` in `ElfBytes::parse`.
//! 2. Re-parsing the ELF data on method calls to avoid lifetime issues with self-referential
//!    structs across FFI.
//! 3. All public methods return owned data structures (e.g., `Vec<T>`, `String`, structs containing
//!    them) rather than borrowed slices or iterators. Data from section/segment iterators in the
//!    underlying library is eagerly collected into `Vec`s.
//!
//! This design ensures that all data returned to C++ is fully owned by Rust
//! wrapper types, and its lifetime is managed correctly by Crubit's generated
//! bindings. Helper macros like `make_option_type!` and `make_result_type!`
//! are used to generate FFI-safe `Option` and `Result` wrappers.

use crate::dynamic::*;
use crate::file::*;
use crate::gnu_symver::*;
use crate::hash::*;
use crate::note::*;
use crate::relocation::*;
use crate::section::*;
use crate::segment::*;
use crate::string_table::*;
use crate::symbol::*;
use crate::types::*;
use crate::{make_option_type, make_result_type, try_crubit};

use anyhow::anyhow;
use elf::abi;
use elf::endian;
use elf::ElfBytes as ThirdPartyElfBytes;

// NOTE: b/367916605 - Swap these with a real `Result<T>` and `Option<T>` once available.

make_result_type!(u16, ResultU16);

/// The result of a symbol lookup in a hash table.
#[derive(Debug, Default)]
pub struct SymbolFindResult {
    /// The index of the found symbol in the symbol table.
    pub index: usize,
    /// The found symbol.
    pub symbol: Symbol,
}
make_option_type!(SymbolFindResult);
make_result_type!(OptionSymbolFindResult);

impl SysVHashTable {
    /// Finds a symbol by name in the given symbol and string tables using
    /// this SysV hash table.
    pub fn find_symbol(
        &self,
        name: &[u8],
        symbol_table: &SymbolTable,
        string_table: &StringTable,
    ) -> ResultOptionSymbolFindResult {
        try_crubit!({
            Ok(self
                .find(name, symbol_table, string_table)?
                .map(|(index, symbol)| SymbolFindResult { index, symbol })
                .into())
        })
    }
}

impl GnuHashTable {
    /// Finds a symbol by name in the given symbol and string tables using
    /// this GNU hash table.
    pub fn find_symbol(
        &self,
        name: &[u8],
        symbol_table: &SymbolTable,
        string_table: &StringTable,
    ) -> ResultOptionSymbolFindResult {
        try_crubit!({
            Ok(self
                .find(name, symbol_table, string_table)?
                .map(|(index, symbol)| SymbolFindResult { index, symbol })
                .into())
        })
    }
}

/// Holds commonly accessed ELF section data, such as symbol tables,
/// string tables, and dynamic section information.
#[derive(Debug, Default)]
pub struct CommonElfData {
    /// The `.symtab` section.
    pub symtab: OptionSymbolTable,
    /// The string table for `.symtab`.
    pub symtab_strs: OptionStringTable,
    /// The `.dynsym` section.
    pub dynsyms: OptionSymbolTable,
    /// The string table for `.dynsym`.
    pub dynsyms_strs: OptionStringTable,
    /// The `.dynamic` section.
    pub dynamic: OptionDynTable,
    /// The `.hash` section (SysV hash table).
    pub sysv_hash: OptionSysVHashTable,
    /// The `.gnu.hash` section (GNU hash table).
    pub gnu_hash: OptionGnuHashTable,
}
make_result_type!(CommonElfData);

/// An owned ELF file parser.
///
/// This struct holds an owned copy of the ELF file data and provides methods
/// to parse and return owned versions of ELF structures, suitable for use
/// across an FFI boundary with C++.
#[derive(Debug, Default)]
pub struct ElfBytes {
    /// Owns the ELF file data, copied from the C++ string_view.
    buffer: Vec<u8>,
}

impl ElfBytes {
    /// Returns a temporary parser over the buffer or an error if parsing fails
    /// or buffer is empty.
    fn elf_or_err(&self) -> anyhow::Result<ThirdPartyElfBytes<'_, endian::AnyEndian>> {
        if self.buffer.is_empty() {
            return Err(anyhow!("Empty Elf: ElfBytes was default-constructed"));
        }
        ThirdPartyElfBytes::minimal_parse(&self.buffer)
            .map_err(|e| anyhow!("ELF parse failed: {}", e))
    }

    /// Returns the ELF file header.
    pub fn file_header(&self) -> ResultFileHeader {
        try_crubit!({ Ok(self.elf_or_err()?.ehdr.into()) })
    }
}

make_result_type!(ElfBytes);

impl ElfBytes {
    /// Parses `elf_data` into an `ElfBytes` instance.
    ///
    /// This function copies the data from the given byte slice into an
    /// owned buffer and performs an initial validation to ensure it is a
    /// valid ELF file.
    ///
    /// Returns `Ok(ElfBytes)` on success, or an error if `elf_data` cannot be
    /// parsed as an ELF file.
    pub fn parse(elf_data: &[u8]) -> ResultElfBytes {
        try_crubit!({
            let buffer = elf_data.to_vec();

            // Check that it's valid ELF.
            ThirdPartyElfBytes::<endian::AnyEndian>::minimal_parse(&buffer)
                .map_err(|e| anyhow!("ELF parse failed: {}", e))?;

            Ok(ElfBytes { buffer })
        })
    }

    /// Returns the ELF segment table (program header table) of the ELF file, if
    /// one is present.
    ///
    /// The segment table is an array of program headers, each describing a
    /// segment or other information required to prepare the program for
    /// execution. If the ELF file does not contain a segment table, this
    /// method returns `Ok(None)`.
    ///
    /// Returns an error if parsing fails or if the `ElfBytes` instance is
    /// uninitialized.
    pub fn segments(&self) -> ResultOptionSegmentTable {
        try_crubit!({ Ok(self.elf_or_err()?.segments().map(|table| table.into()).into()) })
    }

    /// Returns the raw data for the segment specified by `program_header`.
    ///
    /// This method reads the segment's data as defined by the program
    /// header's file offset and size, and returns it as an owned byte
    /// vector (`Vec<u8>`). This data represents the segment's content as stored
    /// in the ELF file.
    ///
    /// Returns an error if parsing fails, if the `ElfBytes` instance is
    /// uninitialized, or if the `program_header` specifies an invalid data
    /// range (e.g., outside the file bounds).
    pub fn segment_data(&self, program_header: &ProgramHeader) -> anyhow::Result<Vec<u8>> {
        let phdr = program_header.as_third_party();
        let data = self.elf_or_err()?.segment_data(&phdr)?;
        Ok(data.to_vec())
    }

    /// Returns the data for the given segment, interpreted as a note table.
    /// Fails if the segment type is not `PT_NOTE`.
    pub fn segment_data_as_notes(&self, program_header: &ProgramHeader) -> ResultNoteTable {
        try_crubit!({
            let elf = self.elf_or_err()?;
            let phdr = program_header.as_third_party();
            if phdr.p_type != abi::PT_NOTE {
                return Err(anyhow!(
                    "Unexpected segment type: expected PT_NOTE, got {}",
                    phdr.p_type
                ));
            }
            let data = elf.segment_data(&phdr)?;
            Ok(NoteTable::new(
                elf.ehdr.endianness,
                elf.ehdr.class,
                program_header.p_align as usize,
                data,
            ))
        })
    }

    /// Returns the section header table of the ELF file, if present.
    pub fn section_headers(&self) -> ResultOptionSectionHeaderTable {
        try_crubit!({ Ok(self.elf_or_err()?.section_headers().map(|table| table.into()).into()) })
    }

    /// Returns the section header table along with the section name string table,
    /// if present.
    pub fn section_headers_with_strtab(&self) -> ResultSectionHeaderTableWithStrings {
        try_crubit!({
            let elf = self.elf_or_err()?;
            let shdrs = match elf.section_headers() {
                Some(shdrs) => shdrs,
                None => {
                    return Ok(SectionHeaderTableWithStrings {
                        section_header_table: None.into(),
                        string_table: None.into(),
                    })
                }
            };

            if elf.ehdr.e_shstrndx == abi::SHN_UNDEF {
                return Ok(SectionHeaderTableWithStrings {
                    section_header_table: Some(shdrs.into()).into(),
                    string_table: None.into(),
                });
            }

            let mut shstrndx = elf.ehdr.e_shstrndx as usize;
            if elf.ehdr.e_shstrndx == abi::SHN_XINDEX {
                let shdr_0 = shdrs.get(0)?;
                shstrndx = shdr_0.sh_link as usize;
            }

            let strtab_shdr = shdrs.get(shstrndx)?;
            let (strtab_data, _compression) = elf.section_data(&strtab_shdr)?;

            Ok(SectionHeaderTableWithStrings {
                section_header_table: Some(shdrs.into()).into(),
                string_table: Some(StringTable::new(strtab_data)).into(),
            })
        })
    }

    /// Finds a section header by its name (e.g., ".symtab", ".text").
    ///
    /// Returns an error if `name` is not valid UTF-8.
    pub fn section_header_by_name(&self, name: &[u8]) -> ResultOptionSectionHeader {
        try_crubit!({
            // The upstream `elf` crate matches section names as `&str`, so the
            // name must be valid UTF-8. Validate here rather than at the FFI
            // boundary so that C++ callers keep getting a descriptive error
            // instead of having to pre-validate.
            let name_str = std::str::from_utf8(name).map_err(|_| {
                anyhow!("Section name {:?} is not valid UTF-8", String::from_utf8_lossy(name))
            })?;
            Ok(self.elf_or_err()?.section_header_by_name(name_str)?.map(|shdr| shdr.into()).into())
        })
    }

    /// Finds and returns common ELF sections like symbol tables, string tables,
    /// dynamic section, and hash tables. This is more efficient than calling
    /// individual methods if multiple sections are needed, as it iterates
    /// through section headers only once.
    pub fn find_common_data(&self) -> ResultCommonElfData {
        try_crubit!({
            let elf = self.elf_or_err()?;
            let shdrs = match elf.section_headers() {
                Some(shdrs) => shdrs,
                None => {
                    return Ok(CommonElfData::default());
                }
            };

            let mut data = CommonElfData::default();
            for shdr in shdrs.iter() {
                match shdr.sh_type {
                    abi::SHT_SYMTAB => {
                        let strtab_shdr = shdrs.get(shdr.sh_link as usize)?;
                        let (symtab_data, _comp) = elf.section_data(&shdr)?;
                        let (strtab_data, _comp) = elf.section_data(&strtab_shdr)?;
                        let tp_symtab = elf::symbol::SymbolTable::new(
                            elf.ehdr.endianness,
                            elf.ehdr.class,
                            symtab_data,
                        );
                        data.symtab = Some(SymbolTable::from(tp_symtab)).into();
                        data.symtab_strs = Some(StringTable::new(strtab_data)).into();
                    }
                    abi::SHT_DYNSYM => {
                        let strtab_shdr = shdrs.get(shdr.sh_link as usize)?;
                        let (symtab_data, _comp) = elf.section_data(&shdr)?;
                        let (strtab_data, _comp) = elf.section_data(&strtab_shdr)?;
                        let tp_symtab = elf::symbol::SymbolTable::new(
                            elf.ehdr.endianness,
                            elf.ehdr.class,
                            symtab_data,
                        );
                        data.dynsyms = Some(SymbolTable::from(tp_symtab)).into();
                        data.dynsyms_strs = Some(StringTable::new(strtab_data)).into();
                    }
                    abi::SHT_DYNAMIC => {
                        let (dynamic_data, _comp) = elf.section_data(&shdr)?;
                        let tp_dynamic = elf::dynamic::DynamicTable::new(
                            elf.ehdr.endianness,
                            elf.ehdr.class,
                            dynamic_data,
                        );
                        data.dynamic = Some(DynTable::from(tp_dynamic)).into();
                    }
                    abi::SHT_HASH => {
                        let (hash_data, _comp) = elf.section_data(&shdr)?;
                        data.sysv_hash = Some(SysVHashTable::new(
                            elf.ehdr.endianness,
                            elf.ehdr.class,
                            hash_data,
                        )?)
                        .into();
                    }
                    abi::SHT_GNU_HASH => {
                        let (gnu_hash_data, _comp) = elf.section_data(&shdr)?;
                        data.gnu_hash = Some(GnuHashTable::new(
                            elf.ehdr.endianness,
                            elf.ehdr.class,
                            gnu_hash_data,
                        )?)
                        .into();
                    }
                    _ => continue,
                }

                if data.symtab.is_some()
                    && data.dynsyms.is_some()
                    && data.dynamic.is_some()
                    && data.sysv_hash.is_some()
                    && data.gnu_hash.is_some()
                {
                    break;
                }
            }

            if data.dynamic.is_none()
                && let Some(dyn_phdr) = elf
                    .segments()
                    .and_then(|phdrs| phdrs.iter().find(|phdr| phdr.p_type == abi::PT_DYNAMIC))
            {
                let dynamic_data = elf.segment_data(&dyn_phdr)?;
                let tp_dynamic = elf::dynamic::DynamicTable::new(
                    elf.ehdr.endianness,
                    elf.ehdr.class,
                    dynamic_data,
                );
                data.dynamic = Some(DynTable::from(tp_dynamic)).into();
            }
            Ok(data)
        })
    }

    /// Returns the data for the given section header. If the section is
    /// compressed, it returns the compressed data along with compression
    /// information.
    pub fn section_data(&self, section_header: &SectionHeader) -> ResultSectionData {
        try_crubit!({
            let shdr = section_header.as_third_party();
            let (data, compression) = self.elf_or_err()?.section_data(&shdr)?;
            Ok(SectionData {
                data: data.to_vec(),
                compression: compression.map(|ch| ch.into()).into(),
            })
        })
    }

    /// Returns the data for the given section header, interpreted as a string table.
    /// Fails if the section type is not `SHT_STRTAB`.
    pub fn section_data_as_strtab(&self, section_header: &SectionHeader) -> ResultStringTable {
        try_crubit!({
            let elf = self.elf_or_err()?;
            if section_header.sh_type != abi::SHT_STRTAB {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_STRTAB, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let (data, _compression) = elf.section_data(&shdr)?;
            Ok(StringTable::new(data))
        })
    }

    /// Returns the data for the given section header, interpreted as a `Rel`
    /// relocation table. Fails if the section type is not `SHT_REL`.
    pub fn section_data_as_rels(&self, section_header: &SectionHeader) -> ResultRelTable {
        try_crubit!({
            let elf = self.elf_or_err()?;
            if section_header.sh_type != abi::SHT_REL {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_REL, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let (data, _compression) = elf.section_data(&shdr)?;
            Ok(RelTable::new(elf.ehdr.endianness, elf.ehdr.class, data))
        })
    }

    /// Returns the data for the given section header, interpreted as a `Rela`
    /// relocation table. Fails if the section type is not `SHT_RELA`.
    pub fn section_data_as_relas(&self, section_header: &SectionHeader) -> ResultRelaTable {
        try_crubit!({
            let elf = self.elf_or_err()?;
            if section_header.sh_type != abi::SHT_RELA {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_RELA, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let (data, _compression) = elf.section_data(&shdr)?;
            Ok(RelaTable::new(elf.ehdr.endianness, elf.ehdr.class, data))
        })
    }

    /// Returns the data for the given section header, interpreted as a note table.
    /// Fails if the section type is not `SHT_NOTE`.
    pub fn section_data_as_notes(&self, section_header: &SectionHeader) -> ResultNoteTable {
        try_crubit!({
            let elf = self.elf_or_err()?;
            if section_header.sh_type != abi::SHT_NOTE {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_NOTE, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let (data, _compression) = elf.section_data(&shdr)?;
            Ok(NoteTable::new(
                elf.ehdr.endianness,
                elf.ehdr.class,
                section_header.sh_addralign as usize,
                data,
            ))
        })
    }

    /// Returns the symbol table (`.symtab`) and its associated string table, if present.
    pub fn symbol_table(&self) -> ResultOptionSymbolTableWithStrings {
        try_crubit!({
            let elf = self.elf_or_err()?;
            let shdrs = match elf.section_headers() {
                Some(shdrs) => shdrs,
                None => return Ok(None.into()),
            };

            let symtab_shdr = match shdrs.iter().find(|shdr| shdr.sh_type == abi::SHT_SYMTAB) {
                Some(shdr) => shdr,
                None => return Ok(None.into()),
            };

            let strtab_shdr = shdrs.get(symtab_shdr.sh_link as usize)?;

            let (symtab_data, _comp) = elf.section_data(&symtab_shdr)?;
            let (strtab_data, _comp) = elf.section_data(&strtab_shdr)?;

            let tp_symtab =
                elf::symbol::SymbolTable::new(elf.ehdr.endianness, elf.ehdr.class, symtab_data);

            Ok(Some(SymbolTableWithStrings {
                symbol_table: SymbolTable::from(tp_symtab),
                string_table: StringTable::new(strtab_data),
            })
            .into())
        })
    }

    /// Returns the dynamic section table (`.dynamic`), if present.
    pub fn dynamic(&self) -> ResultOptionDynTable {
        try_crubit!({ Ok(self.elf_or_err()?.dynamic()?.map(DynTable::from).into()) })
    }

    /// Returns the dynamic symbol table (`.dynsym`) and its associated string table,
    /// if present.
    pub fn dynamic_symbol_table(&self) -> ResultOptionSymbolTableWithStrings {
        try_crubit!({
            let elf = self.elf_or_err()?;
            let shdrs = match elf.section_headers() {
                Some(shdrs) => shdrs,
                None => return Ok(None.into()),
            };

            let symtab_shdr = match shdrs.iter().find(|shdr| shdr.sh_type == abi::SHT_DYNSYM) {
                Some(shdr) => shdr,
                None => return Ok(None.into()),
            };

            let strtab_shdr = shdrs.get(symtab_shdr.sh_link as usize)?;

            let (symtab_data, _comp) = elf.section_data(&symtab_shdr)?;
            let (strtab_data, _comp) = elf.section_data(&strtab_shdr)?;

            let tp_symtab =
                elf::symbol::SymbolTable::new(elf.ehdr.endianness, elf.ehdr.class, symtab_data);

            Ok(Some(SymbolTableWithStrings {
                symbol_table: SymbolTable::from(tp_symtab),
                string_table: StringTable::new(strtab_data),
            })
            .into())
        })
    }

    /// Returns the GNU symbol versioning tables (.gnu.version, .gnu.version_r,
    /// .gnu.version_d), if present.
    pub fn symbol_version_table(&self) -> ResultOptionSymbolVersionTable {
        try_crubit!({
            let tp_table_opt = self.elf_or_err()?.symbol_version_table()?;
            let st_opt = self.dynamic_symbol_table().inner?.inner;

            let result: Option<anyhow::Result<SymbolVersionTable>> =
                tp_table_opt.zip(st_opt).map(|(tp_table, st)| {
                    SymbolVersionTable::new_from_third_party(&tp_table, st.symbol_table.len())
                });

            Ok(result.transpose()?.into())
        })
    }
}
