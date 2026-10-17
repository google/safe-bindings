// Showcase of the elf_bridge C++ API, a safe wrapper around the Rust `elf`
// crate.
//
// Usage: elf_example [path-to-elf-file]
//
// If no path is given, the example inspects its own executable.

#include <cstddef>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <optional>
#include <string>

#include "dump.h"
#include "elf_bridge.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"

namespace {

std::string ReadFile(const std::string& path) {
  std::ifstream file(path, std::ios::binary);
  return std::string((std::istreambuf_iterator<char>(file)),
                     std::istreambuf_iterator<char>());
}

const char* ClassName(elf_bridge::ElfClass elf_class) {
  return elf_class == elf_bridge::ElfClass::kElf64 ? "ELF64" : "ELF32";
}

}  // namespace

int main(int argc, char** argv) {
  const std::string path = argc > 1 ? argv[1] : "/proc/self/exe";
  std::cout << "=== ELF Library Showcase ===\n";
  std::cout << "Inspecting: " << path << "\n\n";

  // 1. Parsing an in-memory ELF file with ElfBytes.
  std::cout << "--- 1. Parsing and the File Header ---\n";
  const std::string data = ReadFile(path);
  absl::StatusOr<elf_bridge::ElfBytes> elf = elf_bridge::ElfBytes::Parse(data);
  if (!elf.ok()) {
    std::cerr << "Failed to parse ELF file: " << elf.status() << "\n";
    return 1;
  }
  absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
  if (!header.ok()) {
    std::cerr << "Failed to read file header: " << header.status() << "\n";
    return 1;
  }
  absl::StatusOr<elf_bridge::ElfMachine> machine = header->GetMachine();
  std::cout << "Class:   " << ClassName(header->GetClass()) << "\n"
            << "Machine: "
            << (machine.ok() ? elf_bridge::MachineToHumanString(*machine)
                             : "unknown")
            << "\n"
            << "OS/ABI:  " << elf_bridge::OsAbiToString(header->GetOsAbi())
            << "\n"
            << "Entry:   " << absl::StrFormat("%#x", header->GetEntry())
            << "\n\n";

  // 2. Iterating section headers and resolving their names.
  std::cout << "--- 2. Sections ---\n";
  absl::StatusOr<elf_bridge::SectionHeaderTableWithStrings> sections =
      elf->GetSectionHeadersWithStrTab();
  if (sections.ok() && sections->GetSectionHeaderTable().has_value() &&
      sections->GetStringTable().has_value()) {
    const elf_bridge::SectionHeaderTable& table =
        *sections->GetSectionHeaderTable();
    const elf_bridge::StringTable& names = *sections->GetStringTable();
    for (size_t i = 0; i < table.size(); ++i) {
      absl::StatusOr<elf_bridge::SectionHeader> section =
          table.GetSectionHeader(i);
      if (!section.ok()) continue;
      absl::StatusOr<std::string> name = names.Get(section->GetName());
      absl::StatusOr<elf_bridge::SectionType> type = section->GetType();
      std::cout << absl::StrFormat(
          "  [%2d] %-20s %-18s size=%#x\n", i, name.ok() ? *name : "?",
          type.ok() ? elf_bridge::SectionTypeToString(*type) : "?",
          section->GetSize());
    }
  }

  // 3. Looking up a single section by name and reading its contents.
  std::cout << "\n--- 3. Section Lookup by Name ---\n";
  absl::StatusOr<std::optional<elf_bridge::SectionHeader>> text =
      elf->GetSectionHeaderByName(".text");
  if (text.ok() && text->has_value()) {
    absl::StatusOr<elf_bridge::SectionData> text_data =
        elf->GetSectionData(**text);
    if (text_data.ok()) {
      std::cout << ".text is " << text_data->GetData().size()
                << " bytes, loaded at "
                << absl::StrFormat("%#x", (*text)->GetAddress()) << "\n";
    }
  } else {
    std::cout << "No .text section found.\n";
  }

  // 4. Iterating segments (program headers).
  std::cout << "\n--- 4. Segments ---\n";
  absl::StatusOr<std::optional<elf_bridge::SegmentTable>> segments =
      elf->GetSegments();
  if (segments.ok() && segments->has_value()) {
    for (size_t i = 0; i < (*segments)->size(); ++i) {
      absl::StatusOr<elf_bridge::ProgramHeader> phdr =
          (*segments)->GetProgramHeader(i);
      if (!phdr.ok()) continue;
      absl::StatusOr<elf_bridge::ProgramHeaderType> type = phdr->GetType();
      std::cout << absl::StrFormat(
          "  %-16s vaddr=0x%08x filesz=%#x memsz=%#x\n",
          type.ok() ? elf_bridge::ProgramHeaderTypeToString(*type) : "?",
          phdr->GetVirtualAddress(), phdr->GetFileSize(),
          phdr->GetMemorySize());
    }
  }

  // 5. Reading the dynamic section to list shared library dependencies.
  std::cout << "\n--- 5. Shared Library Dependencies (DT_NEEDED) ---\n";
  absl::StatusOr<elf_bridge::CommonElfData> common = elf->GetCommonData();
  if (common.ok() && common->GetDynamicTable().has_value() &&
      common->GetDynamicSymbolTableStrings().has_value()) {
    const elf_bridge::DynTable& dynamic = *common->GetDynamicTable();
    const elf_bridge::StringTable& dynstr =
        *common->GetDynamicSymbolTableStrings();
    for (size_t i = 0; i < dynamic.size(); ++i) {
      absl::StatusOr<elf_bridge::Dyn> entry = dynamic.GetDyn(i);
      if (!entry.ok() || entry->GetTag() != elf_bridge::DynamicTag::kNeeded) {
        continue;
      }
      absl::StatusOr<std::string> lib = dynstr.Get(entry->GetValue());
      std::cout << "  " << (lib.ok() ? *lib : "?") << "\n";
    }
  } else {
    std::cout << "Not dynamically linked.\n";
  }

  // 6. Iterating the symbol table.
  std::cout << "\n--- 6. Symbols (first 10 defined functions) ---\n";
  absl::StatusOr<std::optional<elf_bridge::SymbolTableWithStrings>> symtab =
      elf->GetSymbolTable();
  if (!symtab.ok() || !symtab->has_value()) {
    // Stripped binaries usually only have a dynamic symbol table.
    symtab = elf->GetDynamicSymbolTable();
  }
  if (symtab.ok() && symtab->has_value()) {
    const elf_bridge::SymbolTable& symbols = (*symtab)->GetSymbolTable();
    const elf_bridge::StringTable& strings = (*symtab)->GetStringTable();
    int printed = 0;
    for (size_t i = 0; i < symbols.size() && printed < 10; ++i) {
      absl::StatusOr<elf_bridge::Symbol> symbol = symbols.GetSymbol(i);
      if (!symbol.ok() || symbol->IsUndefined()) continue;
      absl::StatusOr<elf_bridge::SymbolType> type = symbol->GetType();
      if (!type.ok() || *type != elf_bridge::SymbolType::kFunc) continue;
      absl::StatusOr<std::string> name = strings.Get(symbol->GetName());
      absl::StatusOr<elf_bridge::SymbolBind> bind = symbol->GetBinding();
      std::cout << absl::StrFormat(
          "  %#018x %6d %-8s %s\n", symbol->GetValue(), symbol->GetSize(),
          bind.ok() ? elf_bridge::SymbolBindToString(*bind) : "?",
          name.ok() ? *name : "?");
      ++printed;
    }
  } else {
    std::cout << "No symbol table found.\n";
  }

  // 7. Reading notes, e.g. the GNU build ID.
  std::cout << "\n--- 7. GNU Build ID ---\n";
  absl::StatusOr<std::optional<elf_bridge::SectionHeader>> build_id_section =
      elf->GetSectionHeaderByName(".note.gnu.build-id");
  bool found_build_id = false;
  if (build_id_section.ok() && build_id_section->has_value()) {
    absl::StatusOr<elf_bridge::NoteTable> notes =
        elf->GetSectionDataAsNotes(**build_id_section);
    for (size_t i = 0; notes.ok() && i < notes->size(); ++i) {
      absl::StatusOr<elf_bridge::Note> note = notes->Get(i);
      if (!note.ok() || note->GetType() != elf_bridge::NoteType::kGnuBuildId) {
        continue;
      }
      absl::StatusOr<elf_bridge::NoteGnuBuildId> id = note->GetAsGnuBuildId();
      if (!id.ok()) continue;
      std::cout << "  ";
      for (char c : id->GetBuildId()) {
        std::cout << absl::StrFormat("%02x", static_cast<uint8_t>(c));
      }
      std::cout << "\n";
      found_build_id = true;
    }
  }
  if (!found_build_id) std::cout << "No GNU build ID found.\n";

  // 8. Streaming access with ElfStream, which reads from disk on demand
  //    instead of loading the whole file into memory.
  std::cout << "\n--- 8. Streaming with ElfStream ---\n";
  absl::StatusOr<elf_bridge::ElfStream> stream =
      elf_bridge::ElfStream::Open(path);
  if (stream.ok()) {
    absl::StatusOr<elf_bridge::SectionHeaderTable> stream_sections =
        stream->GetSectionHeaders();
    absl::StatusOr<std::optional<elf_bridge::SectionHeader>> comment =
        stream->GetSectionHeaderByName(".comment");
    if (stream_sections.ok()) {
      std::cout << "Section count: " << stream_sections->size() << "\n";
    }
    if (comment.ok() && comment->has_value()) {
      absl::StatusOr<elf_bridge::SectionData> comment_data =
          stream->GetSectionData(**comment);
      if (comment_data.ok()) {
        // .comment holds NUL-separated compiler identification strings.
        absl::string_view comment_text = comment_data->GetData();
        std::cout << "Compiler: "
                  << comment_text.substr(0, comment_text.find('\0')) << "\n";
      }
    }
  } else {
    std::cerr << "Failed to open stream: " << stream.status() << "\n";
  }

  // 9. Built-in human-readable dump helpers (see dump.h).
  std::cout << "\n--- 9. Dump Helpers ---\n";
  elf_bridge::DumpFileHeader(std::cout, *elf);

  return 0;
}
