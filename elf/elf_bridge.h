#ifndef SAFE_BINDINGS_ELF_ELF_BRIDGE_H_
#define SAFE_BINDINGS_ELF_ELF_BRIDGE_H_

// This library provides functionality for parsing ELF (Executable and Linkable
// Format) files in C++. It is a bridge to the underlying Rust `elf-wrapper`
// crate, offering a safe and efficient way to inspect ELF file structures such
// as headers, sections, and symbols.
//
// This library is suitable for tools and systems that need to extract
// information from ELF files, such as binary analysis tools, debuggers, or
// linkers. It supports both 32-bit and 64-bit ELF files and various machine
// architectures.
//
// This library provides two main entry points for parsing:
//  * ElfBytes::Parse(data) for parsing ELF files already loaded into memory via
//    an absl::string_view.
//  * ElfStream::Open(path) for parsing ELF files from disk via streaming,
//    which is more suitable for large files.
//
// Example using ElfBytes:
//
//   #include "elf_bridge.h"
//   #include "absl/status/statusor.h"
//
//   void ProcessElf(absl::string_view elf_data) {
//     // Assume elf_data is a string containing the ELF file contents.
//     absl::StatusOr<elf_bridge::ElfBytes> elf_bytes =
//         elf_bridge::ElfBytes::Parse(elf_data);
//     if (!elf_bytes.ok()) {
//       LOG(ERROR) << "Failed to parse ELF file: " << elf_bytes.status();
//       return;
//     }
//
//     absl::StatusOr<elf_bridge::FileHeader> header =
//         elf_bytes->GetFileHeader();
//     if (!header.ok()) {
//       LOG(ERROR) << "Failed to get file header: " << header.status();
//       return;
//     }
//
//     LOG(INFO) << "ELF class: " << header->GetClass();
//     LOG(INFO) << "ELF entry point: " << header->GetEntry();
//
//     absl::StatusOr<elf_bridge::ElfMachine> machine = header->GetMachine();
//     if (!machine.ok()) {
//       LOG(ERROR) << "Failed to get machine type: " << machine.status();
//       return;
//     }
//     LOG(INFO) << "Machine: " << *machine;
//   }
//
// Lifetime, and what the compiler does and does not catch
// -------------------------------------------------------
// Every accessor in this header that hands out a view or a reference into an
// object is `const&`-qualified, carries ABSL_ATTRIBUTE_LIFETIME_BOUND, and has
// its `const&&` overload deleted. Clang then rejects (-Werror,-Wdangling) an
// initialization taken off a temporary, including one reached through
// `absl::StatusOr::operator->`, which absl does annotate:
//
//     absl::string_view v = elf.GetSectionData(i)->GetData();   // error
//
// Three patterns are NOT diagnosed anywhere in this header and remain the
// caller's responsibility:
//   1. Assignment into an already-declared variable rather than
//      initialization.
//   2. A view or reference escaping the scope of the named object it came
//      from.
//   3. Laundering a view through an intervening unannotated function call.
//
// There is a fourth gap that matters here specifically, because several entry
// points have the shape `absl::StatusOr<std::optional<T>>`.
// `std::optional::operator->` is the one optional accessor that is neither
// ref-qualified nor ABSL_ATTRIBUTE_LIFETIME_BOUND, so it breaks the chain.
// Verified against the compiler; each of these compiles silently and dangles:
//
//     const StringTable& st =
//         (*elf.GetSymbolTable())->GetStringTable();
//     std::string_view file =
//         (*version_table.GetRequirement(i))->GetFile();
//     const Symbol& sym = (*hash_table.Find(name))->GetSymbol();
//
// Spelling the same thing with a full dereference does reach the deleted `&&`
// overload and is rejected, but the readable fix is to bind the optional to a
// named local before touching the object inside it.

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>

#include "crubit/elf_cpp_bindings.h"
#include "absl/base/attributes.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace elf_bridge {

// Forward declarations for helper functions
template <typename To, typename From>
std::optional<To> MapOption(const From& o);

template <typename To, typename From>
absl::StatusOr<To> MapResult(const From& r);

template <typename To, typename From>
absl::StatusOr<To> MapResult(From&& r);

template <typename To, typename From, typename F>
absl::StatusOr<To> MapResult(const From& r, F func);

template <typename To, typename From, typename F>
absl::StatusOr<To> MapResult(From&& r, F func);

template <typename To, typename From, typename F, typename E>
absl::StatusOr<To> MapResult(const From& r, F func, E err_func);

template <typename To, typename From, typename F, typename E>
absl::StatusOr<To> MapResult(From&& r, F func, E err_func);

// ELF types based on ELF specification for semantic clarity
using Elf_Half = uint16_t;
using Elf_Word = uint32_t;
using Elf_Sword = int32_t;
using Elf_Xword = uint64_t;
using Elf_Sxword = int64_t;
using Elf32_Addr = uint32_t;
using Elf32_Off = uint32_t;
using Elf64_Addr = uint64_t;
using Elf64_Off = uint64_t;

class CommonElfData;
class CompressionHeader;
class Dyn;
class DynTable;
class FileHeader;
class GnuHashTable;
class Note;
class NoteAny;
class NoteTable;
class NoteGnuAbiTag;
class NoteGnuBuildId;
class ProgramHeader;
class Rel;
class Rela;
class RelTable;
class RelaTable;
class SegmentTable;
class StringTable;
class SectionData;
class SectionHeader;
class SectionHeaderTable;
class SectionHeaderTableWithStrings;
class Symbol;
class SymbolDefinition;
class SymbolFindResult;
class SymbolRequirement;
class SymbolTable;
class SymbolTableWithStrings;
class SymbolVersionTable;
class SysVHashTable;

