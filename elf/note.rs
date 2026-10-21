//! Owned ELF note sections.
use crate::{make_option_type, make_result_type, try_crubit};
use anyhow::anyhow;
use crubit_annotate::cpp_enum;
use elf::endian::EndianParse;
use elf::file::Class;
use elf::note::{
    Note as ThirdPartyNote, NoteAny as ThirdPartyNoteAny, NoteGnuAbiTag as ThirdPartyNoteGnuAbiTag,
    NoteGnuBuildId as ThirdPartyNoteGnuBuildId, NoteIterator as ThirdPartyNoteIterator,
};

/// The type of an ELF note.
#[cpp_enum(kind = "enum class")]
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoteType(u32);

#[allow(non_upper_case_globals)]
impl NoteType {
    /// A GNU ABI tag note (.note.ABI-tag).
    pub const kGnuAbiTag: NoteType = NoteType(0);
    /// A GNU build ID note (.note.gnu.build-id).
    pub const kGnuBuildId: NoteType = NoteType(1);
    /// An unknown or generic note.
    pub const kUnknown: NoteType = NoteType(2);
}

/// An owned ELF GNU ABI tag note, analogous to `elf::note::NoteGnuAbiTag`
/// but fully owned for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct NoteGnuAbiTag {
    /// The OS identifier (e.g., 0 for Linux).
    pub os: u32,
    /// The major version of the ABI.
    pub major: u32,
    /// The minor version of the ABI.
    pub minor: u32,
    /// The subminor version of the ABI.
    pub subminor: u32,
}

impl From<ThirdPartyNoteGnuAbiTag> for NoteGnuAbiTag {
    fn from(note: ThirdPartyNoteGnuAbiTag) -> Self {
        Self { os: note.os, major: note.major, minor: note.minor, subminor: note.subminor }
    }
}
make_option_type!(NoteGnuAbiTag);
make_result_type!(NoteGnuAbiTag);
make_result_type!(OptionNoteGnuAbiTag);

/// An owned ELF GNU build ID note, analogous to `elf::note::NoteGnuBuildId`
/// but holding owned data (`Vec<u8>`) rather than a slice for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct NoteGnuBuildId {
    /// The build ID bytes.
    pub build_id: Vec<u8>,
}

impl<'a> From<ThirdPartyNoteGnuBuildId<'a>> for NoteGnuBuildId {
    fn from(note: ThirdPartyNoteGnuBuildId<'a>) -> Self {
        Self { build_id: note.0.into() }
    }
}

make_option_type!(NoteGnuBuildId);
make_result_type!(NoteGnuBuildId);
make_result_type!(OptionNoteGnuBuildId);

/// An owned unknown ELF note, analogous to `elf::note::NoteAny` but holding
/// owned data (`String`, `Vec<u8>`) rather than slices for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C)]
pub struct NoteAny {
    /// The note type identifier.
    pub n_type: u64,
    /// The name field of the note.
    pub name: Vec<u8>,
    /// The descriptor field of the note.
    pub desc: Vec<u8>,
}

impl<'a> From<ThirdPartyNoteAny<'a>> for NoteAny {
    fn from(note: ThirdPartyNoteAny<'a>) -> Self {
        Self { n_type: note.n_type, name: note.name.into(), desc: note.desc.into() }
    }
}
make_option_type!(NoteAny);
make_result_type!(NoteAny);
make_result_type!(OptionNoteAny);

/// An enum representing different types of ELF notes, holding owned data
/// for easier FFI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// A GNU ABI tag note.
    GnuAbiTag(NoteGnuAbiTag),
    /// A GNU build ID note.
    GnuBuildId(NoteGnuBuildId),
    /// An unknown or generic note.
    Unknown(NoteAny),
}

impl Note {
    /// Returns the type of this note.
    pub fn get_type(&self) -> NoteType {
        match self {
            Note::GnuAbiTag(_) => NoteType::kGnuAbiTag,
            Note::GnuBuildId(_) => NoteType::kGnuBuildId,
            Note::Unknown(_) => NoteType::kUnknown,
        }
    }

    /// If this note is a `GnuAbiTag`, returns it, otherwise returns an error.
    pub fn get_as_gnu_abi_tag(&self) -> ResultNoteGnuAbiTag {
        try_crubit!({
            match self {
                Note::GnuAbiTag(tag) => Ok(tag.clone()),
                _ => Err(anyhow!("Note is not a GnuAbiTag")),
            }
        })
    }

    /// If this note is a `GnuBuildId`, returns it, otherwise returns an error.
    pub fn get_as_gnu_build_id(&self) -> ResultNoteGnuBuildId {
        try_crubit!({
            match self {
                Note::GnuBuildId(id) => Ok(id.clone()),
                _ => Err(anyhow!("Note is not a GnuBuildId")),
            }
        })
    }

    /// If this note is an `Unknown` note, returns it, otherwise returns an error.
    pub fn get_as_unknown(&self) -> ResultNoteAny {
        try_crubit!({
            match self {
                Note::Unknown(any) => Ok(any.clone()),
                _ => Err(anyhow!("Note is not an Unknown note")),
            }
        })
    }
}

impl<'a> From<ThirdPartyNote<'a>> for Note {
    fn from(note: ThirdPartyNote<'a>) -> Self {
        match note {
            ThirdPartyNote::GnuAbiTag(tag) => Note::GnuAbiTag(tag.into()),
            ThirdPartyNote::GnuBuildId(id) => Note::GnuBuildId(id.into()),
            ThirdPartyNote::Unknown(any) => Note::Unknown(any.into()),
        }
    }
}
make_option_type!(Note);
make_result_type!(Note);
make_result_type!(OptionNote);

/// An owned table of ELF notes, analogous to iterating over
/// `elf::note::NoteIterator` but holding owned `Note` enums in a `Vec` for
/// easier FFI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteTable {
    pub entries: Vec<Note>,
}

impl NoteTable {
    /// Creates a new `NoteTable` by parsing note section data.
    pub fn new<E: EndianParse>(endian: E, class: Class, align: usize, data: &[u8]) -> Self {
        let entries =
            ThirdPartyNoteIterator::new(endian, class, align, data).map(|n| n.into()).collect();
        Self { entries }
    }

    /// Returns the number of notes in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the table contains no note entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retrieves the note at the given index.
    pub fn get(&self, index: usize) -> ResultNote {
        try_crubit!({
            self.entries
                .get(index)
                .cloned()
                .ok_or(anyhow!("NoteTable: index {index} out of bounds"))
        })
    }
}
make_option_type!(NoteTable);
make_result_type!(NoteTable);
make_result_type!(OptionNoteTable);
