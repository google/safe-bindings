#include "dump.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <ostream>
#include <string>

#include "elf_bridge.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"

namespace elf_bridge {

namespace {
const char* ClassString(ElfClass elf_class) {
  switch (elf_class) {
    case ElfClass::kElf32:
      return "ELF32";
    case ElfClass::kElf64:
      return "ELF64";
    default:
      return "Unknown";
  };
}

const char* EndianString(AnyEndian endian) {
  switch (endian) {
    case AnyEndian::kLittle:
      return "Little endian";
    case AnyEndian::kBig:
      return "Big endian";
    default:
      return "Unknown";
  }
}

void DumpGnuAbiTagNote(std::ostream& out, const Note& note) {
  absl::StatusOr<NoteGnuAbiTag> tag = note.GetAsGnuAbiTag();
  if (!tag.ok()) {
    out << "Error dumping GNU ABI tag note";
    return;
  }
  out << absl::StrFormat(
      "GNU ABI tag. OS: %d, major: %d, minor: %d, subminor: %d", tag->GetOs(),
      tag->GetMajor(), tag->GetMinor(), tag->GetSubminor());
}

void DumpGnuBuildIdNote(std::ostream& out, const Note& note) {
  absl::StatusOr<NoteGnuBuildId> build_id = note.GetAsGnuBuildId();
  if (!build_id.ok()) {
    out << "Error dumping GNU build ID note";
    return;
  }
  for (char c : build_id->GetBuildId()) {
    out << absl::StrFormat("%02x ", static_cast<uint8_t>(c));
  }
}

void DumpUnknownNote(std::ostream& out, const Note& note) {}

bool IsStringTag(DynamicTag tag) {
  return tag == DynamicTag::kNeeded || tag == DynamicTag::kSoname ||
         tag == DynamicTag::kRpath || tag == DynamicTag::kRunpath;
}

bool IsNullTag(DynamicTag tag) { return tag == DynamicTag::kNull; }

}  // namespace

void DumpFileHeader(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<FileHeader> header = elf_bytes.GetFileHeader();
  if (!header.ok()) {
    out << "Failed to get file header: " << header.status() << "\n";
    return;
  }
  absl::StatusOr<ElfMachine> machine = header->GetMachine();
  // NOTE(b/457862262): The Rust ELF library doesn't expose e_ident[EI_VERSION].
  // It only supports EV_CURRENT(1) and errors out if other value is found, so
  // we don't print the ELFVersion field that ELFIO prints.
  // NOTE(b/457862262): Add functions to prettyprint other fields like OS/ABI
  // that don't have constants exposed in elf_bridge.
  out << "ELF Header\n\n"
      << "  Class:      " << ClassString(header->GetClass()) << "\n"
      << "  Encoding:   " << EndianString(header->GetEndianness()) << "\n"
      << "  OS/ABI:     " << OsAbiToString(header->GetOsAbi()) << "\n"
      << "  ABI Version:" << static_cast<unsigned int>(header->GetAbiVersion())
      << "\n"
      << "  Type:       " << header->GetType() << "\n"
      << "  Machine:    "
      << (machine.ok() ? MachineToHumanString(*machine) : "error") << "\n"
      << "  Version:    " << header->GetVersion() << "\n"
      << "  Entry:      " << absl::StrFormat("%#x", header->GetEntry()) << "\n"
      << "  Flags:      " << absl::StrFormat("%#x", header->GetFlags()) << "\n"
      << "\n";
}

void DumpSectionHeaders(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<SectionHeaderTableWithStrings> sections_with_strings =
      elf_bytes.GetSectionHeadersWithStrTab();
  if (!sections_with_strings.ok()) {
    out << "Failed to get section headers: " << sections_with_strings.status()
        << "\n";
    return;
  }
  const std::optional<SectionHeaderTable>& sections =
      sections_with_strings->GetSectionHeaderTable();
  if (!sections.has_value()) {
    out << "Section header table not found.\n";
    return;
  }
  const std::optional<StringTable>& strings =
      sections_with_strings->GetStringTable();
  if (!strings.has_value()) {
    out << "Failed to dump section headers. String table not found.\n";
    return;
  }
  out << "Section Headers:\n";
  for (size_t i = 0; i < sections->size(); ++i) {
    absl::StatusOr<SectionHeader> section = sections->GetSectionHeader(i);
    if (!section.ok()) continue;
    out << "  [" << i << "] ";
    DumpSectionHeader(out, *section, *strings);
  }
  out << "\n";
}

void DumpSectionHeader(std::ostream& out, const SectionHeader& section,
                       const StringTable& string_table) {
  absl::StatusOr<std::string> name = string_table.Get(section.GetName());
  absl::StatusOr<SectionType> type = section.GetType();
  out << (name.ok() ? *name : absl::StrFormat("<error: %v>", name.status()))
      << ", type: "
      << (type.ok() ? SectionTypeToString(*type)
                    : absl::StrFormat("<error: %v>", type.status()))
      << ", addr: " << absl::StrFormat("%#x", section.GetAddress())
      << ", size: " << absl::StrFormat("%#x", section.GetSize())
      << ", entry_size: " << absl::StrFormat("%#x", section.GetEntrySize())
      << ", flags: " << absl::StrFormat("%#x", section.GetFlags())
      << ", link: " << absl::StrFormat("%#x", section.GetLink())
      << ", info: " << absl::StrFormat("%#x", section.GetInfo())
      << ", addr_align: " << absl::StrFormat("%#x", section.GetAddressAlign())
      << "\n";
}

void DumpSegmentHeaders(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<std::optional<SegmentTable>> segments =
      elf_bytes.GetSegments();
  if (!segments.ok()) {
    out << "Failed to get segment table: " << segments.status() << "\n";
    return;
  }
  if (!segments->has_value()) {
    out << "Segment table not found\n";
    return;
  }
  out << "Segment Headers:\n";
  for (size_t i = 0; i < (**segments).size(); ++i) {
    absl::StatusOr<ProgramHeader> program_header =
        (**segments).GetProgramHeader(i);
    if (!program_header.ok()) continue;
    out << "  [" << i << "] ";
    DumpProgramHeader(out, *program_header);
  }
  out << "\n";
}

void DumpProgramHeader(std::ostream& out, const ProgramHeader& program_header) {
  absl::StatusOr<ProgramHeaderType> type = program_header.GetType();
  out << "type: "
      << (type.ok() ? ProgramHeaderTypeToString(*type)
                    : absl::StrFormat("<error: %v>", type.status()))
      << ", p_offset: " << absl::StrFormat("%#x", program_header.GetOffset())
      << ", p_vaddr: "
      << absl::StrFormat("%#x", program_header.GetVirtualAddress())
      << ", p_paddr: "
      << absl::StrFormat("%#x", program_header.GetPhysicalAddress())
      << ", p_filesz: " << absl::StrFormat("%#x", program_header.GetFileSize())
      << ", p_memsz: " << absl::StrFormat("%#x", program_header.GetMemorySize())
      << ", p_flags: " << absl::StrFormat("%#x", program_header.GetFlags())
      << ", p_align: " << absl::StrFormat("%#x", program_header.GetAlignment())
      << "\n";
}

void DumpSymbolTables(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<std::optional<SymbolTableWithStrings>> symbols =
      elf_bytes.GetSymbolTable();
  // Don't early return on failure. We still want to try .dynsym.
  if (symbols.ok()) {
    if (symbols->has_value()) {
      out << "Symbol table (.symtab):\n";
      DumpSymbolTable(out, **symbols);
    }
  } else {
    out << "Failed to get symbol table: " << symbols.status();
  }
  absl::StatusOr<std::optional<SymbolTableWithStrings>> dynamic_symbols =
      elf_bytes.GetDynamicSymbolTable();
  // Don't early return on failure. We still want to try .dynsym.
  if (dynamic_symbols.ok()) {
    if (dynamic_symbols->has_value()) {
      out << "Dynamic symbol table (.dynsym):\n";
      DumpSymbolTable(out, **dynamic_symbols);
    }
  } else {
    out << "Failed to get symbol table: " << dynamic_symbols.status();
  }
}

void DumpSymbolTable(std::ostream& out, const SymbolTableWithStrings& table) {
  const SymbolTable& symbol_table = table.GetSymbolTable();
  const StringTable& string_table = table.GetStringTable();
  for (size_t i = 0; i < symbol_table.size(); ++i) {
    absl::StatusOr<Symbol> symbol = symbol_table.GetSymbol(i);
    if (!symbol.ok()) continue;
    absl::StatusOr<std::string> name = string_table.Get(symbol->GetName());
    absl::StatusOr<SymbolType> type = symbol->GetType();
    absl::StatusOr<SymbolBind> binding = symbol->GetBinding();
    absl::StatusOr<SymbolVisibility> visibility = symbol->GetVisibility();
    out << "  [" << i << "] "
        << (name.ok() ? *name : absl::StrFormat("<error: %v>", name.status()))
        << ", value: " << absl::StrFormat("%#x", symbol->GetValue())
        << ", size: " << absl::StrFormat("%#x", symbol->GetSize()) << ", type: "
        << (type.ok() ? SymbolTypeToString(*type)
                      : absl::StrFormat("<error: %v>", type.status()))
        << ", bind: "
        << (binding.ok() ? SymbolBindToString(*binding)
                         : absl::StrFormat("<error: %v>", binding.status()))
        << ", visibility: "
        << (visibility.ok()
                ? SymbolVisibilityToString(*visibility)
                : absl::StrFormat("<error: %v>", visibility.status()))
        << "\n";
  }
  out << "\n";
}

// Dumps all sections in the file with type SHT_NOTE.
void DumpNoteSections(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<SectionHeaderTableWithStrings> sections_with_strings =
      elf_bytes.GetSectionHeadersWithStrTab();
  if (!sections_with_strings.ok()) {
    out << "Failed to dump notes. Error getting section header table: "
        << sections_with_strings.status();
    return;
  }
  const std::optional<SectionHeaderTable>& sections =
      sections_with_strings->GetSectionHeaderTable();
  const std::optional<StringTable>& strings =
      sections_with_strings->GetStringTable();
  if (!sections.has_value() || !strings.has_value()) {
    return;
  }
  for (size_t i = 0; i < sections->size(); ++i) {
    absl::StatusOr<SectionHeader> section = sections->GetSectionHeader(i);
    if (!section.ok()) continue;
    absl::StatusOr<SectionType> type = section->GetType();
    if (!type.ok() || *type != SectionType::kNote) continue;
    absl::StatusOr<std::string> name = strings->Get(section->GetName());
    out << "Note section ("
        << (name.ok() ? *name : absl::StrFormat("<error: %v>", name.status()))
        << "):";
    absl::StatusOr<NoteTable> notes = elf_bytes.GetSectionDataAsNotes(*section);
    if (notes.ok()) {
      DumpNoteTable(out, *notes);
    } else {
      out << "Error getting notes: " << notes.status();
    }
  }
}

void DumpNoteTable(std::ostream& out, const NoteTable& table) {
  for (size_t i = 0; i < table.size(); ++i) {
    out << "[" << i << "] ";
    absl::StatusOr<Note> note = table.Get(i);
    if (!note.ok()) continue;
    switch (note->GetType()) {
      case NoteType::kGnuAbiTag:
        DumpGnuAbiTagNote(out, *note);
        break;
      case NoteType::kGnuBuildId:
        DumpGnuBuildIdNote(out, *note);
        break;
      case NoteType::kUnknown:
        DumpUnknownNote(out, *note);
        break;
      default:
        out << "Invalid note type value";
    }
    out << "\n";
  }
  out << "\n";
}

void DumpNoteSegments(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<std::optional<SegmentTable>> segments =
      elf_bytes.GetSegments();
  if (!segments.ok()) {
    out << "Failed to dump notes. Error getting segment table: "
        << segments.status();
    return;
  }
  if (!segments->has_value()) {
    return;
  }
  for (size_t i = 0; i < (**segments).size(); ++i) {
    absl::StatusOr<ProgramHeader> program_header =
        (**segments).GetProgramHeader(i);
    if (!program_header.ok()) continue;
    absl::StatusOr<ProgramHeaderType> type = program_header->GetType();
    if (!type.ok() || *type != ProgramHeaderType::kNote) continue;
    out << "Note segment [" << i << "]:";
    absl::StatusOr<NoteTable> notes =
        elf_bytes.GetSegmentDataAsNotes(*program_header);
    if (notes.ok()) {
      DumpNoteTable(out, *notes);
    } else {
      out << "Error getting notes: " << notes.status();
    }
  }
}

void DumpNotes(std::ostream& out, const ElfBytes& elf_bytes) {
  DumpNoteSections(out, elf_bytes);
  DumpNoteSegments(out, elf_bytes);
}

void DumpDynamicTags(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<std::optional<DynTable>> dynamic_table =
      elf_bytes.GetDynamic();
  if (!dynamic_table.ok()) {
    out << "Failed to get dynamic table: " << dynamic_table.status() << "\n";
    return;
  }
  if (!dynamic_table->has_value()) {
    return;
  }

  std::optional<StringTable> dynstr;
  // Try to find string table via the dynamic section's link field.
  absl::StatusOr<std::optional<SectionHeader>> dynamic_section_header =
      elf_bytes.GetSectionHeaderByName(".dynamic");
  if (dynamic_section_header.ok() && dynamic_section_header->has_value()) {
    Elf_Word link = (*dynamic_section_header)->GetLink();
    absl::StatusOr<std::optional<SectionHeaderTable>> sections =
        elf_bytes.GetSectionHeaders();
    if (sections.ok() && sections->has_value()) {
      absl::StatusOr<SectionHeader> strtab_header =
          (*sections)->GetSectionHeader(link);
      if (strtab_header.ok()) {
        absl::StatusOr<StringTable> string_table =
            elf_bytes.GetSectionDataAsStrTab(*strtab_header);
        if (string_table.ok()) {
          dynstr = *string_table;
        }
      }
    }
  }

  // Fallback: try to find ".dynstr" by name if the above failed.
  if (!dynstr.has_value()) {
    absl::StatusOr<std::optional<SectionHeader>> dynstr_header =
        elf_bytes.GetSectionHeaderByName(".dynstr");
    if (dynstr_header.ok() && dynstr_header->has_value()) {
      absl::StatusOr<StringTable> string_table =
          elf_bytes.GetSectionDataAsStrTab(**dynstr_header);
      if (string_table.ok()) {
        dynstr = *string_table;
      }
    }
  }

  if (!dynstr.has_value()) {
    out << "  # Warning: Could not resolve dynamic string table. "
           "String values will be dumped as raw offsets.\n";
  }

  out << "Dynamic section:\n";
  for (size_t i = 0; i < (**dynamic_table).size(); ++i) {
    absl::StatusOr<Dyn> dyn = (**dynamic_table).GetDyn(i);
    if (!dyn.ok()) continue;

    Elf_Xword value = dyn->GetValue();
    std::string tag_string = DynamicTagToString(dyn->GetTag());

    out << "  [" << i << "] tag: " << tag_string;
    if (IsStringTag(dyn->GetTag()) && dynstr.has_value()) {
      absl::StatusOr<std::string> str = dynstr->Get(value);
      out << ", value: "
          << (str.ok() ? *str : absl::StrFormat("<error: %v>", str.status()));
    } else {
      out << ", value: " << absl::StrFormat("%#x", value);
    }
    out << "\n";

    if (IsNullTag(dyn->GetTag())) {
      break;
    }
  }
  out << "\n";
}

void DumpSectionDatas(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<SectionHeaderTableWithStrings> sections_with_strings =
      elf_bytes.GetSectionHeadersWithStrTab();
  if (!sections_with_strings.ok()) {
    out << "Failed to get section headers: " << sections_with_strings.status()
        << "\n";
    return;
  }
  const std::optional<SectionHeaderTable>& sections =
      sections_with_strings->GetSectionHeaderTable();
  if (!sections.has_value()) {
    out << "Section header table not found.\n";
    return;
  }
  const std::optional<StringTable>& strings =
      sections_with_strings->GetStringTable();
  if (!strings.has_value()) {
    out << "Failed to dump section data. String table not found.\n";
    return;
  }
  out << "Section Data:\n";
  // Skipping the null section index 0, just like ELFIO does.
  for (size_t i = 1; i < sections->size(); ++i) {
    absl::StatusOr<SectionHeader> section = sections->GetSectionHeader(i);
    if (!section.ok()) continue;
    absl::StatusOr<SectionType> type = section->GetType();
    if (type.ok() && *type == SectionType::kNoBits) {
      continue;
    }
    DumpSectionData(out, *section, elf_bytes, *strings);
  }
  out << "\n";
}

void DumpSectionData(std::ostream& out, const SectionHeader& section,
                     const ElfBytes& elf_bytes,
                     const StringTable& string_table) {
  absl::StatusOr<std::string> name = string_table.Get(section.GetName());
  out << (name.ok() ? *name : absl::StrFormat("<error: %v>", name.status()))
      << "\n";
  absl::StatusOr<SectionData> data = elf_bytes.GetSectionData(section);
  if (!data.ok()) {
    out << "Failed to get section data: " << data.status() << "\n";
    return;
  }
  absl::string_view pdata = data->GetData();
  if (!pdata.empty()) {
    // The maximum number of entries to print. We limit it to 64 entries
    // to match ELFIO's behavior (ELFIO::dump::MAX_DATA_ENTRIES).
    constexpr size_t kMaxEntries = 64;
    size_t size_to_dump = std::min(pdata.size(), kMaxEntries);
    for (size_t i = 0; i < size_to_dump; ++i) {
      if (i % 16 == 0) {
        out << "[" << absl::StrFormat("0x%08x", i) << "]";
      }
      out << " " << absl::StrFormat("0x%02x", static_cast<uint8_t>(pdata[i]));
      if (i % 16 == 15) {
        out << "\n";
      }
    }
    out << "\n";
  }
}

void DumpSegmentDatas(std::ostream& out, const ElfBytes& elf_bytes) {
  absl::StatusOr<std::optional<SegmentTable>> segments =
      elf_bytes.GetSegments();
  if (!segments.ok()) {
    out << "Failed to get segment table: " << segments.status() << "\n";
    return;
  }
  if (!segments->has_value()) {
    out << "Segment table not found\n";
    return;
  }
  out << "Segment Data:\n";
  for (size_t i = 0; i < (**segments).size(); ++i) {
    absl::StatusOr<ProgramHeader> program_header =
        (**segments).GetProgramHeader(i);
    if (!program_header.ok()) continue;
    DumpSegmentData(out, i, *program_header, elf_bytes);
  }
  out << "\n";
}

void DumpSegmentData(std::ostream& out, size_t segment_index,
                     const ProgramHeader& program_header,
                     const ElfBytes& elf_bytes) {
  out << "Segment # " << segment_index << "\n";
  absl::StatusOr<std::string> data = elf_bytes.GetSegmentData(program_header);
  if (!data.ok()) {
    out << "Failed to get segment data: " << data.status() << "\n";
    return;
  }
  if (!data->empty()) {
    // The maximum number of entries to print. We limit it to 64 entries
    // to match ELFIO's behavior (ELFIO::dump::MAX_DATA_ENTRIES).
    constexpr size_t kMaxEntries = 64;
    size_t size_to_dump = std::min(data->size(), kMaxEntries);
    for (size_t i = 0; i < size_to_dump; ++i) {
      if (i % 16 == 0) {
        out << "[" << absl::StrFormat("0x%08x", i) << "]";
      }
      out << " " << absl::StrFormat("0x%02x", static_cast<uint8_t>((*data)[i]));
      if (i % 16 == 15) {
        out << "\n";
      }
    }
    out << "\n";
  }
}

}  // namespace elf_bridge