using AnyEndian = elf_cpp_bindings::constants::AnyEndian;
using DynamicTag = elf_cpp_bindings::constants::DynamicTag;
using ElfClass = elf_cpp_bindings::constants::ElfClass;
using ElfMachine = elf_cpp_bindings::constants::ElfMachine;
using NoteType = elf_cpp_bindings::note::NoteType;
using ProgramHeaderType = elf_cpp_bindings::constants::ProgramHeaderType;
using SectionType = elf_cpp_bindings::constants::SectionType;
using SymbolType = elf_cpp_bindings::constants::SymbolType;
using SymbolBind = elf_cpp_bindings::constants::SymbolBind;
using SymbolVisibility = elf_cpp_bindings::constants::SymbolVisibility;

// ElfBytes is a wrapper around an ELF file's bytes, providing access to
// various parts of the ELF file such as headers, sections, and segments.
// This is the main entry point for parsing ELF files.
class ElfBytes final {
 public:
  // Parses ELF data from the provided string_view. The data is copied by the
  // underlying Rust implementation for safety.
  //
  // Typical return codes:
  //  * OK
  //  * INVALID_ARGUMENT - If elf_data is not a valid or supported ELF file.
  static absl::StatusOr<ElfBytes> Parse(absl::string_view elf_data);

  // ElfBytes is move-only.
  ElfBytes(const ElfBytes&) = delete;
  ElfBytes& operator=(const ElfBytes&) = delete;
  ElfBytes(ElfBytes&&) = default;
  ElfBytes& operator=(ElfBytes&&) = default;

  // Returns the ELF file header.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the file header cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<FileHeader> GetFileHeader() const;

  // Returns the ELF segment table, also known as program header table.
  // Returns std::nullopt if no segment table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If segments cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<std::optional<SegmentTable>> GetSegments() const;

  // Returns raw data for the segment represented by `program_header`.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If segment data cannot be accessed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<std::string> GetSegmentData(
      const ProgramHeader& program_header) const;

  // Returns the ELF section header table.
  // Returns std::nullopt if no section header table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section headers cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<std::optional<SectionHeaderTable>> GetSectionHeaders() const;

  // Returns the section header for the section with the given `name`.
  // Returns std::nullopt if no section with that name is found or if
  // section names are not available.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If `name` is not valid UTF-8, or if section headers
  //               cannot be accessed or parsed, which should not happen
  //               for an ElfBytes object created via Parse.
  absl::StatusOr<std::optional<SectionHeader>> GetSectionHeaderByName(
      absl::string_view name) const;

  // Returns the section header table along with the associated section name
  // string table.
  // Returns std::nullopt if no section header table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section headers cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<SectionHeaderTableWithStrings> GetSectionHeadersWithStrTab()
      const;

  // Returns commonly used ELF data structures such as symbol tables, string
  // tables, dynamic table and hash tables.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If common data structures cannot be accessed or parsed,
  //               which should not happen for an ElfBytes object created via
  //               Parse on a valid ELF file.
  absl::StatusOr<CommonElfData> GetCommonData() const;

  // Returns data for the section represented by `section_header`.
  // This may include compression information if the section is compressed.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<SectionData> GetSectionData(
      const SectionHeader& section_header) const;

  // Returns section data for `section_header` parsed as a `StringTable`.
  // This is useful for sections like `.shstrtab` or `.strtab`.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a string
  //               table, which should not happen for an ElfBytes object
  //               created via Parse on a valid ELF file.
  absl::StatusOr<StringTable> GetSectionDataAsStrTab(
      const SectionHeader& section_header) const;

  // Returns symbol table section (.symtab) along with its associated string
  // table (.strtab).
  // Returns std::nullopt if no symbol table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol table cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolTableWithStrings>> GetSymbolTable() const;

  // Returns dynamic symbol table section (.dynsym) along with its associated
  // string table (.dynstr).
  // Returns std::nullopt if no dynamic symbol table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If dynamic symbol table cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolTableWithStrings>> GetDynamicSymbolTable()
      const;

  // Returns the dynamic section table (.dynamic).
  // This section contains information for dynamic linking.
  // Returns std::nullopt if no dynamic section is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If dynamic section cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<std::optional<DynTable>> GetDynamic() const;

  // Returns section data for `section_header` parsed as a `RelTable` (a
  // table of relocation entries without explicit addends).
  // This is used for sections of type SHT_REL.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a RelTable,
  //               which should not happen for an ElfBytes object created via
  //               Parse on a valid ELF file.
  absl::StatusOr<RelTable> GetSectionDataAsRels(
      const SectionHeader& section_header) const;

  // Returns section data for `section_header` parsed as a `RelaTable` (a
  // table of relocation entries with explicit addends).
  // This is used for sections of type SHT_RELA.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a RelaTable,
  //               which should not happen for an ElfBytes object created via
  //               Parse on a valid ELF file.
  absl::StatusOr<RelaTable> GetSectionDataAsRelas(
      const SectionHeader& section_header) const;

  // Returns section data for `section_header` parsed as a `NoteTable` (a
  // table of note entries).
  // This is used for sections of type SHT_NOTE.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a NoteTable,
  //               which should not happen for an ElfBytes object created via
  //               Parse on a valid ELF file.
  absl::StatusOr<NoteTable> GetSectionDataAsNotes(
      const SectionHeader& section_header) const;

  // Returns segment data for `program_header` parsed as a `NoteTable` (a
  // table of note entries).
  // This is used for segments of type PT_NOTE.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If segment data cannot be accessed or parsed as a NoteTable,
  //               which should not happen for an ElfBytes object created via
  //               Parse on a valid ELF file.
  absl::StatusOr<NoteTable> GetSegmentDataAsNotes(
      const ProgramHeader& program_header) const;

  // Returns symbol version table section (.gnu.version), which contains
  // symbol versioning information.
  // Returns std::nullopt if no symbol version table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol version table cannot be accessed or parsed, which
  //               should not happen for an ElfBytes object created via Parse.
  absl::StatusOr<std::optional<SymbolVersionTable>> GetSymbolVersionTable()
      const;

 private:
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From>
  friend std::optional<To> elf_bridge::MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> elf_bridge::MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> elf_bridge::MapResult(From&& r);

  explicit ElfBytes(elf_cpp_bindings::elf_bytes::ElfBytes inner);

  elf_cpp_bindings::elf_bytes::ElfBytes inner_;
};

