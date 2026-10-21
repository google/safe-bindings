//! Owned ELF file header.
use crate::constants::AnyEndian;
use crate::constants::ElfClass;
use crate::constants::ElfMachine;
use crate::constants::ResultElfMachine;
use crate::{make_option_type, make_result_type};
use elf::endian::AnyEndian as ThirdPartyAnyEndian;

/// An owned ELF file header, analogous to `elf::file::FileHeader` but fully
/// owned for easier FFI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FileHeader {
    /// The ELF class (32-bit or 64-bit).
    pub class: ElfClass,
    /// The endianness of the file data.
    pub endianness: AnyEndian,
    /// The ELF specification version.
    pub version: u32,
    /// The OS ABI identification.
    pub osabi: u8,
    /// The ABI version.
    pub abiversion: u8,
    /// The object file type (e.g., executable, shared object).
    pub e_type: u16,
    /// The required architecture.
    pub e_machine_raw: u16,
    /// The virtual address of the entry point of the process.
    pub e_entry: u64,
    /// The offset of the program header table in the file.
    pub e_phoff: u64,
    /// The offset of the section header table in the file.
    pub e_shoff: u64,
    /// Processor-specific flags.
    pub e_flags: u32,
    /// The size of this ELF header in bytes.
    pub e_ehsize: u16,
    /// The size of a program header table entry.
    pub e_phentsize: u16,
    /// The number of entries in the program header table.
    pub e_phnum: u16,
    /// The size of a section header table entry.
    pub e_shentsize: u16,
    /// The number of entries in the section header table.
    pub e_shnum: u16,
    /// The section header table index of the section name string table.
    pub e_shstrndx: u16,
}

impl FileHeader {
    pub fn machine(&self) -> ResultElfMachine {
        let machine = ElfMachine::try_from(self.e_machine_raw);
        machine.into()
    }
}

impl Default for FileHeader {
    fn default() -> Self {
        Self {
            class: ElfClass::kElf32,
            endianness: AnyEndian::kLittle,
            version: 1,
            osabi: 0,
            abiversion: 0,
            e_type: 0,
            e_machine_raw: 0,
            e_entry: 0,
            e_phoff: 0,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: 0,
            e_phentsize: 0,
            e_phnum: 0,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        }
    }
}

make_option_type!(FileHeader, OptionFileHeader);
make_result_type!(FileHeader, ResultFileHeader);
make_result_type!(OptionFileHeader);

impl From<elf::file::FileHeader<ThirdPartyAnyEndian>> for FileHeader {
    fn from(header: elf::file::FileHeader<ThirdPartyAnyEndian>) -> Self {
        Self {
            class: header.class.into(),
            endianness: header.endianness.into(),
            version: header.version,
            osabi: header.osabi,
            abiversion: header.abiversion,
            e_type: header.e_type,
            e_machine_raw: header.e_machine,
            e_entry: header.e_entry,
            e_phoff: header.e_phoff,
            e_shoff: header.e_shoff,
            e_flags: header.e_flags,
            e_ehsize: header.e_ehsize,
            e_phentsize: header.e_phentsize,
            e_phnum: header.e_phnum,
            e_shentsize: header.e_shentsize,
            e_shnum: header.e_shnum,
            e_shstrndx: header.e_shstrndx,
        }
    }
}
