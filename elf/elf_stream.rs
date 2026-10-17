//! This module provides an FFI-friendly streaming ELF parser wrapper around
//! the `elf` crate.
//!
//! The `ElfStream` struct opens an ELF file from a given path and prepares it
//! for streaming parsing. Unlike `ElfBytes`, which requires the entire ELF
//! file to be loaded into memory, `ElfStream` reads from the file on disk,
//! making it suitable for very large ELF files or situations where only
//! specific sections or segments need to be accessed without consuming large
//! amounts of memory.
//!
//! ## Design
//!
//! This wrapper holds an `elf::ElfStream` instance, which wraps a `File`
//! handle. Methods on `ElfStream` will read only the necessary parts of the
//! file to satisfy parsing requests, using `Read` and `Seek` on the
//! underlying file. This is FFI-safe because `ElfStream` owns the file handle
//! and the parser state. Data returned to C++ is owned, similar to `ElfBytes`.

use crate::dynamic::*;
use crate::file::{FileHeader, ResultFileHeader};
use crate::gnu_symver::{ResultOptionSymbolVersionTable, SymbolVersionTable};
use crate::make_result_type;
use crate::note::*;
use crate::relocation::*;
use crate::section::{
    ResultOptionSectionHeader, ResultSectionHeaderTable, SectionHeader, SectionHeaderTable,
};
use crate::segment::{ProgramHeader, ResultSegmentTable, SegmentTable};
use crate::string_table::{ResultStringTable, StringTable};
use crate::symbol::SymbolTable;
use crate::try_crubit;
use crate::types::{
    ResultOptionSymbolTableWithStrings, ResultSectionData, ResultSectionHeaderTableWithStrings,
    SectionData, SectionHeaderTableWithStrings, SymbolTableWithStrings,
};
use anyhow::anyhow;
use elf::abi;
use elf::endian::AnyEndian;
use elf::ElfStream as ThirdPartyElfStream;
use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::ffi::OsStrExt;

/// An ELF file parser that reads from a file stream.
///
/// This struct holds an `elf::ElfStream` which wraps an open `File` handle,
/// allowing for parsing of ELF files by reading and seeking within the file,
/// rather than loading the entire file into memory.
#[derive(Debug, Default)]
pub struct ElfStream {
    // Holds the file handle via RAII.
    inner_stream: Option<ThirdPartyElfStream<AnyEndian, File>>,
}

make_result_type!(ElfStream);

impl ElfStream {
    /// Returns a reference to the underlying `ThirdPartyElfStream` or an error
    /// if the struct was not initialized via `open_stream()`.
    fn inner_stream_or_err(&self) -> anyhow::Result<&ThirdPartyElfStream<AnyEndian, File>> {
        self.inner_stream.as_ref().ok_or(anyhow!("ElfStream is not initialized"))
    }

    /// Returns a mutable reference to the underlying `ThirdPartyElfStream` or an error
    /// if the struct was not initialized via `open_stream()`.
    fn inner_stream_mut_or_err(
        &mut self,
    ) -> anyhow::Result<&mut ThirdPartyElfStream<AnyEndian, File>> {
        self.inner_stream.as_mut().ok_or(anyhow!("ElfStream is not initialized"))
    }

    /// Opens the ELF file at `file_path` for streaming parsing.
    ///
    /// This function opens the file and initializes an `ElfStream` parser.
    /// It reads file metadata to parse the main ELF header but does not read
    /// section or segment data until requested by other methods.
    ///
    /// Returns an error if the file cannot be opened, read, or is not a
    /// valid ELF file.
    pub fn open_stream(file_path: &[u8]) -> ResultElfStream {
        try_crubit!({
            let path = OsStr::from_bytes(file_path);
            let file = File::open(path)?;
            let inner_stream = ThirdPartyElfStream::open_stream(file)?;

            Ok(ElfStream { inner_stream: Some(inner_stream) })
        })
    }

    /// Returns the main ELF file header.
    ///
    /// Returns an error if the stream is not initialized.
    pub fn file_header(&self) -> ResultFileHeader {
        try_crubit!({
            let ehdr = self.inner_stream_or_err()?.ehdr;
            Ok(FileHeader::from(ehdr))
        })
    }