// ElfStream provides streaming access to an ELF file on disk.
// Unlike ElfBytes, which requires the entire file to be loaded into memory,
// ElfStream reads from the file only as needed, making it efficient for
// very large files or memory-constrained environments.
class ElfStream final {
 public:
  // Opens an ELF file for streaming parsing.
  // This method is suitable for parsing large ELF files from disk without
  // reading the entire file into memory. It opens the file at `file_path`
  // and prepares it for streaming reads.
  //
  // Typical return codes:
  // * OK
  // * INVALID_ARGUMENT - If file_path is invalid, the file cannot be
  //                      opened/read, or is not a valid ELF file.
  static absl::StatusOr<ElfStream> Open(std::string_view file_path);

  // ElfStream is move-only.
  ElfStream(const ElfStream&) = delete;
  ElfStream& operator=(const ElfStream&) = delete;
  ElfStream(ElfStream&&) = default;
  ElfStream& operator=(ElfStream&&) = default;

  // Returns the ELF file header.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If file header cannot be accessed or parsed from the stream.
  absl::StatusOr<FileHeader> GetFileHeader() const;

  // Returns the ELF segment table, also known as program header table.
  // If no segment table is present, returns a table for which `empty()` is
  // true.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If segments cannot be accessed or parsed from the stream.
  absl::StatusOr<SegmentTable> GetSegments() const;

  // Returns the ELF section header table.
  // If no section header table is present, returns a table for which `empty()`
  // is true.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section headers cannot be accessed from the stream.
  absl::StatusOr<SectionHeaderTable> GetSectionHeaders() const;

  // Returns the section header table along with the associated section name
  // string table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section headers cannot be accessed from the stream.
  absl::StatusOr<SectionHeaderTableWithStrings> GetSectionHeadersWithStrTab();

  // Returns the section header for the section with the given `name`.
  // Returns std::nullopt if no section with that name is found or if
  // section names are not available.
  //
  // Because this is a streaming parser, this method may read from the
  // underlying file to access section headers and load the section name
  // string table if they haven't been accessed yet.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section headers cannot be accessed or parsed from the
  //  stream.
  absl::StatusOr<std::optional<SectionHeader>> GetSectionHeaderByName(
      absl::string_view name);

  // Returns data for the section represented by `section_header`.
  // This may include compression information if the section is compressed.
  //
  // Because this is a streaming parser, this method may read from the
  // underlying file to access section data.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed from the
  //  stream.
  absl::StatusOr<SectionData> GetSectionData(
      const SectionHeader& section_header);

  // Returns section data for `section_header` parsed as a `StringTable`.
  // This is useful for sections like `.shstrtab` or `.strtab`.
  //
  // Because this is a streaming parser, this method may read from the
  // underlying file to access section data.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a string
  //               table from the stream.
  absl::StatusOr<StringTable> GetSectionDataAsStrTab(
      const SectionHeader& section_header);

  // Returns symbol table section (.symtab) along with its associated string
  // table (.strtab).
  // Returns std::nullopt if no symbol table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol table cannot be accessed or parsed from stream.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolTableWithStrings>> GetSymbolTable();

  // Returns dynamic symbol table section (.dynsym) along with its associated
  // string table (.dynstr).
  // Returns std::nullopt if no dynamic symbol table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If dynamic symbol table cannot be accessed or parsed from
  //  stream.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolTableWithStrings>> GetDynamicSymbolTable();

  // Returns the dynamic section table (.dynamic).
  // This section contains information for dynamic linking.
  // Returns std::nullopt if no dynamic section is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If dynamic section cannot be accessed or parsed, which
  //               should not happen for an ElfStream object created via Open on
  //               a valid ELF file.
  absl::StatusOr<std::optional<DynTable>> GetDynamic();

  // Returns section data for `section_header` parsed as a `RelTable` (a
  // table of relocation entries without explicit addends).
  // This is used for sections of type SHT_REL.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a RelTable,
  //               which should not happen for an ElfStream object created via
  //               Open on a valid ELF file.
  absl::StatusOr<RelTable> GetSectionDataAsRels(
      const SectionHeader& section_header);

  // Returns section data for `section_header` parsed as a `RelaTable` (a
  // table of relocation entries with explicit addends).
  // This is used for sections of type SHT_RELA.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a RelaTable,
  //               which should not happen for an ElfStream object created via
  //               Open on a valid ELF file.
  absl::StatusOr<RelaTable> GetSectionDataAsRelas(
      const SectionHeader& section_header);

  // Returns section data for `section_header` parsed as a `NoteTable` (a
  // table of note entries).
  // This is used for sections of type SHT_NOTE.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If section data cannot be accessed or parsed as a NoteTable,
  //               which should not happen for an ElfStream object created via
  //               Open on a valid ELF file.
  absl::StatusOr<NoteTable> GetSectionDataAsNotes(
      const SectionHeader& section_header);

  // Returns segment data for `program_header` parsed as a `NoteTable` (a
  // table of note entries).
  // This is used for segments of type PT_NOTE.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If segment data cannot be accessed or parsed as a NoteTable,
  //               which should not happen for an ElfStream object created via
  //               Open on a valid ELF file.
  absl::StatusOr<NoteTable> GetSegmentDataAsNotes(
      const ProgramHeader& program_header);

