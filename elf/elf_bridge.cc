#include "elf_bridge.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "crubit/elf_cpp_bindings.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "support/rs_std/result.h"

namespace elf_bridge {

template <typename To, typename From>
std::optional<To> MapOption(const From& o) {
  if (o.is_none()) {
    return std::nullopt;
  }
  return To(o.unwrap_as_ref());
}

template <typename To, typename From>
std::optional<To> MapOption(From&& o) {
  if (o.is_none()) {
    return std::nullopt;
  }
  if constexpr (std::is_same_v<decltype(o),
                               typename std::remove_reference_t<From>&&>) {
    // Only call the && unwrap if o is truly an rvalue reference
    return To(std::forward<From>(o).unwrap());
  } else {
    // Otherwise, call the const& unwrap_as_ref
    return To(o.unwrap_as_ref());
  }
}

template <typename To, typename From>
absl::StatusOr<To> MapResult(const From& r) {
  if (r.is_err()) {
    return absl::InternalError(r.unwrap_err_ref().as_str());
  }
  return To(r.unwrap_as_ref());
}

template <typename To, typename From>
absl::StatusOr<To> MapResult(From&& r) {
  if (r.is_err()) {
    return absl::InternalError(r.unwrap_err_ref().as_str());
  }
  if constexpr (std::is_same_v<decltype(r),
                               typename std::remove_reference_t<From>&&>) {
    // Only call the && unwrap if r is truly an rvalue reference
    return To(std::forward<From>(r).unwrap());
  } else {
    // Otherwise, call the const& unwrap_as_ref
    return To(r.unwrap_as_ref());
  }
}

template <typename T, typename E>
absl::StatusOr<std::string> MapResult(rs_std::Result<T, E>&& r) {
  if (!r.has_value()) {
    return absl::InternalError(absl::StrCat(r.error()));
  }
  return std::string(security::crubit_helpers::StringViewFromVecU8(r.value()));
}

template <typename To, typename From, typename F>
absl::StatusOr<To> MapResult(const From& r, F func) {
  if (r.is_err()) {
    return absl::InternalError(r.unwrap_err_ref().as_str());
  }
  return func(r.unwrap_as_ref());
}

template <typename To, typename From, typename F>
absl::StatusOr<To> MapResult(From&& r, F func) {
  if (r.is_err()) {
    return absl::InternalError(r.unwrap_err_ref().as_str());
  }
  if constexpr (std::is_same_v<decltype(r),
                               typename std::remove_reference_t<From>&&>) {
    // Only call the && unwrap if r is truly an rvalue reference
    return func(std::forward<From>(r).unwrap());
  } else {
    // Otherwise, call the const& unwrap_as_ref
    return func(r.unwrap_as_ref());
  }
}

template <typename To, typename From, typename F, typename E>
absl::StatusOr<To> MapResult(const From& r, F func, E err_func) {
  if (r.is_err()) {
    return err_func(r.unwrap_err_ref().as_str());
  }
  return func(r.unwrap_as_ref());
}

template <typename To, typename From, typename F, typename E>
absl::StatusOr<To> MapResult(From&& r, F func, E err_func) {
  if (r.is_err()) {
    return err_func(r.unwrap_err_ref().as_str());
  }
  if constexpr (std::is_same_v<decltype(r),
                               typename std::remove_reference_t<From>&&>) {
    // Only call the && unwrap if r is truly an rvalue reference
    return func(std::forward<From>(r).unwrap());
  } else {
    // Otherwise, call the const& unwrap_as_ref
    return func(r.unwrap_as_ref());
  }
}

// ElfBytes

ElfBytes::ElfBytes(elf_cpp_bindings::elf_bytes::ElfBytes inner)
    : inner_(std::move(inner)) {}

absl::StatusOr<ElfBytes> ElfBytes::Parse(absl::string_view elf_data) {
  elf_cpp_bindings::elf_bytes::ResultElfBytes result_elf_bytes =
      elf_cpp_bindings::elf_bytes::ElfBytes::parse(elf_data);
  return MapResult<ElfBytes>(std::move(result_elf_bytes));
}

absl::StatusOr<FileHeader> ElfBytes::GetFileHeader() const {
  auto result = inner_.file_header();
  return MapResult<FileHeader>(result);
}

absl::StatusOr<std::optional<SegmentTable>> ElfBytes::GetSegments() const {
  auto result = inner_.segments();
  return MapResult<std::optional<SegmentTable>>(
      std::move(result), [](const auto& optional_table) {
        return MapOption<SegmentTable>(optional_table);
      });
}

absl::StatusOr<std::string> ElfBytes::GetSegmentData(
    const ProgramHeader& program_header) const {
  auto result = inner_.segment_data(program_header.header_);
  return MapResult(std::move(result));
}

absl::StatusOr<std::optional<SectionHeaderTable>> ElfBytes::GetSectionHeaders()
    const {
  auto result = inner_.section_headers();
  return MapResult<std::optional<SectionHeaderTable>>(
      std::move(result), [](const auto& optional_table) {
        return MapOption<SectionHeaderTable>(optional_table);
      });
}

absl::StatusOr<std::optional<SectionHeader>> ElfBytes::GetSectionHeaderByName(
    absl::string_view name) const {
  auto result = inner_.section_header_by_name(name);
  return MapResult<std::optional<SectionHeader>>(
      std::move(result), [](const auto& optional_header) {
        return MapOption<SectionHeader>(optional_header);
      });
}

absl::StatusOr<SectionHeaderTableWithStrings>
ElfBytes::GetSectionHeadersWithStrTab() const {
  auto result = inner_.section_headers_with_strtab();
  return MapResult<SectionHeaderTableWithStrings>(std::move(result));
}

absl::StatusOr<SectionData> ElfBytes::GetSectionData(
    const SectionHeader& section_header) const {
  auto result = inner_.section_data(section_header.header_);
  return MapResult<SectionData>(std::move(result), [](auto&& section_data) {
    return SectionData(
        std::string(
            security::crubit_helpers::StringViewFromVecU8(section_data.data)),
        MapOption<CompressionHeader>(section_data.compression));
  });
}

absl::StatusOr<StringTable> ElfBytes::GetSectionDataAsStrTab(
    const SectionHeader& section_header) const {
  auto result = inner_.section_data_as_strtab(section_header.header_);
  return MapResult<StringTable>(std::move(result));
}

absl::StatusOr<std::optional<SymbolTableWithStrings>> ElfBytes::GetSymbolTable()
    const {
  auto result = inner_.symbol_table();
  return MapResult<std::optional<SymbolTableWithStrings>>(
      std::move(result), [](const auto& optional_table) {
        return MapOption<SymbolTableWithStrings>(optional_table);
      });
}

absl::StatusOr<std::optional<SymbolTableWithStrings>>
ElfBytes::GetDynamicSymbolTable() const {
  auto result = inner_.dynamic_symbol_table();
  return MapResult<std::optional<SymbolTableWithStrings>>(
      std::move(result), [](const auto& optional_table) {
        return MapOption<SymbolTableWithStrings>(optional_table);
      });
}

absl::StatusOr<std::optional<DynTable>> ElfBytes::GetDynamic() const {
  auto result = inner_.dynamic();
  return MapResult<std::optional<DynTable>>(
      std::move(result), [](const auto& optional_table) {
        return MapOption<DynTable>(optional_table);
      });
}

absl::StatusOr<RelTable> ElfBytes::GetSectionDataAsRels(
    const SectionHeader& section_header) const {
  auto result = inner_.section_data_as_rels(section_header.header_);
  return MapResult<RelTable>(result);
}

absl::StatusOr<RelaTable> ElfBytes::GetSectionDataAsRelas(
    const SectionHeader& section_header) const {
  auto result = inner_.section_data_as_relas(section_header.header_);
  return MapResult<RelaTable>(result);
}

absl::StatusOr<NoteTable> ElfBytes::GetSectionDataAsNotes(
    const SectionHeader& section_header) const {
  auto result = inner_.section_data_as_notes(section_header.header_);
  return MapResult<NoteTable>(result);
}

absl::StatusOr<NoteTable> ElfBytes::GetSegmentDataAsNotes(
    const ProgramHeader& program_header) const {
  auto result = inner_.segment_data_as_notes(program_header.header_);
  return MapResult<NoteTable>(result);
}

absl::StatusOr<std::optional<SymbolVersionTable>>
ElfBytes::GetSymbolVersionTable() const {
  auto result = inner_.symbol_version_table();
  return MapResult<std::optional<SymbolVersionTable>>(
      std::move(result), [](auto&& optional_table) {
        return MapOption<SymbolVersionTable>(
            std::forward<decltype(optional_table)>(optional_table));
      });
}

absl::StatusOr<CommonElfData> ElfBytes::GetCommonData() const {
  auto result = inner_.find_common_data();
  if (result.is_err()) {
    return absl::InternalError(result.unwrap_err_ref().as_str());
  }
  auto common_data = std::move(result).unwrap();
  return CommonElfData(
      MapOption<SymbolTable>(std::move(common_data.symtab)),
      MapOption<StringTable>(std::move(common_data.symtab_strs)),
      MapOption<SymbolTable>(std::move(common_data.dynsyms)),
      MapOption<StringTable>(std::move(common_data.dynsyms_strs)),
      MapOption<DynTable>(std::move(common_data.dynamic)),
      MapOption<SysVHashTable>(std::move(common_data.sysv_hash)),
      MapOption<GnuHashTable>(std::move(common_data.gnu_hash)));
}

// ElfStream
ElfStream::ElfStream(elf_cpp_bindings::elf_stream::ElfStream stream)
    : stream_(std::move(stream)) {}

absl::StatusOr<ElfStream> ElfStream::Open(std::string_view file_path) {
  elf_cpp_bindings::elf_stream::ResultElfStream result =
      elf_cpp_bindings::elf_stream::ElfStream::open_stream(file_path);
  return MapResult<ElfStream>(
      std::move(result),
      [](auto&& stream) {
        return ElfStream(std::forward<decltype(stream)>(stream));
      },
      [](absl::string_view err) { return absl::InvalidArgumentError(err); });
}

absl::StatusOr<FileHeader> ElfStream::GetFileHeader() const {
  auto result = stream_.file_header();
  if (result.is_err()) {
    return absl::InternalError(result.unwrap_err_ref().as_str());
  }
  return FileHeader(result.unwrap_as_ref());
}

absl::StatusOr<SegmentTable> ElfStream::GetSegments() const {
  auto result = stream_.segments();
  return MapResult<SegmentTable>(result);
}

absl::StatusOr<SectionHeaderTable> ElfStream::GetSectionHeaders() const {
  auto result = stream_.section_headers();
  return MapResult<SectionHeaderTable>(result);
}

absl::StatusOr<SectionHeaderTableWithStrings>
ElfStream::GetSectionHeadersWithStrTab() {
  auto result = stream_.section_headers_with_strtab();
  return MapResult<SectionHeaderTableWithStrings>(std::move(result));
}

absl::StatusOr<std::optional<SectionHeader>> ElfStream::GetSectionHeaderByName(
    absl::string_view name) {
  auto result = stream_.section_header_by_name(name);
  return MapResult<std::optional<SectionHeader>>(
      std::move(result), [](auto&& optional_header) {
        return MapOption<SectionHeader>(
            std::forward<decltype(optional_header)>(optional_header));
      });
}

absl::StatusOr<SectionData> ElfStream::GetSectionData(
    const SectionHeader& section_header) {
  auto result = stream_.section_data(section_header.header_);
  return MapResult<SectionData>(std::move(result), [](auto&& section_data) {
    return SectionData(
        std::string(
            security::crubit_helpers::StringViewFromVecU8(section_data.data)),
        MapOption<CompressionHeader>(std::move(section_data.compression)));
  });
}

absl::StatusOr<StringTable> ElfStream::GetSectionDataAsStrTab(
    const SectionHeader& section_header) {
  auto result = stream_.section_data_as_strtab(section_header.header_);
  return MapResult<StringTable>(std::move(result));
}

absl::StatusOr<std::optional<SymbolTableWithStrings>>
ElfStream::GetSymbolTable() {
  auto result = stream_.symbol_table();
  return MapResult<std::optional<SymbolTableWithStrings>>(
      std::move(result), [](auto&& optional_table) {
        return MapOption<SymbolTableWithStrings>(
            std::forward<decltype(optional_table)>(optional_table));
      });
}

absl::StatusOr<std::optional<SymbolTableWithStrings>>
ElfStream::GetDynamicSymbolTable() {
  auto result = stream_.dynamic_symbol_table();
  return MapResult<std::optional<SymbolTableWithStrings>>(
      std::move(result), [](auto&& optional_table) {
        return MapOption<SymbolTableWithStrings>(
            std::forward<decltype(optional_table)>(optional_table));
      });
}

absl::StatusOr<std::optional<DynTable>> ElfStream::GetDynamic() {
  auto result = stream_.dynamic();
  return MapResult<std::optional<DynTable>>(
      std::move(result), [](auto&& optional_table) {
        return MapOption<DynTable>(
            std::forward<decltype(optional_table)>(optional_table));
      });
}

absl::StatusOr<RelTable> ElfStream::GetSectionDataAsRels(
    const SectionHeader& section_header) {
  auto result = stream_.section_data_as_rels(section_header.header_);
  return MapResult<RelTable>(result);
}

absl::StatusOr<RelaTable> ElfStream::GetSectionDataAsRelas(
    const SectionHeader& section_header) {
  auto result = stream_.section_data_as_relas(section_header.header_);
  return MapResult<RelaTable>(result);
}

absl::StatusOr<NoteTable> ElfStream::GetSectionDataAsNotes(
    const SectionHeader& section_header) {
  auto result = stream_.section_data_as_notes(section_header.header_);
  return MapResult<NoteTable>(result);
}

absl::StatusOr<NoteTable> ElfStream::GetSegmentDataAsNotes(
    const ProgramHeader& program_header) {
  auto result = stream_.segment_data_as_notes(program_header.header_);
  return MapResult<NoteTable>(result);
}

absl::StatusOr<std::optional<SymbolVersionTable>>
ElfStream::GetSymbolVersionTable() {
  auto result = stream_.symbol_version_table();
  return MapResult<std::optional<SymbolVersionTable>>(
      std::move(result), [](auto&& optional_table) {
        return MapOption<SymbolVersionTable>(
            std::forward<decltype(optional_table)>(optional_table));
      });
}

// FileHeader
FileHeader::FileHeader(const elf_cpp_bindings::file::FileHeader& header)
    : header_(header) {}

ElfClass FileHeader::GetClass() const { return header_.class_; }
AnyEndian FileHeader::GetEndianness() const { return header_.endianness; }
absl::StatusOr<ElfMachine> FileHeader::GetMachine() const {
  auto result = header_.machine();
  return MapResult<ElfMachine>(result);
}
Elf_Half FileHeader::GetType() const { return header_.e_type; }
Elf_Word FileHeader::GetVersion() const { return header_.version; }
Elf64_Addr FileHeader::GetEntry() const { return header_.e_entry; }
uint8_t FileHeader::GetOsAbi() const { return header_.osabi; }
uint8_t FileHeader::GetAbiVersion() const { return header_.abiversion; }
Elf64_Off FileHeader::GetProgramHeaderOffset() const { return header_.e_phoff; }
Elf64_Off FileHeader::GetSectionHeaderOffset() const { return header_.e_shoff; }
Elf_Word FileHeader::GetFlags() const { return header_.e_flags; }
Elf_Half FileHeader::GetElfHeaderSize() const { return header_.e_ehsize; }
Elf_Half FileHeader::GetProgramHeaderEntrySize() const {
  return header_.e_phentsize;
}
Elf_Half FileHeader::GetProgramHeaderNum() const { return header_.e_phnum; }
Elf_Half FileHeader::GetSectionHeaderEntrySize() const {
  return header_.e_shentsize;
}
Elf_Half FileHeader::GetSectionHeaderNum() const { return header_.e_shnum; }
Elf_Half FileHeader::GetSectionHeaderStringTableIndex() const {
  return header_.e_shstrndx;
}

// SectionHeader

SectionHeader::SectionHeader(const elf_cpp_bindings::section::SectionHeader& header)
    : header_(header) {}

Elf_Word SectionHeader::GetName() const { return header_.sh_name; }

absl::StatusOr<SectionType> SectionHeader::GetType() const {
  auto result = header_.get_type();
  return MapResult<SectionType>(result);
}

Elf_Xword SectionHeader::GetFlags() const { return header_.sh_flags; }

Elf64_Off SectionHeader::GetOffset() const { return header_.sh_offset; }

Elf64_Addr SectionHeader::GetAddress() const { return header_.sh_addr; }

Elf_Xword SectionHeader::GetSize() const { return header_.sh_size; }

Elf_Word SectionHeader::GetLink() const { return header_.sh_link; }

Elf_Word SectionHeader::GetInfo() const { return header_.sh_info; }

Elf_Xword SectionHeader::GetAddressAlign() const {
  return header_.sh_addralign;
}

Elf_Xword SectionHeader::GetEntrySize() const { return header_.sh_entsize; }

// SectionHeaderTable

SectionHeaderTable::SectionHeaderTable(
    const elf_cpp_bindings::section::SectionHeaderTable& table)
    : table_(table) {}

size_t SectionHeaderTable::size() const { return table_.len(); }

bool SectionHeaderTable::empty() const { return table_.is_empty(); }

absl::StatusOr<SectionHeader> SectionHeaderTable::GetSectionHeader(
    size_t index) const {
  auto result = table_.get(index);
  return MapResult<SectionHeader>(result);
}

// SectionHeaderTableWithStrings

SectionHeaderTableWithStrings::SectionHeaderTableWithStrings(
    const elf_cpp_bindings::types::SectionHeaderTableWithStrings& table_with_strings)
    : section_header_table_(MapOption<SectionHeaderTable>(
          table_with_strings.section_header_table)),
      string_table_(MapOption<StringTable>(table_with_strings.string_table)) {}

const std::optional<SectionHeaderTable>&
SectionHeaderTableWithStrings::GetSectionHeaderTable() const {
  return section_header_table_;
}

const std::optional<StringTable>&
SectionHeaderTableWithStrings::GetStringTable() const {
  return string_table_;
}

// SectionData

SectionData::SectionData(std::string data,
                         std::optional<CompressionHeader> compression)
    : data_(std::move(data)), compression_(std::move(compression)) {}

absl::string_view SectionData::GetData() const { return data_; }
const std::optional<CompressionHeader>& SectionData::GetCompression() const {
  return compression_;
}

// CompressionHeader

CompressionHeader::CompressionHeader(
    const elf_cpp_bindings::compression::CompressionHeader& header)
    : header_(header) {}

Elf_Word CompressionHeader::GetType() const { return header_.ch_type; }

Elf_Xword CompressionHeader::GetSize() const { return header_.ch_size; }

Elf_Xword CompressionHeader::GetAddressAlign() const {
  return header_.ch_addralign;
}

// StringTable

StringTable::StringTable(const elf_cpp_bindings::string_table::StringTable& table)
    : table_(table) {}

absl::StatusOr<std::string> StringTable::Get(size_t index) const {
  auto result = table_.get(index);
  return MapResult(std::move(result));
}

// SegmentTable

SegmentTable::SegmentTable(const elf_cpp_bindings::segment::SegmentTable& table)
    : table_(table) {}

size_t SegmentTable::size() const { return table_.len(); }

bool SegmentTable::empty() const { return table_.is_empty(); }

absl::StatusOr<ProgramHeader> SegmentTable::GetProgramHeader(
    size_t index) const {
  auto result = table_.get(index);
  return MapResult<ProgramHeader>(result);
}

// ProgramHeader

ProgramHeader::ProgramHeader(const elf_cpp_bindings::segment::ProgramHeader& header)
    : header_(header) {}

absl::StatusOr<ProgramHeaderType> ProgramHeader::GetType() const {
  auto result = header_.get_type();
  return MapResult<ProgramHeaderType>(result);
}

Elf_Word ProgramHeader::GetFlags() const { return header_.p_flags; }

Elf64_Off ProgramHeader::GetOffset() const { return header_.p_offset; }

Elf64_Addr ProgramHeader::GetVirtualAddress() const { return header_.p_vaddr; }

Elf64_Addr ProgramHeader::GetPhysicalAddress() const { return header_.p_paddr; }

Elf_Xword ProgramHeader::GetFileSize() const { return header_.p_filesz; }

Elf_Xword ProgramHeader::GetMemorySize() const { return header_.p_memsz; }

Elf_Xword ProgramHeader::GetAlignment() const { return header_.p_align; }

// Symbol
Symbol::Symbol(const elf_cpp_bindings::symbol::Symbol& symbol) : symbol_(symbol) {}

Elf_Word Symbol::GetName() const { return symbol_.st_name; }

Elf_Half Symbol::GetSectionHeaderIndex() const { return symbol_.st_shndx; }

Elf64_Addr Symbol::GetValue() const { return symbol_.st_value; }

Elf_Xword Symbol::GetSize() const { return symbol_.st_size; }

bool Symbol::IsUndefined() const { return symbol_.is_undefined(); }

absl::StatusOr<elf_cpp_bindings::constants::SymbolType> Symbol::GetType() const {
  auto result = symbol_.st_symtype();
  return MapResult<elf_cpp_bindings::constants::SymbolType>(result);
}

absl::StatusOr<elf_cpp_bindings::constants::SymbolBind> Symbol::GetBinding() const {
  auto result = symbol_.st_bind();
  return MapResult<elf_cpp_bindings::constants::SymbolBind>(result);
}

absl::StatusOr<elf_cpp_bindings::constants::SymbolVisibility> Symbol::GetVisibility()
    const {
  auto result = symbol_.st_vis();
  return MapResult<elf_cpp_bindings::constants::SymbolVisibility>(result);
}

// SymbolTable

SymbolTable::SymbolTable(const elf_cpp_bindings::symbol::SymbolTable& table)
    : table_(table) {}

size_t SymbolTable::size() const { return table_.len(); }

bool SymbolTable::empty() const { return table_.is_empty(); }

absl::StatusOr<Symbol> SymbolTable::GetSymbol(size_t index) const {
  auto result = table_.get(index);
  return MapResult<Symbol>(result);
}

// SymbolTableWithStrings

SymbolTableWithStrings::SymbolTableWithStrings(
    const elf_cpp_bindings::types::SymbolTableWithStrings& table_with_strings)
    : symbol_table_(SymbolTable(table_with_strings.symbol_table)),
      string_table_(StringTable(table_with_strings.string_table)) {}

const SymbolTable& SymbolTableWithStrings::GetSymbolTable() const {
  return symbol_table_;
}

const StringTable& SymbolTableWithStrings::GetStringTable() const {
  return string_table_;
}

// SymbolVersionTable

SymbolVersionTable::SymbolVersionTable(
    elf_cpp_bindings::gnu_symver::SymbolVersionTable&& table)
    : table_(std::move(table)) {}

absl::StatusOr<std::optional<SymbolRequirement>>
SymbolVersionTable::GetRequirement(size_t sym_idx) const {
  auto result = table_.get_requirement(sym_idx);
  return MapResult<std::optional<SymbolRequirement>>(
      std::move(result), [](auto&& optional_req) {
        return MapOption<SymbolRequirement>(
            std::forward<decltype(optional_req)>(optional_req));
      });
}

absl::StatusOr<std::optional<SymbolDefinition>>
SymbolVersionTable::GetDefinition(size_t sym_idx) const {
  auto result = table_.get_definition(sym_idx);
  return MapResult<std::optional<SymbolDefinition>>(
      std::move(result), [](auto&& optional_def) {
        return MapOption<SymbolDefinition>(
            std::forward<decltype(optional_def)>(optional_def));
      });
}

// SymbolDefinition

SymbolDefinition::SymbolDefinition(
    elf_cpp_bindings::gnu_symver::SymbolDefinition def)
    : def_(std::move(def)) {}

Elf_Word SymbolDefinition::GetHash() const { return def_.hash; }

Elf_Half SymbolDefinition::GetFlags() const { return def_.flags; }

bool SymbolDefinition::IsHidden() const { return def_.hidden; }

size_t SymbolDefinition::GetNameCount() const { return def_.get_name_count(); }

absl::StatusOr<std::string> SymbolDefinition::GetName(size_t index) const {
  auto result = def_.get_name(index);
  return MapResult(std::move(result));
}

// SymbolRequirement

SymbolRequirement::SymbolRequirement(
    elf_cpp_bindings::gnu_symver::SymbolRequirement&& req)
    : req_(std::move(req)) {}

std::string_view SymbolRequirement::GetFile() const {
  return security::crubit_helpers::StringViewFromVecU8(req_.file);
}

std::string_view SymbolRequirement::GetName() const {
  return security::crubit_helpers::StringViewFromVecU8(req_.name);
}

Elf_Word SymbolRequirement::GetHash() const { return req_.hash; }

Elf_Half SymbolRequirement::GetFlags() const { return req_.flags; }

bool SymbolRequirement::IsHidden() const { return req_.hidden; }

// SymbolFindResult

SymbolFindResult::SymbolFindResult(
    elf_cpp_bindings::elf_bytes::SymbolFindResult&& result)
    : index_(result.index), symbol_(result.symbol) {}

size_t SymbolFindResult::GetIndex() const { return index_; }

const Symbol& SymbolFindResult::GetSymbol() const { return symbol_; }

// SysVHashTable

SysVHashTable::SysVHashTable(elf_cpp_bindings::hash::SysVHashTable&& table)
    : table_(std::move(table)) {}

absl::StatusOr<std::optional<SymbolFindResult>> SysVHashTable::Find(
    std::string_view name, const SymbolTable& symbol_table,
    const StringTable& string_table) const {
  auto result =
      table_.find_symbol(name, symbol_table.table_, string_table.table_);
  return MapResult<std::optional<SymbolFindResult>>(
      std::move(result), [](auto&& optional_find_result) {
        return MapOption<SymbolFindResult>(
            std::forward<decltype(optional_find_result)>(optional_find_result));
      });
}

// Dyn

Dyn::Dyn(const elf_cpp_bindings::dynamic::Dyn& dyn) : dyn_(dyn) {}

DynamicTag Dyn::GetTag() const { return dyn_.d_tag(); }

Elf_Xword Dyn::GetValue() const { return dyn_.d_val(); }

Elf64_Addr Dyn::GetPtr() const { return dyn_.d_ptr(); }

// DynTable

DynTable::DynTable(const elf_cpp_bindings::dynamic::DynTable& table)
    : table_(table) {}

size_t DynTable::size() const { return table_.len(); }

bool DynTable::empty() const { return table_.is_empty(); }

absl::StatusOr<Dyn> DynTable::GetDyn(size_t index) const {
  auto result = table_.get(index);
  return MapResult<Dyn>(result);
}

// Rel

Rel::Rel(const elf_cpp_bindings::relocation::Rel& rel) : rel_(rel) {}

Elf64_Addr Rel::GetOffset() const { return rel_.r_offset; }

Elf_Word Rel::GetSym() const { return rel_.r_sym; }

Elf_Word Rel::GetType() const { return rel_.r_type; }

// RelTable

RelTable::RelTable(const elf_cpp_bindings::relocation::RelTable& table)
    : table_(table) {}

size_t RelTable::size() const { return table_.len(); }

bool RelTable::empty() const { return table_.is_empty(); }

absl::StatusOr<Rel> RelTable::GetRel(size_t index) const {
  auto result = table_.get(index);
  return MapResult<Rel>(result);
}

// Rela

Rela::Rela(const elf_cpp_bindings::relocation::Rela& rela) : rela_(rela) {}

Elf64_Addr Rela::GetOffset() const { return rela_.r_offset; }

Elf_Word Rela::GetSym() const { return rela_.r_sym; }

Elf_Word Rela::GetType() const { return rela_.r_type; }

Elf_Sxword Rela::GetAddend() const { return rela_.r_addend; }

// RelaTable

RelaTable::RelaTable(const elf_cpp_bindings::relocation::RelaTable& table)
    : table_(table) {}

size_t RelaTable::size() const { return table_.len(); }

bool RelaTable::empty() const { return table_.is_empty(); }

absl::StatusOr<Rela> RelaTable::GetRela(size_t index) const {
  auto result = table_.get(index);
  return MapResult<Rela>(result);
}

// Note

Note::Note(const elf_cpp_bindings::note::Note& note) : note_(note) {}

elf_cpp_bindings::note::NoteType Note::GetType() const { return note_.get_type(); }

absl::StatusOr<NoteGnuAbiTag> Note::GetAsGnuAbiTag() const {
  auto result = note_.get_as_gnu_abi_tag();
  return MapResult<NoteGnuAbiTag>(result);
}

absl::StatusOr<NoteGnuBuildId> Note::GetAsGnuBuildId() const {
  auto result = note_.get_as_gnu_build_id();
  return MapResult<NoteGnuBuildId>(result);
}

absl::StatusOr<NoteAny> Note::GetAsUnknown() const {
  auto result = note_.get_as_unknown();
  return MapResult<NoteAny>(result);
}

// NoteAny

NoteAny::NoteAny(const elf_cpp_bindings::note::NoteAny& any) : any_(any) {}

Elf_Xword NoteAny::GetType() const { return any_.n_type; }

std::string_view NoteAny::GetName() const {
  return security::crubit_helpers::StringViewFromVecU8(any_.name);
}

std::string_view NoteAny::GetDesc() const {
  return security::crubit_helpers::StringViewFromVecU8(any_.desc);
}

// NoteTable

NoteTable::NoteTable(const elf_cpp_bindings::note::NoteTable& table)
    : table_(table) {}

size_t NoteTable::size() const { return table_.len(); }

bool NoteTable::empty() const { return table_.is_empty(); }

absl::StatusOr<Note> NoteTable::Get(size_t index) const {
  auto result = table_.get(index);
  return MapResult<Note>(result);
}

// NoteGnuAbiTag

NoteGnuAbiTag::NoteGnuAbiTag(const elf_cpp_bindings::note::NoteGnuAbiTag& tag)
    : tag_(tag) {}

Elf_Word NoteGnuAbiTag::GetOs() const { return tag_.os; }

Elf_Word NoteGnuAbiTag::GetMajor() const { return tag_.major; }

Elf_Word NoteGnuAbiTag::GetMinor() const { return tag_.minor; }

Elf_Word NoteGnuAbiTag::GetSubminor() const { return tag_.subminor; }

// NoteGnuBuildId

NoteGnuBuildId::NoteGnuBuildId(
    const elf_cpp_bindings::note::NoteGnuBuildId& build_id)
    : build_id_(build_id) {}

std::string_view NoteGnuBuildId::GetBuildId() const {
  return security::crubit_helpers::StringViewFromVecU8(build_id_.build_id);
}

// GnuHashTable

GnuHashTable::GnuHashTable(elf_cpp_bindings::hash::GnuHashTable&& table)
    : table_(std::move(table)) {}

absl::StatusOr<std::optional<SymbolFindResult>> GnuHashTable::Find(
    std::string_view name, const SymbolTable& symbol_table,
    const StringTable& string_table) const {
  auto result =
      table_.find_symbol(name, symbol_table.table_, string_table.table_);
  return MapResult<std::optional<SymbolFindResult>>(
      std::move(result), [](auto&& optional_find_result) {
        return MapOption<SymbolFindResult>(
            std::forward<decltype(optional_find_result)>(optional_find_result));
      });
}

// CommonElfData

CommonElfData::CommonElfData(std::optional<SymbolTable> symtab,
                             std::optional<StringTable> symtab_strs,
                             std::optional<SymbolTable> dynsyms,
                             std::optional<StringTable> dynsyms_strs,
                             std::optional<DynTable> dynamic,
                             std::optional<SysVHashTable> sysv_hash,
                             std::optional<GnuHashTable> gnu_hash)
    : symtab_(std::move(symtab)),
      symtab_strs_(std::move(symtab_strs)),
      dynsyms_(std::move(dynsyms)),
      dynsyms_strs_(std::move(dynsyms_strs)),
      dynamic_(std::move(dynamic)),
      sysv_hash_(std::move(sysv_hash)),
      gnu_hash_(std::move(gnu_hash)) {}

const std::optional<SymbolTable>& CommonElfData::GetSymbolTable() const {
  return symtab_;
}

const std::optional<StringTable>& CommonElfData::GetSymbolTableStrings() const {
  return symtab_strs_;
}

const std::optional<SymbolTable>& CommonElfData::GetDynamicSymbolTable() const {
  return dynsyms_;
}

const std::optional<StringTable>& CommonElfData::GetDynamicSymbolTableStrings()
    const {
  return dynsyms_strs_;
}

const std::optional<DynTable>& CommonElfData::GetDynamicTable() const {
  return dynamic_;
}

const std::optional<SysVHashTable>& CommonElfData::GetSysVHashTable() const {
  return sysv_hash_;
}

const std::optional<GnuHashTable>& CommonElfData::GetGnuHashTable() const {
  return gnu_hash_;
}

std::string DynamicTagToString(DynamicTag tag) {
  return std::string(elf_cpp_bindings::to_str::d_tag_to_str(tag));
}

std::string SectionTypeToString(SectionType type) {
  return std::string(elf_cpp_bindings::to_str::sh_type_to_str(type));
}

std::string MachineToString(ElfMachine machine) {
  return std::string(elf_cpp_bindings::to_str::e_machine_to_str(machine));
}

std::string MachineToHumanString(ElfMachine machine) {
  return std::string(elf_cpp_bindings::to_str::e_machine_to_human_str(machine));
}

std::string OsAbiToString(uint8_t osabi) {
  return std::string(elf_cpp_bindings::to_str::e_osabi_to_string(osabi).as_str());
}

std::string ProgramHeaderTypeToString(ProgramHeaderType type) {
  return std::string(elf_cpp_bindings::to_str::p_type_to_str(type));
}

std::string SymbolTypeToString(SymbolType type) {
  return std::string(elf_cpp_bindings::to_str::st_symtype_to_str(type));
}

std::string SymbolBindToString(SymbolBind bind) {
  return std::string(elf_cpp_bindings::to_str::st_bind_to_str(bind));
}

std::string SymbolVisibilityToString(SymbolVisibility visibility) {
  return std::string(elf_cpp_bindings::to_str::st_vis_to_str(visibility));
}

}  // namespace elf_bridge