    /// Returns the ELF segment table (program header table) that was parsed
    /// when this stream was opened.
    ///
    /// If the ELF file does not contain a segment table, an empty table is
    /// returned.
    ///
    /// Returns an error if the stream is not initialized.
    pub fn segments(&self) -> ResultSegmentTable {
        try_crubit!({
            let phdrs = self.inner_stream_or_err()?.segments();
            Ok(SegmentTable::from(phdrs))
        })
    }

    /// Returns the ELF section header table that was parsed when this stream
    /// was opened.
    ///
    /// If the ELF file does not contain a section header table, an empty table
    /// is returned.
    ///
    /// Returns an error if the stream is not initialized.
    pub fn section_headers(&self) -> ResultSectionHeaderTable {
        try_crubit!({
            let shdrs = self.inner_stream_or_err()?.section_headers();
            Ok(SectionHeaderTable::from(shdrs))
        })
    }

    /// Returns the ELF section header table along with the string table
    /// containing section names.
    ///
    /// This method provides access to section headers and fetches the
    /// string table containing section names, as indicated by the
    /// `e_shstrndx` field in the ELF header.
    ///
    /// If the ELF file contains no section headers, both table and string
    /// table in `SectionHeaderTableWithStrings` will be `None`. If section
    /// headers are present but `e_shstrndx` is `SHN_UNDEF`, indicating no
    /// section name string table exists, `string_table` will be `None`.
    ///
    /// This operation may read from the underlying file to load the string
    /// table section data.
    ///
    /// Returns an error if the stream is not initialized, if `e_shstrndx`
    /// is invalid, or if reading the string table section data fails.
    pub fn section_headers_with_strtab(&mut self) -> ResultSectionHeaderTableWithStrings {
        try_crubit!({
            let stream = self.inner_stream_mut_or_err()?;

            if stream.section_headers().is_empty() {
                return Ok(SectionHeaderTableWithStrings {
                    section_header_table: None.into(),
                    string_table: None.into(),
                });
            }

            if stream.ehdr.e_shstrndx == abi::SHN_UNDEF {
                return Ok(SectionHeaderTableWithStrings {
                    section_header_table: Some(SectionHeaderTable::from(stream.section_headers()))
                        .into(),
                    string_table: None.into(),
                });
            }

            let strtab_shdr = {
                let shdrs = stream.section_headers();
                let mut shstrndx = stream.ehdr.e_shstrndx as usize;
                if stream.ehdr.e_shstrndx == abi::SHN_XINDEX {
                    shstrndx = shdrs[0].sh_link as usize;
                }
                *shdrs.get(shstrndx).ok_or_else(|| anyhow!("Invalid shstrndx"))?
            };

            let (strtab_slice, _compression) = stream.section_data(&strtab_shdr)?;
            let strtab_data = strtab_slice.to_vec();

            Ok(SectionHeaderTableWithStrings {
                section_header_table: Some(SectionHeaderTable::from(stream.section_headers()))
                    .into(),
                string_table: Some(StringTable::new(&strtab_data)).into(),
            })
        })
    }