  // Returns symbol version table section (.gnu.version), which contains
  // symbol versioning information.
  // Returns std::nullopt if no symbol version table is present.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol version table cannot be accessed or parsed from
  //               the stream.
  absl::StatusOr<std::optional<SymbolVersionTable>> GetSymbolVersionTable();

 private:
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit ElfStream(elf_cpp_bindings::elf_stream::ElfStream stream);

  elf_cpp_bindings::elf_stream::ElfStream stream_;
};

// Represents the ELF file header. The file header is located at the beginning
// of the file and provides a roadmap to the rest of the file's contents.
class FileHeader final {
 public:
  // Returns the file class, which identifies the file as 32-bit or 64-bit.
  ElfClass GetClass() const;
  // Returns the data encoding of the processor-specific data in the file.
  // This specifies whether the ELF file uses little-endian or big-endian
  // byte order.
  AnyEndian GetEndianness() const;
  // Returns the machine architecture for which the file is intended.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the machine type is not recognized or cannot be accessed.
  absl::StatusOr<ElfMachine> GetMachine() const;
  // Returns the object file type, which categorizes the file as relocatable,
  // executable, shared object, or core dump.
  Elf_Half GetType() const;
  // Returns the file version.
  Elf_Word GetVersion() const;
  // Returns the virtual address of the program entry point.
  Elf64_Addr GetEntry() const;
  // Returns the OS ABI (Application Binary Interface).
  uint8_t GetOsAbi() const;
  // Returns the ABI version. The interpretation of this field depends on the
  // OS ABI.
  uint8_t GetAbiVersion() const;
  // Returns the file offset of the program header table.
  Elf64_Off GetProgramHeaderOffset() const;
  // Returns the file offset of the section header table.
  Elf64_Off GetSectionHeaderOffset() const;
  // Returns processor-specific flags associated with the file.
  Elf_Word GetFlags() const;
  // Returns the size of this header in bytes.
  Elf_Half GetElfHeaderSize() const;
  // Returns the size of a program header table entry in bytes.
  Elf_Half GetProgramHeaderEntrySize() const;
  // Returns the number of entries in the program header table.
  Elf_Half GetProgramHeaderNum() const;
  // Returns the size of a section header table entry in bytes.
  Elf_Half GetSectionHeaderEntrySize() const;
  // Returns the number of entries in the section header table.
  Elf_Half GetSectionHeaderNum() const;
  // Returns the section header table index of the entry associated with the
  // section name string table.
  Elf_Half GetSectionHeaderStringTableIndex() const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit FileHeader(const elf_cpp_bindings::file::FileHeader& header);
  elf_cpp_bindings::file::FileHeader header_;
};

// Represents a compression header found at the beginning of a
// compressed section's data.
class CompressionHeader final {
 public:
  // Returns the compression type.
  Elf_Word GetType() const;
  // Returns the uncompressed size of the section data.
  Elf_Xword GetSize() const;
  // Returns the required alignment for the uncompressed data.
  Elf_Xword GetAddressAlign() const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  friend class SectionData;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit CompressionHeader(
      const elf_cpp_bindings::compression::CompressionHeader& header);
  elf_cpp_bindings::compression::CompressionHeader header_;
};

// Represents a string table section, which contains null-terminated character
// sequences.
class StringTable final {
 public:
  // Returns the string at the given index in the string table.
  // The index refers to a byte offset within the string table section.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds, or if the string at the given
  //               index is malformed or not valid UTF-8.
  absl::StatusOr<std::string> Get(size_t index) const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  friend class SymbolTableWithStrings;
  friend class SectionHeaderTableWithStrings;
  friend class GnuHashTable;
  friend class SysVHashTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit StringTable(const elf_cpp_bindings::string_table::StringTable& table);
  elf_cpp_bindings::string_table::StringTable table_;
};

// Represents the program header table, which is an array of segment
// descriptors.
class SegmentTable final {
 public:
  // Returns the number of entries in the segment table.
  size_t size() const;
  // Returns true if the segment table is empty.
  bool empty() const;
  // Returns the program header at `index` in the segment table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<ProgramHeader> GetProgramHeader(size_t index) const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit SegmentTable(const elf_cpp_bindings::segment::SegmentTable& table);
  elf_cpp_bindings::segment::SegmentTable table_;
};

// Represents a single entry in the program header table, describing a segment
// or other information required to prepare the program for execution.
class ProgramHeader final {
 public:
  // Returns the type of segment this header describes (e.g., loadable segment,
  // dynamic linking information).
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the segment type is not recognized or cannot be accessed.
  absl::StatusOr<ProgramHeaderType> GetType() const;
  // Returns segment-dependent flags.
  Elf_Word GetFlags() const;
  // Returns the offset from the beginning of the file at which the first byte
  // of the segment resides.
  Elf64_Off GetOffset() const;
  // Returns the virtual address at which the first byte of the segment resides
  // in memory.
  Elf64_Addr GetVirtualAddress() const;
  // On systems for which physical addressing is relevant, this member is
  // reserved for the segment's physical address.
  Elf64_Addr GetPhysicalAddress() const;
  // Returns the number of bytes in the file image of the segment. May be zero.
  Elf_Xword GetFileSize() const;
  // Returns the number of bytes in the memory image of the segment. May be
  // zero.
  Elf_Xword GetMemorySize() const;
  // Returns the alignment constraint for the segment. 0 and 1 mean no alignment
  // is required. Otherwise, `p_align` should be a positive power of 2, and
  // `p_vaddr` should equal `p_offset` % `p_align`.
  Elf_Xword GetAlignment() const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  friend class SegmentTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit ProgramHeader(const elf_cpp_bindings::segment::ProgramHeader& header);
  elf_cpp_bindings::segment::ProgramHeader header_;
};

