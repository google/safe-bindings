#ifndef SAFE_BINDINGS_ELF_DUMP_H_
#define SAFE_BINDINGS_ELF_DUMP_H_

// This file defines an API to write to an ostream a human-readable textual dump
// of ELF file header data, similar to third_party/elfio/elfio/elfio_dump.hpp
// but not necessarily with the same formatting.

#include <ostream>

#include "elf_bridge.h"

namespace elf_bridge {

// Dumps the file header to `out`.
void DumpFileHeader(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps section headers to `out`.
void DumpSectionHeaders(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps a single section header to `out`. It takes a StringTable in order to
// display the section name.
void DumpSectionHeader(std::ostream& out, const SectionHeader& section,
                       const StringTable& string_table);

// Dumps segment headers to `out`.
void DumpSegmentHeaders(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps a program header to `out`.
void DumpProgramHeader(std::ostream& out, const ProgramHeader& program_header);

// Dumps symbol tables to `out`.
void DumpSymbolTables(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps a given SymbolTableWithStrings to `out`.
void DumpSymbolTable(std::ostream& out, const SymbolTableWithStrings& table);

// Dumps all notes in the binary (sections and segments) to `out`.
void DumpNotes(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps all sections in the file with type SHT_NOTE to `out`.
void DumpNoteSections(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps all segments in the file with type PT_NOTE to `out`.
void DumpNoteSegments(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps a NoteTable to `out`.
void DumpNoteTable(std::ostream& out, const NoteTable& table);

// Dumps dynamic section tags to `out`.
void DumpDynamicTags(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps all section data to `out`.
void DumpSectionDatas(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps data of a single section to `out`.
void DumpSectionData(std::ostream& out, const SectionHeader& section,
                     const ElfBytes& elf_bytes,
                     const StringTable& string_table);

// Dumps all segment data to `out`.
void DumpSegmentDatas(std::ostream& out, const ElfBytes& elf_bytes);

// Dumps data of a single segment to `out`.
void DumpSegmentData(std::ostream& out, size_t segment_index,
                     const ProgramHeader& program_header,
                     const ElfBytes& elf_bytes);

}  // namespace elf_bridge

#endif  // SAFE_BINDINGS_ELF_DUMP_H_