    /// Returns the section header with the given `name`, if one exists.
    ///
    /// This method searches for a section by name and returns its header.
    /// It may read from the underlying file to access section headers and
    /// load the section name string table if they haven't been read yet.
    ///
    /// Returns `Ok(None)` if no section with the specified name is found, or if
    /// the ELF file has no section header table or section name string table.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn section_header_by_name(&mut self, name: &[u8]) -> ResultOptionSectionHeader {
        try_crubit!({
            let name_str = std::str::from_utf8(name).map_err(|_| {
                anyhow!("Section name {:?} is not valid UTF-8", String::from_utf8_lossy(name))
            })?;
            Ok(self
                .inner_stream_mut_or_err()?
                .section_header_by_name(name_str)?
                .map(|shdr| (*shdr).into())
                .into())
        })
    }

    /// Returns data for the section represented by `section_header`.
    ///
    /// This method reads section data from the underlying file stream.
    /// If the section is compressed, compression information is also returned.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn section_data(&mut self, section_header: &SectionHeader) -> ResultSectionData {
        try_crubit!({
            let shdr = section_header.as_third_party();
            let (data, compression) = self.inner_stream_mut_or_err()?.section_data(&shdr)?;
            Ok(SectionData {
                data: data.to_vec(),
                compression: compression.map(|ch| ch.into()).into(),
            })
        })
    }

    /// Returns the data for the given section header, interpreted as a `StringTable`.
    ///
    /// This method reads section data from the underlying file stream and parses
    /// it as an ELF string table. It fails if `section_header.sh_type` is not
    /// `SHT_STRTAB`. This is useful for sections like `.shstrtab` or `.strtab`.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing string table data fails.
    pub fn section_data_as_strtab(&mut self, section_header: &SectionHeader) -> ResultStringTable {
        try_crubit!({
            let inner_stream = self.inner_stream_mut_or_err()?;
            if section_header.sh_type != abi::SHT_STRTAB {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_STRTAB, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let (data, _compression) = inner_stream.section_data(&shdr)?;
            Ok(StringTable::new(data))
        })
    }

    /// Returns the symbol table (`.symtab`) and its associated string table, if present.
    ///
    /// This method reads section data from the underlying file stream to find
    /// and parse the SHT_SYMTAB section and its linked string table.
    ///
    /// Returns `Ok(None)` if no section of type `SHT_SYMTAB` is found.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn symbol_table(&mut self) -> ResultOptionSymbolTableWithStrings {
        try_crubit!({
            let stream = self.inner_stream_mut_or_err()?;
            let shdrs = stream.section_headers();
            if shdrs.is_empty() {
                return Ok(None.into());
            }

            let symtab_shdr = match shdrs.iter().find(|shdr| shdr.sh_type == abi::SHT_SYMTAB) {
                Some(shdr) => *shdr,
                None => return Ok(None.into()),
            };

            let strtab_shdr = *shdrs
                .get(symtab_shdr.sh_link as usize)
                .ok_or_else(|| anyhow!("Invalid sh_link in SHT_SYMTAB section header"))?;

            let (symtab_data_slice, _) = stream.section_data(&symtab_shdr)?;
            let symtab_data = symtab_data_slice.to_vec();
            let (strtab_data_slice, _) = stream.section_data(&strtab_shdr)?;
            let strtab_data = strtab_data_slice.to_vec();

            let tp_symtab = elf::symbol::SymbolTable::new(
                stream.ehdr.endianness,
                stream.ehdr.class,
                &symtab_data,
            );

            Ok(Some(SymbolTableWithStrings {
                symbol_table: SymbolTable::from(tp_symtab),
                string_table: StringTable::new(&strtab_data),
            })
            .into())
        })
    }

    /// Returns the dynamic symbol table (`.dynsym`) and its associated string table,
    /// if present.
    ///
    /// This method reads section data from the underlying file stream to find
    /// and parse the SHT_DYNSYM section and its linked string table.
    ///
    /// Returns `Ok(None)` if no section of type `SHT_DYNSYM` is found.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn dynamic_symbol_table(&mut self) -> ResultOptionSymbolTableWithStrings {
        try_crubit!({
            let stream = self.inner_stream_mut_or_err()?;
            let shdrs = stream.section_headers();
            if shdrs.is_empty() {
                return Ok(None.into());
            }

            let symtab_shdr = match shdrs.iter().find(|shdr| shdr.sh_type == abi::SHT_DYNSYM) {
                Some(shdr) => *shdr,
                None => return Ok(None.into()),
            };

            let strtab_shdr = *shdrs
                .get(symtab_shdr.sh_link as usize)
                .ok_or_else(|| anyhow!("Invalid sh_link in SHT_DYNSYM section header"))?;

            let (symtab_data_slice, _) = stream.section_data(&symtab_shdr)?;
            let symtab_data = symtab_data_slice.to_vec();
            let (strtab_data_slice, _) = stream.section_data(&strtab_shdr)?;
            let strtab_data = strtab_data_slice.to_vec();

            let tp_symtab = elf::symbol::SymbolTable::new(
                stream.ehdr.endianness,
                stream.ehdr.class,
                &symtab_data,
            );

            Ok(Some(SymbolTableWithStrings {
                symbol_table: SymbolTable::from(tp_symtab),
                string_table: StringTable::new(&strtab_data),
            })
            .into())
        })
    }

    /// Returns the data for the given section header, interpreted as a `RelTable`.
    ///
    /// This method reads section data from the underlying file stream and parses
    /// it as a table of ELF relocation entries without explicit addends. It
    /// fails if `section_header.sh_type` is not `SHT_REL`.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing relocation data fails.
    pub fn section_data_as_rels(&mut self, section_header: &SectionHeader) -> ResultRelTable {
        try_crubit!({
            let stream = self.inner_stream_mut_or_err()?;
            if section_header.sh_type != abi::SHT_REL {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_REL, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let endianness = stream.ehdr.endianness;
            let class = stream.ehdr.class;
            let (data, _compression) = stream.section_data(&shdr)?;
            Ok(RelTable::new(endianness, class, data))
        })
    }

    /// Returns the data for the given section header, interpreted as a `RelaTable`.
    ///
    /// This method reads section data from the underlying file stream and parses
    /// it as a table of ELF relocation entries with explicit addends. It fails
    /// if `section_header.sh_type` is not `SHT_RELA`.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing relocation data fails.
    pub fn section_data_as_relas(&mut self, section_header: &SectionHeader) -> ResultRelaTable {
        try_crubit!({
            let stream = self.inner_stream_mut_or_err()?;
            if section_header.sh_type != abi::SHT_RELA {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_RELA, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let endianness = stream.ehdr.endianness;
            let class = stream.ehdr.class;
            let (data, _compression) = stream.section_data(&shdr)?;
            Ok(RelaTable::new(endianness, class, data))
        })
    }

    /// Returns the data for the given section header, interpreted as a `NoteTable`.
    ///
    /// This method reads section data from the underlying file stream and parses
    /// it as a table of ELF notes. It fails if `section_header.sh_type` is not
    /// `SHT_NOTE`.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing note data fails.
    pub fn section_data_as_notes(&mut self, section_header: &SectionHeader) -> ResultNoteTable {
        try_crubit!({
            let elf_stream = self.inner_stream_mut_or_err()?;
            if section_header.sh_type != abi::SHT_NOTE {
                return Err(anyhow!(
                    "Unexpected section type: expected SHT_NOTE, got {}",
                    section_header.sh_type
                ));
            }
            let shdr = section_header.as_third_party();
            let notes = elf_stream.section_data_as_notes(&shdr)?;
            Ok(NoteTable { entries: notes.map(|n| n.into()).collect() })
        })
    }

    /// Returns the data for the given program header, interpreted as a `NoteTable`.
    ///
    /// This method reads segment data from the underlying file stream and parses
    /// it as a table of ELF notes. It fails if `program_header.p_type` is not
    /// `PT_NOTE`.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing note data fails.
    pub fn segment_data_as_notes(&mut self, program_header: &ProgramHeader) -> ResultNoteTable {
        try_crubit!({
            let elf_stream = self.inner_stream_mut_or_err()?;
            let phdr = program_header.as_third_party();
            if phdr.p_type != abi::PT_NOTE {
                return Err(anyhow!(
                    "Unexpected segment type: expected PT_NOTE, got {}",
                    phdr.p_type
                ));
            }
            let notes = elf_stream.segment_data_as_notes(&phdr)?;
            Ok(NoteTable { entries: notes.map(|n| n.into()).collect() })
        })
    }

    /// Returns the dynamic section table (`.dynamic`), if present.
    ///
    /// This method reads section data from the underlying file stream to find
    /// and parse the `SHT_DYNAMIC` section.
    ///
    /// Returns `Ok(None)` if no section of type `SHT_DYNAMIC` is found.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn dynamic(&mut self) -> ResultOptionDynTable {
        try_crubit!({ Ok(self.inner_stream_mut_or_err()?.dynamic()?.map(DynTable::from).into()) })
    }

    /// Returns the GNU symbol versioning tables (.gnu.version, .gnu.version_r,
    /// .gnu.version_d), if present.
    ///
    /// This method reads section data from the underlying file stream to find
    /// and parse sections related to GNU symbol versioning.
    ///
    /// Returns `Ok(None)` if symbol versioning sections are not found.
    ///
    /// Returns an error if the stream is not initialized, or if reading or
    /// parsing section data fails.
    pub fn symbol_version_table(&mut self) -> ResultOptionSymbolVersionTable {
        try_crubit!({
            let st_opt = self.dynamic_symbol_table().inner?.inner;
            let tp_table_opt = self.inner_stream_mut_or_err()?.symbol_version_table()?;

            let result: Option<anyhow::Result<SymbolVersionTable>> =
                tp_table_opt.zip(st_opt).map(|(tp_table, st)| {
                    SymbolVersionTable::new_from_third_party(&tp_table, st.symbol_table.len())
                });

            Ok(result.transpose()?.into())
        })
    }
}