// Represents a single entry in the section header table, describing a section.
class SectionHeader final {
 public:
  // Returns an index into the section header string table section, giving the
  // name of the section.
  Elf_Word GetName() const;
  // Returns the section's type, categorizing its contents and semantics.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the section type is not recognized or cannot be accessed.
  absl::StatusOr<SectionType> GetType() const;
  // Returns section-specific attribute flags.
  Elf_Xword GetFlags() const;
  // Returns the file offset from the beginning of the file to the first byte
  // in the section.
  Elf64_Off GetOffset() const;
  // If the section appears in the memory image of a process, this member gives
  // the address at which the section's first byte should reside.
  Elf64_Addr GetAddress() const;
  // Returns the section's size in bytes.
  Elf_Xword GetSize() const;
  // Returns a section header table index link, whose interpretation depends
  // on the section type.
  Elf_Word GetLink() const;
  // Holds extra information, whose interpretation depends on the section type.
  Elf_Word GetInfo() const;
  // Contains the required alignment of the section.
  Elf_Xword GetAddressAlign() const;
  // Contains the size in bytes of each entry, for sections that contain a
  // table of fixed-size entries.
  Elf_Xword GetEntrySize() const;

 private:
  friend class SectionHeaderTable;
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit SectionHeader(const elf_cpp_bindings::section::SectionHeader& header);
  elf_cpp_bindings::section::SectionHeader header_;
};

// Represents the ELF section header table, which is an array of section
// descriptors.
class SectionHeaderTable final {
 public:
  // Returns the number of entries in the section header table.
  size_t size() const;
  // Returns true if the section header table is empty.
  bool empty() const;
  // Returns the section header at `index` in the section header table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<SectionHeader> GetSectionHeader(size_t index) const;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  friend class SectionHeaderTableWithStrings;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);
  explicit SectionHeaderTable(
      const elf_cpp_bindings::section::SectionHeaderTable& table);
  elf_cpp_bindings::section::SectionHeaderTable table_;
};

// Holds a section header table and its associated section name string table.
class SectionHeaderTableWithStrings final {
 public:
  // Returns the section header table, if present.
  const std::optional<SectionHeaderTable>& GetSectionHeaderTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<SectionHeaderTable>& GetSectionHeaderTable() const&& =
      delete;
  // Returns the section name string table, if present.
  const std::optional<StringTable>& GetStringTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<StringTable>& GetStringTable() const&& = delete;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);
  explicit SectionHeaderTableWithStrings(
      const elf_cpp_bindings::types::SectionHeaderTableWithStrings&
          table_with_strings);
  std::optional<SectionHeaderTable> section_header_table_;
  std::optional<StringTable> string_table_;
};

// Represents raw data of a section, with optional compression information.
class SectionData final {
 public:
  // Returns the raw data of the section. If compressed, this is the
  // compressed data, starting after the compression header.
  //
  // WARNING: The returned string_view represents raw section bytes and is NOT
  // null-terminated. Do NOT pass `.data()` to C-style APIs expecting
  // null-terminated strings.
  // Lifetime: bound to this SectionData instance.
  //
  // The `const&&` overload is deleted and the accessor is
  // ABSL_ATTRIBUTE_LIFETIME_BOUND, so Clang rejects (-Werror,-Wdangling)
  // dangling *initializations*, including ones reached through
  // `StatusOr::operator->`:
  //     absl::string_view v = elf.GetSectionData(i)->GetData();   // error
  //
  // Three cases are NOT diagnosed and remain the caller's responsibility:
  //     absl::string_view v;
  //     v = elf.GetSectionData(i)->GetData();      // assignment: dangles
  //     { auto s = *elf.GetSectionData(i); v = s.GetData(); }     // scope exit
  //     v = Normalize(elf.GetSectionData(i)->GetData());  // laundered: dangles
  //
  // Consuming inside the full-expression is safe:
  //     std::string owned(elf.GetSectionData(i)->GetData());      // OK
  absl::string_view GetData() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view GetData() const&& = delete;

  // Returns the compression header if the section is compressed, otherwise
  // std::nullopt.
  // Lifetime: bound to this SectionData instance. See GetData() above for
  // exactly what the compiler does and does not catch.
  const std::optional<CompressionHeader>& GetCompression()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<CompressionHeader>& GetCompression() const&& = delete;

 private:
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  template <typename To, typename From>
  friend std::optional<To> elf_bridge::MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> elf_bridge::MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> elf_bridge::MapResult(From&& r);

  explicit SectionData(std::string data,
                       std::optional<CompressionHeader> compression);
  std::string data_;
  std::optional<CompressionHeader> compression_;
};

// Represents a single entry in a symbol table.
class Symbol final {
 public:
  // Returns an index into the symbol table's string table, which holds the
  // character representations of the symbol names.
  Elf_Word GetName() const;
  // Returns the section header index in relation to which the symbol is
  // defined.
  Elf_Half GetSectionHeaderIndex() const;
  // Returns the value of the symbol. This may be an absolute value or an
  // address, depending on context.
  Elf64_Addr GetValue() const;
  // Returns size associated with the symbol. For example, a data object's
  // size is the number of bytes contained in the object.
  Elf_Xword GetSize() const;
  // Returns true if the symbol is undefined (i.e., its section index is
  // SHN_UNDEF).
  bool IsUndefined() const;
  // Returns the symbol's type (e.g., data object, function).
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the symbol type is not recognized or cannot be accessed.
  absl::StatusOr<SymbolType> GetType() const;
  // Returns the symbol's binding attributes (e.g., local, global).
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the symbol binding is not recognized or cannot be
  //  accessed.
  absl::StatusOr<SymbolBind> GetBinding() const;
  // Returns the symbol's visibility (e.g., default, hidden).
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If the symbol visibility is not recognized or cannot be
  //  accessed.
  absl::StatusOr<SymbolVisibility> GetVisibility() const;

 private:
  friend class SymbolTable;
  friend class SectionHeaderTableWithStrings;
  friend class SymbolFindResult;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit Symbol(const elf_cpp_bindings::symbol::Symbol& symbol);
  elf_cpp_bindings::symbol::Symbol symbol_;
};

// Represents a symbol table section (e.g., .symtab).
class SymbolTable final {
 public:
  // Returns the number of entries in the symbol table.
  size_t size() const;
  // Returns true if the symbol table is empty.
  bool empty() const;
  // Returns the symbol at `index` in the symbol table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<Symbol> GetSymbol(size_t index) const;

 private:
  friend class ElfBytes;
  friend class SymbolTableWithStrings;
  friend class SymbolVersionTable;
  friend class GnuHashTable;
  friend class SysVHashTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);
  explicit SymbolTable(const elf_cpp_bindings::symbol::SymbolTable& table);
  elf_cpp_bindings::symbol::SymbolTable table_;
};

// Holds a symbol table and its associated string table.
class SymbolTableWithStrings final {
 public:
  // Returns the symbol table.
  const SymbolTable& GetSymbolTable() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const SymbolTable& GetSymbolTable() const&& = delete;
  // Returns the string table associated with the symbol table.
  const StringTable& GetStringTable() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const StringTable& GetStringTable() const&& = delete;

 private:
  friend class ElfBytes;
  friend class ElfStream;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);
  template <typename To, typename From, typename F, typename E>
  friend absl::StatusOr<To> MapResult(const From& r, F func, E err_func);
  template <typename To, typename From, typename F, typename E>
  friend absl::StatusOr<To> MapResult(From&& r, F func, E err_func);

  explicit SymbolTableWithStrings(
      const elf_cpp_bindings::types::SymbolTableWithStrings& table_with_strings);
  SymbolTable symbol_table_;
  StringTable string_table_;
};

// Represents a single entry in the dynamic section table.
class Dyn final {
 public:
  // Returns tag for this entry, e.g., DT_NEEDED, DT_STRTAB.
  DynamicTag GetTag() const;
  // Returns value or address associated with this tag.
  Elf_Xword GetValue() const;
  // Returns value or address associated with this tag.
  Elf64_Addr GetPtr() const;

 private:
  friend class DynTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit Dyn(const elf_cpp_bindings::dynamic::Dyn& dyn);
  elf_cpp_bindings::dynamic::Dyn dyn_;
};

// Represents dynamic section table (.dynamic), which holds information
// needed by the dynamic linker.
class DynTable final {
 public:
  // Returns the number of entries in the dynamic section table.
  size_t size() const;
  // Returns true if the dynamic section table is empty.
  bool empty() const;
  // Returns the dynamic entry at `index` in the dynamic section table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<Dyn> GetDyn(size_t index) const;

 private:
  explicit DynTable(const elf_cpp_bindings::dynamic::DynTable& table);
  friend class ElfBytes;
  friend class ElfStream;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  elf_cpp_bindings::dynamic::DynTable table_;
};

// Represents a relocation entry without an explicit addend (SHT_REL).
class Rel final {
 public:
  // Returns location at which to apply the relocation action.
  Elf64_Addr GetOffset() const;
  // Returns symbol table index with respect to which the relocation must be
  // made.
  Elf_Word GetSym() const;
  // Returns relocation type, indicating how the relocation should be
  // performed.
  Elf_Word GetType() const;

 private:
  friend class RelTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit Rel(const elf_cpp_bindings::relocation::Rel& rel);
  elf_cpp_bindings::relocation::Rel rel_;
};

// Represents a table of `Rel` entries (section of type SHT_REL).
class RelTable {
 public:
  explicit RelTable(const elf_cpp_bindings::relocation::RelTable& table);
  // Returns the number of entries in the relocation table.
  size_t size() const;
  // Returns true if the relocation table is empty.
  bool empty() const;
  // Returns the relocation entry at `index` in the table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<Rel> GetRel(size_t index) const;

 private:
  elf_cpp_bindings::relocation::RelTable table_;
};

// Represents a relocation entry with an explicit addend (SHT_RELA).
class Rela final {
 public:
  // Returns location at which to apply the relocation action.
  Elf64_Addr GetOffset() const;
  // Returns symbol table index with respect to which the relocation must be
  // made.
  Elf_Word GetSym() const;
  // Returns relocation type, indicating how the relocation should be
  // performed.
  Elf_Word GetType() const;
  // Returns explicit addend used to compute the value to be stored in the
  // relocated field.
  Elf_Sxword GetAddend() const;

 private:
  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit Rela(const elf_cpp_bindings::relocation::Rela& rela);
  elf_cpp_bindings::relocation::Rela rela_;
};

// Represents a table of `Rela` entries (section of type SHT_RELA).
class RelaTable {
 public:
  explicit RelaTable(const elf_cpp_bindings::relocation::RelaTable& table);
  // Returns the number of entries in the relocation table.
  size_t size() const;
  // Returns true if the relocation table is empty.
  bool empty() const;
  // Returns the relocation entry at `index` in the table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<Rela> GetRela(size_t index) const;

 private:
  elf_cpp_bindings::relocation::RelaTable table_;
};

// Represents a generic ELF note with vendor/name and descriptor.
class NoteAny final {
 public:
  // Returns note type.
  Elf_Xword GetType() const;
  // Returns note vendor/name.
  //
  // Zero-copy: the view points into this note's own buffer and is valid while
  // this NoteAny is alive. It is NOT NUL-terminated and may contain embedded
  // NUL bytes -- never hand `.data()` to a C string API. Write
  // `std::string(note.GetName())` at the one call site that needs a C string.
  std::string_view GetName() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::string_view GetName() const&& = delete;
  // Returns note descriptor. Same zero-copy and NUL-termination caveats as
  // GetName(); descriptors are arbitrary binary payloads.
  std::string_view GetDesc() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::string_view GetDesc() const&& = delete;

 private:
  friend class Note;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit NoteAny(const elf_cpp_bindings::note::NoteAny& any);
  elf_cpp_bindings::note::NoteAny any_;
};

// Represents a parsed ELF note. A note can be of a known core/GNU type,
// or generic/unknown.
class Note final {
 public:
  // Returns note type enum, indicating whether this is a known note type or
  // generic/unknown.
  elf_cpp_bindings::note::NoteType GetType() const;
  // If this note is a GNU ABI tag note, returns it as such.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If this note is not a GNU ABI tag note.
  absl::StatusOr<NoteGnuAbiTag> GetAsGnuAbiTag() const;
  // If this note is a GNU build ID note, returns it as such.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If this note is not a GNU build ID note.
  absl::StatusOr<NoteGnuBuildId> GetAsGnuBuildId() const;
  // If this note is not of a known core/GNU type, returns it as a generic
  // note.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If this note is of a known core/GNU type.
  absl::StatusOr<NoteAny> GetAsUnknown() const;

 private:
  friend class ElfBytes;
  friend class NoteTable;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit Note(const elf_cpp_bindings::note::Note& note);
  elf_cpp_bindings::note::Note note_;
};

// Represents a table of ELF notes (section of type SHT_NOTE or segment of
// type PT_NOTE).
class NoteTable final {
 public:
  // Returns number of notes in this table.
  size_t size() const;
  // Returns true if this table is empty.
  bool empty() const;
  // Returns note at `index` in this table.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds.
  absl::StatusOr<Note> Get(size_t index) const;

 private:
  friend class ElfBytes;
  friend class ElfStream;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit NoteTable(const elf_cpp_bindings::note::NoteTable& table);
  elf_cpp_bindings::note::NoteTable table_;
};

// Represents a GNU ABI tag note.
class NoteGnuAbiTag final {
 public:
  // Returns OS descriptor.
  Elf_Word GetOs() const;
  // Returns major version of minimum OS kernel version.
  Elf_Word GetMajor() const;
  // Returns minor version of minimum OS kernel version.
  Elf_Word GetMinor() const;
  // Returns subminor version of minimum OS kernel version.
  Elf_Word GetSubminor() const;

 private:
  friend class Note;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit NoteGnuAbiTag(const elf_cpp_bindings::note::NoteGnuAbiTag& tag);
  elf_cpp_bindings::note::NoteGnuAbiTag tag_;
};

// Represents a GNU build ID note.
class NoteGnuBuildId final {
 public:
  // Returns build ID bytes.
  //
  // Zero-copy: the view points into this note's own buffer and is valid while
  // this NoteGnuBuildId is alive. Build IDs are raw binary, NOT NUL-terminated
  // -- never hand `.data()` to a C string API.
  std::string_view GetBuildId() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::string_view GetBuildId() const&& = delete;

 private:
  friend class Note;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);
  explicit NoteGnuBuildId(const elf_cpp_bindings::note::NoteGnuBuildId& build_id);
  elf_cpp_bindings::note::NoteGnuBuildId build_id_;
};

// Represents result of finding a symbol in a hash table, containing symbol
// index and symbol itself.
class SymbolFindResult final {
 public:
  // Returns symbol index in symbol table.
  size_t GetIndex() const;
  // Returns symbol.
  const Symbol& GetSymbol() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const Symbol& GetSymbol() const&& = delete;

 private:
  friend class GnuHashTable;
  friend class SysVHashTable;
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);

  explicit SymbolFindResult(elf_cpp_bindings::elf_bytes::SymbolFindResult&& result);
  size_t index_;
  Symbol symbol_;
};

// Represents SysV hash table section (.hash), used for dynamic symbol
// lookup.
class SysVHashTable final {
 public:
  // Finds symbol with `name` in `symbol_table` using this hash table and
  // associated `string_table`.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If hash table, symbol table, or string table are malformed.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolFindResult>> Find(
      std::string_view name, const SymbolTable& symbol_table,
      const StringTable& string_table) const;

 private:
  friend class ElfBytes;
  friend class CommonElfData;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  explicit SysVHashTable(elf_cpp_bindings::hash::SysVHashTable&& table);
  elf_cpp_bindings::hash::SysVHashTable table_;
};

// Represents GNU hash table section (.gnu.hash), used for dynamic symbol
// lookup.
class GnuHashTable final {
 public:
  // Finds symbol with `name` in `symbol_table` using this hash table and
  // associated `string_table`.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If hash table, symbol table, or string table are malformed.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolFindResult>> Find(
      std::string_view name, const SymbolTable& symbol_table,
      const StringTable& string_table) const;

 private:
  friend class ElfBytes;
  friend class CommonElfData;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  explicit GnuHashTable(elf_cpp_bindings::hash::GnuHashTable&& table);
  elf_cpp_bindings::hash::GnuHashTable table_;
};

// Represents symbol version table section (.gnu.version), which contains
// symbol versioning requirements and definitions.
class SymbolVersionTable final {
 public:
  // Returns symbol version requirement for symbol at `sym_idx`, if one exists.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol version requirement table is malformed.
  //
  // Lifetime: see the note at the top of this file. Reaching into the
  // returned std::optional with `operator->` is NOT guarded.
  absl::StatusOr<std::optional<SymbolRequirement>> GetRequirement(
      size_t sym_idx) const;
  // Returns symbol version definition for symbol at `sym_idx`, if one exists.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If symbol version definition table is malformed.
  absl::StatusOr<std::optional<SymbolDefinition>> GetDefinition(
      size_t sym_idx) const;

 private:
  friend class ElfBytes;
  friend class ElfStream;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  explicit SymbolVersionTable(
      elf_cpp_bindings::gnu_symver::SymbolVersionTable&& table);
  elf_cpp_bindings::gnu_symver::SymbolVersionTable table_;
};

// Represents symbol version requirement (.gnu.version_r).
class SymbolRequirement final {
 public:
  // Returns name of file containing symbol definition.
  //
  // Zero-copy: the view points into this requirement's own buffer and is valid
  // while this SymbolRequirement is alive. It is NOT NUL-terminated -- never
  // hand `.data()` to a C string API; construct a `std::string` if you need
  // one.
  std::string_view GetFile() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::string_view GetFile() const&& = delete;
  // Returns name of symbol. Same zero-copy and NUL-termination caveats as
  // GetFile().
  std::string_view GetName() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::string_view GetName() const&& = delete;
  // Returns GNU hash of symbol name.
  Elf_Word GetHash() const;
  // Returns symbol flags.
  Elf_Half GetFlags() const;
  // Returns true if symbol is hidden.
  bool IsHidden() const;

 private:
  friend class SymbolVersionTable;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  explicit SymbolRequirement(elf_cpp_bindings::gnu_symver::SymbolRequirement&& req);

  elf_cpp_bindings::gnu_symver::SymbolRequirement req_;
};

// Represents symbol version definition (.gnu.version_d).
class SymbolDefinition final {
 public:
  // Returns GNU hash of symbol name.
  Elf_Word GetHash() const;
  // Returns symbol flags.
  Elf_Half GetFlags() const;
  // Returns true if symbol is hidden.
  bool IsHidden() const;
  // Returns number of names associated with this symbol definition.
  size_t GetNameCount() const;
  // Returns name at `index` associated with this symbol definition.
  //
  // Typical return codes:
  //  * OK
  //  * INTERNAL - If index is out of bounds, or name cannot be read.
  absl::StatusOr<std::string> GetName(size_t index) const;

 private:
  friend class SymbolVersionTable;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  explicit SymbolDefinition(elf_cpp_bindings::gnu_symver::SymbolDefinition def);
  elf_cpp_bindings::gnu_symver::SymbolDefinition def_;
};

// Holds commonly used ELF data structures such as symbol tables, string
// tables, dynamic table and hash tables.
class CommonElfData final {
 public:
  // Returns symbol table section (.symtab), if present.
  const std::optional<SymbolTable>& GetSymbolTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<SymbolTable>& GetSymbolTable() const&& = delete;
  // Returns string table section (.strtab), if present.
  const std::optional<StringTable>& GetSymbolTableStrings()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<StringTable>& GetSymbolTableStrings() const&& = delete;
  // Returns dynamic symbol table section (.dynsym), if present.
  const std::optional<SymbolTable>& GetDynamicSymbolTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<SymbolTable>& GetDynamicSymbolTable() const&& = delete;
  // Returns dynamic string table section (.dynstr), if present.
  const std::optional<StringTable>& GetDynamicSymbolTableStrings()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<StringTable>& GetDynamicSymbolTableStrings() const&& =
      delete;
  // Returns dynamic section table (.dynamic), if present.
  const std::optional<DynTable>& GetDynamicTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<DynTable>& GetDynamicTable() const&& = delete;
  // Returns SysV hash table section (.hash), if present.
  const std::optional<SysVHashTable>& GetSysVHashTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<SysVHashTable>& GetSysVHashTable() const&& = delete;
  // Returns GNU hash table section (.gnu.hash), if present.
  const std::optional<GnuHashTable>& GetGnuHashTable()
      const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  const std::optional<GnuHashTable>& GetGnuHashTable() const&& = delete;

 private:
  friend class ElfBytes;

  template <typename To, typename From>
  friend std::optional<To> MapOption(const From& o);
  template <typename To, typename From>
  friend std::optional<To> MapOption(From&& o);

  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(const From& r);
  template <typename To, typename From>
  friend absl::StatusOr<To> MapResult(From&& r);

  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(const From& r, F func);
  template <typename To, typename From, typename F>
  friend absl::StatusOr<To> MapResult(From&& r, F func);

  CommonElfData(std::optional<SymbolTable> symtab,
                std::optional<StringTable> symtab_strs,
                std::optional<SymbolTable> dynsyms,
                std::optional<StringTable> dynsyms_strs,
                std::optional<DynTable> dynamic,
                std::optional<SysVHashTable> sysv_hash,
                std::optional<GnuHashTable> gnu_hash);

  std::optional<SymbolTable> symtab_;
  std::optional<StringTable> symtab_strs_;
  std::optional<SymbolTable> dynsyms_;
  std::optional<StringTable> dynsyms_strs_;
  std::optional<DynTable> dynamic_;
  std::optional<SysVHashTable> sysv_hash_;
  std::optional<GnuHashTable> gnu_hash_;
};

// Functions to convert the different tag and type enums into strings.
std::string DynamicTagToString(DynamicTag tag);
std::string SectionTypeToString(SectionType type);
std::string MachineToString(ElfMachine machine);
std::string MachineToHumanString(ElfMachine machine);
std::string OsAbiToString(uint8_t osabi);
std::string ProgramHeaderTypeToString(ProgramHeaderType type);
std::string SymbolTypeToString(SymbolType type);
std::string SymbolBindToString(SymbolBind bind);
std::string SymbolVisibilityToString(SymbolVisibility visibility);

}  // namespace elf_bridge

#endif  // SAFE_BINDINGS_ELF_ELF_BRIDGE_H_
