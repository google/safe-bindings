// Differential equivalence tests verifying that the memory-unsafe library
// (//third_party/elfio) and the memory-safe alternative
// (//security/ise_memory_safety/elf:elf_bridge) produce identical parsed
// outputs for ELF headers, section headers, compressed sections, string tables,
// named section lookups, section payloads, program segments, symbol tables
// (.symtab and .dynsym), hash table lookups (.gnu.hash and .hash), dynamic
// tags, relocations (SHT_REL and SHT_RELA), ELF notes (SHT_NOTE and PT_NOTE),
// symbol versioning, and streaming file reads under identical parameters.

#include <cstddef>
#include <cstring>
#include <ios>
#include <optional>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "file/base/helpers.h"
#include "file/base/options.h"
#include "file/base/path.h"
#include "elf_bridge.h"
#include "tests/payloads.h"
#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/container/flat_hash_map.h"
#include "absl/container/flat_hash_set.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/str_join.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "third_party/elfio/elfio/elf_types.hpp"
#include "third_party/elfio/elfio/elfio.hpp"
#include "third_party/elfio/elfio/elfio_dynamic.hpp"
#include "third_party/elfio/elfio/elfio_note.hpp"
#include "third_party/elfio/elfio/elfio_relocation.hpp"
#include "third_party/elfio/elfio/elfio_section.hpp"
#include "third_party/elfio/elfio/elfio_segment.hpp"
#include "third_party/elfio/elfio/elfio_strings.hpp"
#include "third_party/elfio/elfio/elfio_symbols.hpp"
#include "third_party/elfio/elfio/elfio_utils.hpp"
#include "third_party/elfio/elfio/elfio_versym.hpp"

namespace elfio_migration {
namespace {

using ::absl_testing::IsOkAndHolds;
using ::testing::Eq;

// Appends a formatted diagnostic to `mismatches` when `actual` and `expected`
// are not equal.
template <typename Actual, typename Expected>
void RecordIfNe(const Actual& actual, const Expected& expected,
                absl::string_view label, std::vector<std::string>& mismatches) {
  if (!(actual == expected)) {
    mismatches.push_back(absl::StrCat(label, ": got ",
                                      testing::PrintToString(actual), ", want ",
                                      testing::PrintToString(expected)));
  }
}

// Appends a formatted diagnostic to `mismatches` when `actual` is not OK or its
// value does not equal `expected`.
template <typename T, typename Expected>
void RecordIfNotOkAndHolds(const absl::StatusOr<T>& actual,
                           const Expected& expected, absl::string_view label,
                           std::vector<std::string>& mismatches) {
  if (!actual.ok()) {
    mismatches.push_back(absl::StrCat(label, ": unexpected status ",
                                      actual.status().ToString()));
    return;
  }
  RecordIfNe(*actual, expected, label, mismatches);
}

// Converts collected field-level mismatch descriptions into an `absl::Status`
// (`OkStatus` when `mismatches` is empty, `FailedPreconditionError` otherwise).
absl::Status FinishVerification(absl::Span<const std::string> mismatches) {
  if (mismatches.empty()) {
    return absl::OkStatus();
  }
  return absl::FailedPreconditionError(absl::StrJoin(mismatches, "; "));
}

// Verifies that all 17 ELF file header fields match between ELFIO and
// elf_bridge::FileHeader, returning `OkStatus()` on full parity or an error
// status detailing all mismatched fields.
absl::Status VerifyFileHeaderParity(const ELFIO::elfio& mu_reader,
                                    const elf_bridge::FileHeader& sa_hdr) {
  std::vector<std::string> mismatches;
  if (mu_reader.get_class() == ELFIO::ELFCLASS64) {
    RecordIfNe(sa_hdr.GetClass(), elf_bridge::ElfClass::kElf64,
               "e_ident[CLASS]", mismatches);
  } else {
    RecordIfNe(sa_hdr.GetClass(), elf_bridge::ElfClass::kElf32,
               "e_ident[CLASS]", mismatches);
  }

  if (mu_reader.get_encoding() == ELFIO::ELFDATA2LSB) {
    RecordIfNe(sa_hdr.GetEndianness(), elf_bridge::AnyEndian::kLittle,
               "e_ident[DATA]", mismatches);
  } else {
    RecordIfNe(sa_hdr.GetEndianness(), elf_bridge::AnyEndian::kBig,
               "e_ident[DATA]", mismatches);
  }

  RecordIfNe(sa_hdr.GetType(), mu_reader.get_type(), "e_type", mismatches);
  RecordIfNotOkAndHolds(
      sa_hdr.GetMachine(),
      static_cast<elf_bridge::ElfMachine>(mu_reader.get_machine()), "e_machine",
      mismatches);
  RecordIfNe(sa_hdr.GetVersion(), mu_reader.get_version(), "e_version",
             mismatches);
  RecordIfNe(sa_hdr.GetVersion(), mu_reader.get_elf_version(),
             "e_ident[VERSION]", mismatches);
  RecordIfNe(sa_hdr.GetEntry(), mu_reader.get_entry(), "e_entry", mismatches);
  RecordIfNe(sa_hdr.GetOsAbi(), mu_reader.get_os_abi(), "e_ident[OSABI]",
             mismatches);
  RecordIfNe(sa_hdr.GetAbiVersion(), mu_reader.get_abi_version(),
             "e_ident[ABIVERSION]", mismatches);
  RecordIfNe(sa_hdr.GetProgramHeaderOffset(), mu_reader.get_segments_offset(),
             "e_phoff", mismatches);
  RecordIfNe(sa_hdr.GetSectionHeaderOffset(), mu_reader.get_sections_offset(),
             "e_shoff", mismatches);
  RecordIfNe(sa_hdr.GetFlags(), mu_reader.get_flags(), "e_flags", mismatches);
  RecordIfNe(sa_hdr.GetElfHeaderSize(), mu_reader.get_header_size(), "e_ehsize",
             mismatches);
  RecordIfNe(sa_hdr.GetProgramHeaderEntrySize(),
             mu_reader.get_segment_entry_size(), "e_phentsize", mismatches);
  RecordIfNe(sa_hdr.GetProgramHeaderNum(), mu_reader.segments.size(), "e_phnum",
             mismatches);
  RecordIfNe(sa_hdr.GetSectionHeaderEntrySize(),
             mu_reader.get_section_entry_size(), "e_shentsize", mismatches);
  RecordIfNe(sa_hdr.GetSectionHeaderNum(), mu_reader.sections.size(), "e_shnum",
             mismatches);
  RecordIfNe(sa_hdr.GetSectionHeaderStringTableIndex(),
             mu_reader.get_section_name_str_index(), "e_shstrndx", mismatches);
  return FinishVerification(mismatches);
}

// Verifies that section header metadata fields match between ELFIO::section and
// elf_bridge::SectionHeader, returning `OkStatus()` on full parity or an error
// status detailing any mismatches.
absl::Status VerifySectionHeaderFieldsParity(
    const ELFIO::section& mu_sec, const elf_bridge::SectionHeader& sa_sec,
    absl::string_view context_label) {
  std::vector<std::string> mismatches;
  RecordIfNotOkAndHolds(sa_sec.GetType(),
                        static_cast<elf_bridge::SectionType>(mu_sec.get_type()),
                        absl::StrCat(context_label, " sh_type"), mismatches);
  RecordIfNe(sa_sec.GetFlags(), mu_sec.get_flags(),
             absl::StrCat(context_label, " sh_flags"), mismatches);
  RecordIfNe(sa_sec.GetAddress(), mu_sec.get_address(),
             absl::StrCat(context_label, " sh_addr"), mismatches);
  RecordIfNe(sa_sec.GetOffset(), mu_sec.get_offset(),
             absl::StrCat(context_label, " sh_offset"), mismatches);
  RecordIfNe(sa_sec.GetSize(), mu_sec.get_size(),
             absl::StrCat(context_label, " sh_size"), mismatches);
  RecordIfNe(sa_sec.GetLink(), mu_sec.get_link(),
             absl::StrCat(context_label, " sh_link"), mismatches);
  RecordIfNe(sa_sec.GetInfo(), mu_sec.get_info(),
             absl::StrCat(context_label, " sh_info"), mismatches);
  RecordIfNe(sa_sec.GetAddressAlign(), mu_sec.get_addr_align(),
             absl::StrCat(context_label, " sh_addralign"), mismatches);
  RecordIfNe(sa_sec.GetEntrySize(), mu_sec.get_entry_size(),
             absl::StrCat(context_label, " sh_entsize"), mismatches);
  return FinishVerification(mismatches);
}

// Verifies that program header metadata fields match between ELFIO::segment and
// elf_bridge::ProgramHeader, returning `OkStatus()` on full parity or an error
// status detailing any mismatches.
absl::Status VerifyProgramHeaderFieldsParity(
    const ELFIO::segment& mu_seg, const elf_bridge::ProgramHeader& sa_phdr,
    size_t segment_index) {
  std::vector<std::string> mismatches;
  std::string prefix = absl::StrCat("Segment index ", segment_index);
  RecordIfNotOkAndHolds(
      sa_phdr.GetType(),
      static_cast<elf_bridge::ProgramHeaderType>(mu_seg.get_type()),
      absl::StrCat(prefix, " p_type"), mismatches);
  RecordIfNe(sa_phdr.GetFlags(), mu_seg.get_flags(),
             absl::StrCat(prefix, " p_flags"), mismatches);
  RecordIfNe(sa_phdr.GetOffset(), mu_seg.get_offset(),
             absl::StrCat(prefix, " p_offset"), mismatches);
  RecordIfNe(sa_phdr.GetVirtualAddress(), mu_seg.get_virtual_address(),
             absl::StrCat(prefix, " p_vaddr"), mismatches);
  RecordIfNe(sa_phdr.GetPhysicalAddress(), mu_seg.get_physical_address(),
             absl::StrCat(prefix, " p_paddr"), mismatches);
  RecordIfNe(sa_phdr.GetFileSize(), mu_seg.get_file_size(),
             absl::StrCat(prefix, " p_filesz"), mismatches);
  RecordIfNe(sa_phdr.GetMemorySize(), mu_seg.get_memory_size(),
             absl::StrCat(prefix, " p_memsz"), mismatches);
  RecordIfNe(sa_phdr.GetAlignment(), mu_seg.get_align(),
             absl::StrCat(prefix, " p_align"), mismatches);
  return FinishVerification(mismatches);
}

// Verifies section payload parity, handling both uncompressed sections and
// SHF_COMPRESSED sections (where ELFIO without a compression plugin retains the
// raw Elf32_Chdr/Elf64_Chdr prefix while elf_bridge parses it into
// SectionData::GetCompression()). Returns `OkStatus()` on parity or an error
// status describing the mismatch.
absl::Status VerifySectionDataParity(const ELFIO::elfio& mu_reader,
                                     const ELFIO::section& mu_sec,
                                     const elf_bridge::SectionData& sa_data,
                                     size_t section_index,
                                     absl::string_view section_name) {
  std::vector<std::string> mismatches;
  std::string prefix =
      absl::StrCat("Section index ", section_index, " (", section_name, ")");
  if (mu_sec.get_type() == ELFIO::SHT_NOBITS || mu_sec.get_size() == 0) {
    RecordIfNe(sa_data.GetCompression().has_value(), false,
               absl::StrCat(prefix, " has_compression"), mismatches);
    RecordIfNe(sa_data.GetData().empty(), true,
               absl::StrCat(prefix, " data_empty"), mismatches);
    return FinishVerification(mismatches);
  }

  if (mu_sec.get_data() == nullptr) {
    return absl::FailedPreconditionError(
        absl::StrCat(prefix, ": ELFIO section data is null"));
  }
  absl::string_view mu_data(mu_sec.get_data(), mu_sec.get_size());
  if ((mu_sec.get_flags() & ELFIO::SHF_COMPRESSED) == 0) {
    RecordIfNe(sa_data.GetCompression().has_value(), false,
               absl::StrCat(prefix, " has_compression"), mismatches);
    RecordIfNe(sa_data.GetData(), mu_data, absl::StrCat(prefix, " payload"),
               mismatches);
    return FinishVerification(mismatches);
  }

  if (!sa_data.GetCompression().has_value()) {
    return absl::FailedPreconditionError(
        absl::StrCat("Expected compression header at ", prefix));
  }
  const elf_bridge::CompressionHeader& sa_chdr = *sa_data.GetCompression();
  const ELFIO::endianess_convertor& convertor = mu_reader.get_convertor();

  if (mu_reader.get_class() == ELFIO::ELFCLASS64) {
    if (mu_data.size() < sizeof(ELFIO::Elf64_Chdr)) {
      return absl::FailedPreconditionError(
          absl::StrCat("Compressed section too small at ", prefix));
    }
    ELFIO::Elf64_Chdr raw_chdr;
    std::memcpy(&raw_chdr, mu_data.data(), sizeof(raw_chdr));
    RecordIfNe(sa_chdr.GetType(), convertor(raw_chdr.ch_type),
               absl::StrCat(prefix, " ch_type"), mismatches);
    RecordIfNe(sa_chdr.GetSize(), convertor(raw_chdr.ch_size),
               absl::StrCat(prefix, " ch_size"), mismatches);
    RecordIfNe(sa_chdr.GetAddressAlign(), convertor(raw_chdr.ch_addralign),
               absl::StrCat(prefix, " ch_addralign"), mismatches);
    RecordIfNe(sa_data.GetData(), mu_data.substr(sizeof(ELFIO::Elf64_Chdr)),
               absl::StrCat(prefix, " compressed_payload"), mismatches);
  } else {
    if (mu_data.size() < sizeof(ELFIO::Elf32_Chdr)) {
      return absl::FailedPreconditionError(
          absl::StrCat("Compressed section too small at ", prefix));
    }
    ELFIO::Elf32_Chdr raw_chdr;
    std::memcpy(&raw_chdr, mu_data.data(), sizeof(raw_chdr));
    RecordIfNe(sa_chdr.GetType(), convertor(raw_chdr.ch_type),
               absl::StrCat(prefix, " ch_type"), mismatches);
    RecordIfNe(sa_chdr.GetSize(), convertor(raw_chdr.ch_size),
               absl::StrCat(prefix, " ch_size"), mismatches);
    RecordIfNe(sa_chdr.GetAddressAlign(), convertor(raw_chdr.ch_addralign),
               absl::StrCat(prefix, " ch_addralign"), mismatches);
    RecordIfNe(sa_data.GetData(), mu_data.substr(sizeof(ELFIO::Elf32_Chdr)),
               absl::StrCat(prefix, " compressed_payload"), mismatches);
  }
  return FinishVerification(mismatches);
}

// Verifies that every NUL-terminated string in an uncompressed SHT_STRTAB
// section matches between ELFIO::const_string_section_accessor and
// elf_bridge::StringTable, returning `OkStatus()` on parity or an error status.
absl::Status VerifyStringTableParity(const ELFIO::section& mu_sec,
                                     const elf_bridge::StringTable& sa_strtab) {
  std::vector<std::string> mismatches;
  ELFIO::const_string_section_accessor mu_strtab(&mu_sec);
  size_t offset = 0;
  while (offset < mu_sec.get_size()) {
    absl::StatusOr<std::string> sa_str = sa_strtab.Get(offset);
    if (!sa_str.ok()) {
      return absl::FailedPreconditionError(
          absl::StrCat("Section ", mu_sec.get_name(), " offset ", offset, ": ",
                       sa_str.status().ToString()));
    }
    const char* mu_str =
        mu_strtab.get_string(static_cast<ELFIO::Elf_Word>(offset));
    if (mu_str == nullptr) {
      return absl::FailedPreconditionError(absl::StrCat(
          "Section ", mu_sec.get_name(), " offset ", offset, ": null string"));
    }
    RecordIfNe(*sa_str, absl::string_view(mu_str),
               absl::StrCat("Section ", mu_sec.get_name(), " offset ", offset),
               mismatches);
    offset += sa_str->size() + 1;
  }
  return FinishVerification(mismatches);
}

// Verifies that every entry in a symbol table and its associated string table
// matches between ELFIO::const_symbol_section_accessor and elf_bridge,
// returning `OkStatus()` on parity or an error status detailing mismatches.
absl::Status VerifySymbolTableAndStringsParity(
    const ELFIO::elfio& mu_reader, const ELFIO::section& mu_symtab_sec,
    const elf_bridge::SymbolTable& sa_syms,
    const elf_bridge::StringTable& sa_strs, absl::string_view table_name) {
  ELFIO::const_symbol_section_accessor mu_syms(mu_reader, &mu_symtab_sec);

  if (sa_syms.size() != mu_syms.get_symbols_num()) {
    return absl::FailedPreconditionError(
        absl::StrCat(table_name, " size mismatch: ", sa_syms.size(), " vs ",
                     mu_syms.get_symbols_num()));
  }
  std::vector<std::string> mismatches;
  for (size_t i = 0; i < sa_syms.size(); ++i) {
    std::string prefix = absl::StrCat(table_name, " index ", i);
    std::string mu_name;
    ELFIO::Elf64_Addr mu_value = 0;
    ELFIO::Elf_Xword mu_size = 0;
    unsigned char mu_bind = 0;
    unsigned char mu_type = 0;
    ELFIO::Elf_Half mu_shndx = 0;
    unsigned char mu_other = 0;
    if (!mu_syms.get_symbol(i, mu_name, mu_value, mu_size, mu_bind, mu_type,
                            mu_shndx, mu_other)) {
      mismatches.push_back(absl::StrCat(prefix, ": ELFIO get_symbol failed"));
      continue;
    }

    absl::StatusOr<elf_bridge::Symbol> sa_sym = sa_syms.GetSymbol(i);
    if (!sa_sym.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_sym.status().ToString()));
      continue;
    }

    RecordIfNotOkAndHolds(sa_strs.Get(sa_sym->GetName()), mu_name,
                          absl::StrCat(prefix, " st_name"), mismatches);
    RecordIfNe(sa_sym->GetValue(), mu_value, absl::StrCat(prefix, " st_value"),
               mismatches);
    RecordIfNe(sa_sym->GetSize(), mu_size, absl::StrCat(prefix, " st_size"),
               mismatches);
    RecordIfNotOkAndHolds(sa_sym->GetType(),
                          static_cast<elf_bridge::SymbolType>(mu_type),
                          absl::StrCat(prefix, " st_type"), mismatches);
    RecordIfNotOkAndHolds(sa_sym->GetBinding(),
                          static_cast<elf_bridge::SymbolBind>(mu_bind),
                          absl::StrCat(prefix, " st_bind"), mismatches);
    RecordIfNotOkAndHolds(
        sa_sym->GetVisibility(),
        static_cast<elf_bridge::SymbolVisibility>(ELF_ST_VISIBILITY(mu_other)),
        absl::StrCat(prefix, " st_visibility"), mismatches);
    RecordIfNe(sa_sym->GetSectionHeaderIndex(), mu_shndx,
               absl::StrCat(prefix, " st_shndx"), mismatches);
    RecordIfNe(sa_sym->IsUndefined(), mu_shndx == ELFIO::SHN_UNDEF,
               absl::StrCat(prefix, " is_undefined"), mismatches);
  }
  return FinishVerification(mismatches);
}

// Verifies optional SymbolTableWithStrings parity against ELFIO, returning
// `OkStatus()` on parity or an error status describing any mismatch.
absl::Status VerifySymbolTableParity(
    const ELFIO::elfio& mu_reader, const ELFIO::section* mu_symtab_sec,
    const std::optional<elf_bridge::SymbolTableWithStrings>& sa_symtab_opt,
    absl::string_view table_name) {
  if (sa_symtab_opt.has_value() != (mu_symtab_sec != nullptr)) {
    return absl::FailedPreconditionError(absl::StrCat(
        table_name, " presence mismatch: sa=", sa_symtab_opt.has_value(),
        ", mu=", mu_symtab_sec != nullptr));
  }
  if (mu_symtab_sec == nullptr || !sa_symtab_opt.has_value()) {
    return absl::OkStatus();
  }
  return VerifySymbolTableAndStringsParity(
      mu_reader, *mu_symtab_sec, sa_symtab_opt->GetSymbolTable(),
      sa_symtab_opt->GetStringTable(), table_name);
}

// Verifies that all dynamic section (.dynamic) entries match between
// ELFIO::const_dynamic_section_accessor and elf_bridge::DynTable, returning
// `OkStatus()` on parity or an error status detailing mismatches.
absl::Status VerifyDynamicTableParity(
    const ELFIO::elfio& mu_reader, const ELFIO::section* mu_dyn_sec,
    const std::optional<elf_bridge::DynTable>& sa_dyn_opt,
    const std::optional<elf_bridge::StringTable>& sa_dynstr_opt) {
  if (sa_dyn_opt.has_value() != (mu_dyn_sec != nullptr)) {
    return absl::FailedPreconditionError(
        absl::StrCat(".dynamic presence mismatch: sa=", sa_dyn_opt.has_value(),
                     ", mu=", mu_dyn_sec != nullptr));
  }
  if (mu_dyn_sec == nullptr || !sa_dyn_opt.has_value()) {
    return absl::OkStatus();
  }

  ELFIO::const_dynamic_section_accessor mu_dyn(mu_reader, mu_dyn_sec);
  // ELFIO stops counting at the first DT_NULL entry, whereas elf_bridge
  // exposes all raw section entries (where any trailing padding entries are
  // also DT_NULL).
  if (sa_dyn_opt->size() < mu_dyn.get_entries_num()) {
    return absl::FailedPreconditionError(absl::StrCat(
        "Expected sa_dyn_opt->size() (", sa_dyn_opt->size(),
        ") >= mu_dyn.get_entries_num() (", mu_dyn.get_entries_num(), ")"));
  }

  std::vector<std::string> mismatches;
  for (size_t i = 0; i < mu_dyn.get_entries_num(); ++i) {
    std::string prefix = absl::StrCat("Dynamic entry ", i);
    ELFIO::Elf_Xword mu_tag = 0;
    ELFIO::Elf_Xword mu_val = 0;
    std::string mu_str;
    if (!mu_dyn.get_entry(i, mu_tag, mu_val, mu_str)) {
      mismatches.push_back(absl::StrCat(prefix, ": ELFIO get_entry failed"));
      continue;
    }
    absl::StatusOr<elf_bridge::Dyn> sa_entry = sa_dyn_opt->GetDyn(i);
    if (!sa_entry.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_entry.status().ToString()));
      continue;
    }

    RecordIfNe(static_cast<ELFIO::Elf_Xword>(sa_entry->GetTag()), mu_tag,
               absl::StrCat(prefix, " d_tag"), mismatches);
    RecordIfNe(sa_entry->GetValue(), mu_val, absl::StrCat(prefix, " d_val"),
               mismatches);
    RecordIfNe(sa_entry->GetPtr(), mu_val, absl::StrCat(prefix, " d_ptr"),
               mismatches);
    if (!mu_str.empty()) {
      if (!sa_dynstr_opt.has_value()) {
        mismatches.push_back(absl::StrCat("Expected .dynstr for ", prefix));
        continue;
      }
      RecordIfNotOkAndHolds(sa_dynstr_opt->Get(sa_entry->GetValue()), mu_str,
                            absl::StrCat(prefix, " dynstr"), mismatches);
    }
  }
  for (size_t i = mu_dyn.get_entries_num(); i < sa_dyn_opt->size(); ++i) {
    std::string prefix = absl::StrCat("Trailing dynamic entry ", i);
    absl::StatusOr<elf_bridge::Dyn> sa_entry = sa_dyn_opt->GetDyn(i);
    if (!sa_entry.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_entry.status().ToString()));
      continue;
    }
    RecordIfNe(sa_entry->GetTag(), elf_bridge::DynamicTag::kNull,
               absl::StrCat(prefix, " d_tag"), mismatches);
  }
  return FinishVerification(mismatches);
}

// Verifies that all entries in an SHT_REL section match between ELFIO and
// elf_bridge::RelTable, returning `OkStatus()` on parity or an error status.
absl::Status VerifyRelTableParity(const ELFIO::elfio& mu_reader,
                                  const ELFIO::section& mu_sec,
                                  const elf_bridge::RelTable& sa_rels) {
  ELFIO::const_relocation_section_accessor mu_rels(mu_reader, &mu_sec);
  if (sa_rels.size() != mu_rels.get_entries_num()) {
    return absl::FailedPreconditionError(absl::StrCat(
        "Section ", mu_sec.get_name(), " size mismatch: ", sa_rels.size(),
        " vs ", mu_rels.get_entries_num()));
  }

  std::vector<std::string> mismatches;
  for (size_t j = 0; j < sa_rels.size(); ++j) {
    std::string prefix =
        absl::StrCat("Section ", mu_sec.get_name(), " entry ", j);
    ELFIO::Elf64_Addr mu_offset = 0;
    ELFIO::Elf_Word mu_sym = 0;
    unsigned mu_rtype = 0;
    ELFIO::Elf_Sxword mu_addend = 0;
    if (!mu_rels.get_entry(j, mu_offset, mu_sym, mu_rtype, mu_addend)) {
      mismatches.push_back(absl::StrCat(prefix, ": ELFIO get_entry failed"));
      continue;
    }
    absl::StatusOr<elf_bridge::Rel> sa_rel = sa_rels.GetRel(j);
    if (!sa_rel.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_rel.status().ToString()));
      continue;
    }
    RecordIfNe(sa_rel->GetOffset(), mu_offset,
               absl::StrCat(prefix, " r_offset"), mismatches);
    RecordIfNe(sa_rel->GetSym(), mu_sym, absl::StrCat(prefix, " r_sym"),
               mismatches);
    RecordIfNe(sa_rel->GetType(), mu_rtype, absl::StrCat(prefix, " r_type"),
               mismatches);
  }
  return FinishVerification(mismatches);
}

// Verifies that all entries in an SHT_RELA section match between ELFIO and
// elf_bridge::RelaTable, returning `OkStatus()` on parity or an error status.
absl::Status VerifyRelaTableParity(const ELFIO::elfio& mu_reader,
                                   const ELFIO::section& mu_sec,
                                   const elf_bridge::RelaTable& sa_relas) {
  ELFIO::const_relocation_section_accessor mu_relas(mu_reader, &mu_sec);
  if (sa_relas.size() != mu_relas.get_entries_num()) {
    return absl::FailedPreconditionError(absl::StrCat(
        "Section ", mu_sec.get_name(), " size mismatch: ", sa_relas.size(),
        " vs ", mu_relas.get_entries_num()));
  }

  std::vector<std::string> mismatches;
  for (size_t j = 0; j < sa_relas.size(); ++j) {
    std::string prefix =
        absl::StrCat("Section ", mu_sec.get_name(), " entry ", j);
    ELFIO::Elf64_Addr mu_offset = 0;
    ELFIO::Elf_Word mu_sym = 0;
    unsigned mu_rtype = 0;
    ELFIO::Elf_Sxword mu_addend = 0;
    if (!mu_relas.get_entry(j, mu_offset, mu_sym, mu_rtype, mu_addend)) {
      mismatches.push_back(absl::StrCat(prefix, ": ELFIO get_entry failed"));
      continue;
    }
    absl::StatusOr<elf_bridge::Rela> sa_rela = sa_relas.GetRela(j);
    if (!sa_rela.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_rela.status().ToString()));
      continue;
    }
    RecordIfNe(sa_rela->GetOffset(), mu_offset,
               absl::StrCat(prefix, " r_offset"), mismatches);
    RecordIfNe(sa_rela->GetSym(), mu_sym, absl::StrCat(prefix, " r_sym"),
               mismatches);
    RecordIfNe(sa_rela->GetType(), mu_rtype, absl::StrCat(prefix, " r_type"),
               mismatches);
    RecordIfNe(sa_rela->GetAddend(), mu_addend,
               absl::StrCat(prefix, " r_addend"), mismatches);
  }
  return FinishVerification(mismatches);
}

// Verifies that parsed ELF notes in a NoteTable match ELFIO's note accessor,
// returning `OkStatus()` on parity or an error status detailing mismatches.
template <typename MuNoteAccessor>
absl::Status VerifyNoteTableParity(const ELFIO::elfio& mu_reader,
                                   const MuNoteAccessor& mu_notes,
                                   const elf_bridge::NoteTable& sa_notes,
                                   absl::string_view context_label) {
  if (sa_notes.size() != mu_notes.get_notes_num()) {
    return absl::FailedPreconditionError(
        absl::StrCat(context_label, " size mismatch: ", sa_notes.size(), " vs ",
                     mu_notes.get_notes_num()));
  }
  const ELFIO::endianess_convertor& convertor = mu_reader.get_convertor();

  std::vector<std::string> mismatches;
  for (size_t i = 0; i < sa_notes.size(); ++i) {
    std::string prefix = absl::StrCat(context_label, " note ", i);
    ELFIO::Elf_Word mu_type = 0;
    std::string mu_name;
    char* mu_desc = nullptr;
    ELFIO::Elf_Word mu_desc_size = 0;
    if (!mu_notes.get_note(static_cast<ELFIO::Elf_Word>(i), mu_type, mu_name,
                           mu_desc, mu_desc_size)) {
      mismatches.push_back(absl::StrCat(prefix, ": ELFIO get_note failed"));
      continue;
    }
    absl::string_view mu_desc_view(
        mu_desc != nullptr ? mu_desc : "",
        mu_desc != nullptr ? static_cast<size_t>(mu_desc_size) : 0);

    absl::StatusOr<elf_bridge::Note> sa_note = sa_notes.Get(i);
    if (!sa_note.ok()) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ", sa_note.status().ToString()));
      continue;
    }
    switch (sa_note->GetType()) {
      case elf_bridge::NoteType::kGnuAbiTag: {
        RecordIfNe(mu_name, absl::string_view("GNU"),
                   absl::StrCat(prefix, " name"), mismatches);
        RecordIfNe(mu_type, static_cast<ELFIO::Elf_Word>(ELFIO::NT_GNU_ABI_TAG),
                   absl::StrCat(prefix, " type"), mismatches);
        if (mu_desc_view.size() != 4 * sizeof(ELFIO::Elf_Word)) {
          mismatches.push_back(absl::StrCat(
              prefix, " unexpected descriptor size: ", mu_desc_view.size()));
          break;
        }
        ELFIO::Elf_Word words[4];
        std::memcpy(words, mu_desc_view.data(), sizeof(words));
        absl::StatusOr<elf_bridge::NoteGnuAbiTag> sa_abi =
            sa_note->GetAsGnuAbiTag();
        if (!sa_abi.ok()) {
          mismatches.push_back(
              absl::StrCat(prefix, ": ", sa_abi.status().ToString()));
          break;
        }
        RecordIfNe(sa_abi->GetOs(), convertor(words[0]),
                   absl::StrCat(prefix, " os"), mismatches);
        RecordIfNe(sa_abi->GetMajor(), convertor(words[1]),
                   absl::StrCat(prefix, " major"), mismatches);
        RecordIfNe(sa_abi->GetMinor(), convertor(words[2]),
                   absl::StrCat(prefix, " minor"), mismatches);
        RecordIfNe(sa_abi->GetSubminor(), convertor(words[3]),
                   absl::StrCat(prefix, " subminor"), mismatches);
        break;
      }
      case elf_bridge::NoteType::kGnuBuildId: {
        RecordIfNe(mu_name, absl::string_view("GNU"),
                   absl::StrCat(prefix, " name"), mismatches);
        RecordIfNe(mu_type,
                   static_cast<ELFIO::Elf_Word>(ELFIO::NT_GNU_BUILD_ID),
                   absl::StrCat(prefix, " type"), mismatches);
        absl::StatusOr<elf_bridge::NoteGnuBuildId> sa_build_id =
            sa_note->GetAsGnuBuildId();
        if (!sa_build_id.ok()) {
          mismatches.push_back(
              absl::StrCat(prefix, ": ", sa_build_id.status().ToString()));
          break;
        }
        RecordIfNe(sa_build_id->GetBuildId(), mu_desc_view,
                   absl::StrCat(prefix, " build_id"), mismatches);
        break;
      }
      case elf_bridge::NoteType::kUnknown: {
        absl::StatusOr<elf_bridge::NoteAny> sa_any = sa_note->GetAsUnknown();
        if (!sa_any.ok()) {
          mismatches.push_back(
              absl::StrCat(prefix, ": ", sa_any.status().ToString()));
          break;
        }
        // ELF note names on disk include a trailing NUL byte in `namesz`;
        // ELFIO strips the trailing NUL in get_note(), whereas elf_bridge
        // returns the raw `namesz` slice.
        absl::string_view sa_name = sa_any->GetName();
        if (!sa_name.empty() && sa_name.back() == '\0') {
          sa_name.remove_suffix(1);
        }
        RecordIfNe(sa_any->GetType(), mu_type, absl::StrCat(prefix, " type"),
                   mismatches);
        RecordIfNe(sa_name, absl::string_view(mu_name),
                   absl::StrCat(prefix, " name"), mismatches);
        RecordIfNe(sa_any->GetDesc(), mu_desc_view,
                   absl::StrCat(prefix, " desc"), mismatches);
        break;
      }
    }
  }
  return FinishVerification(mismatches);
}

struct ExpectedVerneedAux {
  std::string file_name;
  std::string dep_name;
  ELFIO::Elf_Word hash = 0;
  ELFIO::Elf_Half flags = 0;
};

// Verifies that symbol versioning tables (.gnu.version, .gnu.version_r, and
// .gnu.version_d) are consistent between ELFIO and
// elf_bridge::SymbolVersionTable. Because ELFIO does not provide an accessor
// for .gnu.version_d (SHT_GNU_verdef), SymbolDefinition entries from
// elf_bridge are verified for IsHidden() parity against .gnu.version and
// self-consistency of GetHash() against ELFIO::elf_hash. Returns `OkStatus()`
// on parity or an error status detailing mismatches.
absl::Status VerifySymbolVersionTableParity(
    const ELFIO::elfio& mu_reader,
    const std::optional<elf_bridge::SymbolTableWithStrings>& sa_dynsym_opt,
    const std::optional<elf_bridge::SymbolVersionTable>& sa_svt_opt,
    const ElfPayload& payload) {
  std::vector<std::string> mismatches;
  const ELFIO::section* mu_versym_sec = mu_reader.sections[".gnu.version"];
  RecordIfNe(sa_svt_opt.has_value(), mu_versym_sec != nullptr,
             absl::StrCat(payload.name, " .gnu.version presence"), mismatches);
  if (payload.expect_symbol_versioning) {
    RecordIfNe(mu_versym_sec != nullptr, true,
               absl::StrCat(payload.name, " expected ELFIO .gnu.version"),
               mismatches);
    RecordIfNe(sa_svt_opt.has_value(), true,
               absl::StrCat(payload.name, " expected elf_bridge SVT"),
               mismatches);
  }
  if (mu_versym_sec == nullptr || !sa_svt_opt.has_value()) {
    return FinishVerification(mismatches);
  }

  ELFIO::const_versym_section_accessor mu_versym(mu_versym_sec);
  const ELFIO::endianess_convertor& convertor = mu_reader.get_convertor();
  if (!sa_dynsym_opt.has_value()) {
    return absl::FailedPreconditionError(
        absl::StrCat("Missing .dynsym for payload: ", payload.name));
  }
  if (sa_dynsym_opt->GetSymbolTable().size() != mu_versym.get_entries_num()) {
    return absl::FailedPreconditionError(absl::StrCat(
        payload.name,
        " versym size mismatch: ", sa_dynsym_opt->GetSymbolTable().size(),
        " vs ", mu_versym.get_entries_num()));
  }

  // Map version requirement indices (vna_other & 0x7fff) from ELFIO's
  // .gnu.version_r accessor.
  absl::flat_hash_map<ELFIO::Elf_Half, ExpectedVerneedAux> mu_verneed_by_index;
  absl::flat_hash_set<std::string> mu_verneed_keys;
  if (const ELFIO::section* mu_verneed_sec =
          mu_reader.sections[".gnu.version_r"];
      mu_verneed_sec != nullptr) {
    ELFIO::const_versym_r_section_accessor mu_verneed(mu_reader,
                                                      mu_verneed_sec);
    if (mu_verneed.get_entries_num() == 0U) {
      mismatches.push_back(absl::StrCat(
          payload.name, ": expected positive .gnu.version_r entry count"));
    }
    for (ELFIO::Elf_Word i = 0; i < mu_verneed.get_entries_num(); ++i) {
      ELFIO::Elf_Half version = 0;
      std::string file_name;
      ELFIO::Elf_Word hash = 0;
      ELFIO::Elf_Half flags = 0;
      ELFIO::Elf_Half other = 0;
      std::string dep_name;
      if (!mu_verneed.get_entry(i, version, file_name, hash, flags, other,
                                dep_name)) {
        mismatches.push_back(absl::StrCat(payload.name, " verneed entry ", i,
                                          ": ELFIO get_entry failed"));
        continue;
      }
      mu_verneed_by_index[other & 0x7fffU] = ExpectedVerneedAux{
          .file_name = file_name,
          .dep_name = dep_name,
          .hash = hash,
          .flags = flags,
      };
      mu_verneed_keys.insert(
          absl::StrCat(file_name, ":", dep_name, ":", hash, ":", flags));
    }
  }

  absl::flat_hash_set<std::string> sa_req_keys;
  for (size_t i = 0; i < sa_dynsym_opt->GetSymbolTable().size(); ++i) {
    std::string prefix = absl::StrCat("Symbol index ", i);
    ELFIO::Elf_Half raw_ver = 0;
    if (!mu_versym.get_entry(static_cast<ELFIO::Elf_Word>(i), raw_ver)) {
      mismatches.push_back(
          absl::StrCat(prefix, ": ELFIO versym get_entry failed"));
      continue;
    }
    ELFIO::Elf_Half ver_idx = convertor(raw_ver);
    bool mu_is_hidden = (ver_idx & 0x8000U) != 0;
    ELFIO::Elf_Half mu_ver_num = ver_idx & 0x7fffU;

    absl::StatusOr<std::optional<elf_bridge::SymbolRequirement>> sa_req =
        sa_svt_opt->GetRequirement(i);
    absl::StatusOr<std::optional<elf_bridge::SymbolDefinition>> sa_def =
        sa_svt_opt->GetDefinition(i);
    if (!sa_req.ok() || !sa_def.ok()) {
      mismatches.push_back(absl::StrCat(prefix,
                                        ": req=", sa_req.status().ToString(),
                                        " def=", sa_def.status().ToString()));
      continue;
    }

    if (mu_ver_num <= 1) {
      RecordIfNe(sa_req->has_value(), false,
                 absl::StrCat(prefix, " has_requirement"), mismatches);
      if (mu_ver_num == 0) {
        RecordIfNe(sa_def->has_value(), false,
                   absl::StrCat(prefix, " has_definition"), mismatches);
      }
      continue;
    }

    RecordIfNe(sa_req->has_value() || sa_def->has_value(), true,
               absl::StrCat(prefix, " version ", mu_ver_num, " has_req_or_def"),
               mismatches);
    if (sa_req->has_value()) {
      RecordIfNe((*sa_req)->IsHidden(), mu_is_hidden,
                 absl::StrCat(prefix, " req.is_hidden"), mismatches);
      RecordIfNe((*sa_req)->GetFile().empty(), false,
                 absl::StrCat(prefix, " req.file_empty"), mismatches);
      RecordIfNe((*sa_req)->GetName().empty(), false,
                 absl::StrCat(prefix, " req.name_empty"), mismatches);
      std::string req_name((*sa_req)->GetName());
      RecordIfNe((*sa_req)->GetHash(),
                 ELFIO::elf_hash(
                     reinterpret_cast<const unsigned char*>(req_name.c_str())),
                 absl::StrCat(prefix, " req.hash"), mismatches);
      // ELFIO's const_versym_r_section_accessor::get_entry reads only the
      // first Elfxx_Vernaux per Elfxx_Verneed and does not follow vna_next
      // chains when a shared library has multiple version requirements, so
      // mu_verneed_by_index is a subset on multi-requirement binaries.
      if (auto it = mu_verneed_by_index.find(mu_ver_num);
          it != mu_verneed_by_index.end()) {
        RecordIfNe((*sa_req)->GetFile(),
                   absl::string_view(it->second.file_name),
                   absl::StrCat(prefix, " req.file"), mismatches);
        RecordIfNe((*sa_req)->GetName(), absl::string_view(it->second.dep_name),
                   absl::StrCat(prefix, " req.name"), mismatches);
        RecordIfNe((*sa_req)->GetHash(), it->second.hash,
                   absl::StrCat(prefix, " req.verneed_hash"), mismatches);
        RecordIfNe((*sa_req)->GetFlags(), it->second.flags,
                   absl::StrCat(prefix, " req.flags"), mismatches);
      }
      sa_req_keys.insert(
          absl::StrCat((*sa_req)->GetFile(), ":", (*sa_req)->GetName(), ":",
                       (*sa_req)->GetHash(), ":", (*sa_req)->GetFlags()));
    }
    if (sa_def->has_value()) {
      RecordIfNe((*sa_def)->IsHidden(), mu_is_hidden,
                 absl::StrCat(prefix, " def.is_hidden"), mismatches);
      if ((*sa_def)->GetNameCount() == 0) {
        mismatches.push_back(
            absl::StrCat("Expected positive name count for ", prefix));
        continue;
      }
      absl::StatusOr<std::string> first_def_name = (*sa_def)->GetName(0);
      if (!first_def_name.ok()) {
        mismatches.push_back(
            absl::StrCat(prefix, ": ", first_def_name.status().ToString()));
        continue;
      }
      RecordIfNe(first_def_name->empty(), false,
                 absl::StrCat(prefix, " def.first_name_empty"), mismatches);
      RecordIfNe((*sa_def)->GetHash(),
                 ELFIO::elf_hash(reinterpret_cast<const unsigned char*>(
                     first_def_name->c_str())),
                 absl::StrCat(prefix, " def.hash"), mismatches);
      for (size_t j = 1; j < (*sa_def)->GetNameCount(); ++j) {
        absl::StatusOr<std::string> def_name = (*sa_def)->GetName(j);
        if (!def_name.ok()) {
          mismatches.push_back(absl::StrCat(prefix, " name ", j, ": ",
                                            def_name.status().ToString()));
          continue;
        }
        RecordIfNe(def_name->empty(), false,
                   absl::StrCat(prefix, " def.name[", j, "]_empty"),
                   mismatches);
      }
    }
  }

  for (const std::string& mu_key : mu_verneed_keys) {
    RecordIfNe(sa_req_keys.contains(mu_key), true,
               absl::StrCat("Missing verneed key ", mu_key), mismatches);
  }
  return FinishVerification(mismatches);
}

// Builds a feature-rich synthetic ELF binary parameterized by class
// (ELFCLASS32 or ELFCLASS64) and endianness (ELFDATA2LSB or ELFDATA2MSB)
// containing .dynsym, .hash (SHT_HASH), SHT_REL / SHT_RELA relocations,
// SHT_DYNAMIC, SHT_NOTE + PT_NOTE, .symtab, .debug_info (SHF_COMPRESSED), and
// (for little-endian payloads supported by ELFIO's versym accessor)
// .gnu.version + .gnu.version_r.
ElfPayload BuildFeatureRichSyntheticElf(unsigned char elf_class,
                                        unsigned char encoding,
                                        absl::string_view label) {
  ELFIO::elfio writer;
  writer.create(elf_class, encoding);
  writer.set_os_abi(ELFIO::ELFOSABI_LINUX);
  writer.set_abi_version(1);
  writer.set_type(ELFIO::ET_DYN);
  writer.set_machine(elf_class == ELFIO::ELFCLASS64 ? ELFIO::EM_X86_64
                                                    : ELFIO::EM_386);
  writer.set_entry(0x201000);
  writer.set_flags(0x5);

  const ELFIO::Elf_Xword addr_align = (elf_class == ELFIO::ELFCLASS64) ? 8 : 4;
  const ELFIO::endianess_convertor& convertor = writer.get_convertor();

  // 1. Allocatable .text and .data sections with PT_LOAD segments.
  ELFIO::section* text_sec = writer.sections.add(".text");
  text_sec->set_type(ELFIO::SHT_PROGBITS);
  text_sec->set_flags(ELFIO::SHF_ALLOC | ELFIO::SHF_EXECINSTR);
  text_sec->set_addr_align(16);
  text_sec->set_address(0x201000);
  std::string text_bytes = MakeDeterministicBytes(512, 1001ULL);
  text_sec->set_data(text_bytes.data(),
                     static_cast<ELFIO::Elf_Word>(text_bytes.size()));

  ELFIO::segment* text_seg = writer.segments.add();
  text_seg->set_type(ELFIO::PT_LOAD);
  text_seg->set_virtual_address(0x201000);
  text_seg->set_physical_address(0x201000);
  text_seg->set_flags(ELFIO::PF_R | ELFIO::PF_X);
  text_seg->set_align(0x1000);
  text_seg->add_section_index(text_sec->get_index(),
                              text_sec->get_addr_align());

  ELFIO::section* data_sec = writer.sections.add(".data");
  data_sec->set_type(ELFIO::SHT_PROGBITS);
  data_sec->set_flags(ELFIO::SHF_ALLOC | ELFIO::SHF_WRITE);
  data_sec->set_addr_align(addr_align);
  data_sec->set_address(0x202000);
  std::string data_bytes = MakeDeterministicBytes(256, 2002ULL);
  data_sec->set_data(data_bytes.data(),
                     static_cast<ELFIO::Elf_Word>(data_bytes.size()));

  ELFIO::segment* data_seg = writer.segments.add();
  data_seg->set_type(ELFIO::PT_LOAD);
  data_seg->set_virtual_address(0x202000);
  data_seg->set_physical_address(0x202000);
  data_seg->set_flags(ELFIO::PF_R | ELFIO::PF_W);
  data_seg->set_align(0x1000);
  data_seg->add_section_index(data_sec->get_index(),
                              data_sec->get_addr_align());

  // 2. .dynstr, .dynsym, and .hash (SHT_HASH) sections.
  ELFIO::section* dynstr_sec = writer.sections.add(".dynstr");
  dynstr_sec->set_type(ELFIO::SHT_STRTAB);
  dynstr_sec->set_flags(ELFIO::SHF_ALLOC);
  dynstr_sec->set_addr_align(1);
  ELFIO::string_section_accessor dynstr_writer(dynstr_sec);

  ELFIO::section* dynsym_sec = writer.sections.add(".dynsym");
  dynsym_sec->set_type(ELFIO::SHT_DYNSYM);
  dynsym_sec->set_flags(ELFIO::SHF_ALLOC);
  dynsym_sec->set_info(1);
  dynsym_sec->set_link(dynstr_sec->get_index());
  dynsym_sec->set_addr_align(addr_align);
  dynsym_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_SYMTAB));
  ELFIO::symbol_section_accessor dynsym_writer(writer, dynsym_sec);
  ELFIO::Elf_Word exported_sym_idx = dynsym_writer.add_symbol(
      dynstr_writer, "exported_api_fn", 0x201020, 64, ELFIO::STB_GLOBAL,
      ELFIO::STT_FUNC, ELFIO::STV_DEFAULT, text_sec->get_index());
  dynsym_writer.add_symbol(dynstr_writer, "hidden_helper_fn", 0x201080, 32,
                           ELFIO::STB_WEAK, ELFIO::STT_FUNC, ELFIO::STV_HIDDEN,
                           text_sec->get_index());
  dynsym_writer.add_symbol(dynstr_writer, "cos", 0, 0, ELFIO::STB_GLOBAL,
                           ELFIO::STT_FUNC, ELFIO::STV_DEFAULT,
                           ELFIO::SHN_UNDEF);

  // SysV hash table (.hash) indexing the 4 .dynsym entries with nbucket=1,
  // nchain=4, bucket[0]=1, and chain={0, 2, 3, 0}.
  ELFIO::section* hash_sec = writer.sections.add(".hash");
  hash_sec->set_type(ELFIO::SHT_HASH);
  hash_sec->set_flags(ELFIO::SHF_ALLOC);
  hash_sec->set_link(dynsym_sec->get_index());
  hash_sec->set_addr_align(4);
  hash_sec->set_entry_size(sizeof(ELFIO::Elf_Word));
  const ELFIO::Elf_Word raw_hash_words[7] = {
      convertor(static_cast<ELFIO::Elf_Word>(1)),
      convertor(static_cast<ELFIO::Elf_Word>(4)),
      convertor(static_cast<ELFIO::Elf_Word>(1)),
      convertor(static_cast<ELFIO::Elf_Word>(0)),
      convertor(static_cast<ELFIO::Elf_Word>(2)),
      convertor(static_cast<ELFIO::Elf_Word>(3)),
      convertor(static_cast<ELFIO::Elf_Word>(0)),
  };
  hash_sec->set_data(reinterpret_cast<const char*>(raw_hash_words),
                     sizeof(raw_hash_words));

  // 2b. .gnu.version (SHT_GNU_versym) and .gnu.version_r (SHT_GNU_verneed)
  // symbol versioning sections. ELFIO's versym_r_section_accessor casts
  // Elfxx_Verneed directly without endian conversion, so we emit these on
  // little-endian synthetic payloads.
  ELFIO::section* versym_sec = nullptr;
  ELFIO::section* verneed_sec = nullptr;
  if (encoding == ELFIO::ELFDATA2LSB) {
    versym_sec = writer.sections.add(".gnu.version");
    versym_sec->set_type(ELFIO::SHT_GNU_versym);
    versym_sec->set_flags(ELFIO::SHF_ALLOC);
    versym_sec->set_link(dynsym_sec->get_index());
    versym_sec->set_addr_align(2);
    versym_sec->set_address(0x200300);
    versym_sec->set_entry_size(sizeof(ELFIO::Elf_Half));
    ELFIO::versym_section_accessor versym_writer(versym_sec);
    versym_writer.add_entry(0);  // Symbol 0 (SHN_UNDEF null symbol): local
    versym_writer.add_entry(
        1);  // Symbol 1 (exported_api_fn): global unversioned
    versym_writer.add_entry(
        1);  // Symbol 2 (hidden_helper_fn): global unversioned
    versym_writer.add_entry(
        2);  // Symbol 3 (cos): version index 2 (GLIBC_2.2.5)

    verneed_sec = writer.sections.add(".gnu.version_r");
    verneed_sec->set_type(ELFIO::SHT_GNU_verneed);
    verneed_sec->set_flags(ELFIO::SHF_ALLOC);
    verneed_sec->set_link(dynstr_sec->get_index());
    verneed_sec->set_info(1);
    verneed_sec->set_addr_align(addr_align);
    verneed_sec->set_address(0x200310);
    ELFIO::Elfxx_Verneed verneed{};
    verneed.vn_version = 1;
    verneed.vn_cnt = 1;
    verneed.vn_file = dynstr_writer.add_string("libm.so.6");
    verneed.vn_aux = sizeof(ELFIO::Elfxx_Verneed);
    verneed.vn_next = 0;
    ELFIO::Elfxx_Vernaux vernaux{};
    vernaux.vna_hash =
        ELFIO::elf_hash(reinterpret_cast<const unsigned char*>("GLIBC_2.2.5"));
    vernaux.vna_flags = 0;
    vernaux.vna_other = 2;
    vernaux.vna_name = dynstr_writer.add_string("GLIBC_2.2.5");
    vernaux.vna_next = 0;
    std::string verneed_bytes(
        sizeof(ELFIO::Elfxx_Verneed) + sizeof(ELFIO::Elfxx_Vernaux), '\0');
    std::memcpy(verneed_bytes.data(), &verneed, sizeof(verneed));
    std::memcpy(verneed_bytes.data() + sizeof(verneed), &vernaux,
                sizeof(vernaux));
    verneed_sec->set_data(verneed_bytes.data(),
                          static_cast<ELFIO::Elf_Word>(verneed_bytes.size()));
  }

  // 3. .rel.dyn (SHT_REL) and .rela.dyn (SHT_RELA) relocation sections.
  ELFIO::section* rel_sec = writer.sections.add(".rel.dyn");
  rel_sec->set_type(ELFIO::SHT_REL);
  rel_sec->set_flags(ELFIO::SHF_ALLOC);
  rel_sec->set_info(data_sec->get_index());
  rel_sec->set_link(dynsym_sec->get_index());
  rel_sec->set_addr_align(addr_align);
  rel_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_REL));
  ELFIO::relocation_section_accessor rel_writer(writer, rel_sec);
  rel_writer.add_entry(0x202010, exported_sym_idx, 1U);

  ELFIO::section* rela_sec = writer.sections.add(".rela.dyn");
  rela_sec->set_type(ELFIO::SHT_RELA);
  rela_sec->set_flags(ELFIO::SHF_ALLOC);
  rela_sec->set_info(data_sec->get_index());
  rela_sec->set_link(dynsym_sec->get_index());
  rela_sec->set_addr_align(addr_align);
  rela_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_RELA));
  ELFIO::relocation_section_accessor rela_writer(writer, rela_sec);
  rela_writer.add_entry(0x202000, exported_sym_idx,
                        static_cast<unsigned>(ELFIO::R_X86_64_64), -16);
  rela_writer.add_entry(0x202008, 0,
                        static_cast<unsigned>(ELFIO::R_X86_64_RELATIVE), 4096);

  // 4. .dynamic (SHT_DYNAMIC) section.
  ELFIO::section* dyn_sec = writer.sections.add(".dynamic");
  dyn_sec->set_type(ELFIO::SHT_DYNAMIC);
  dyn_sec->set_flags(ELFIO::SHF_ALLOC | ELFIO::SHF_WRITE);
  dyn_sec->set_link(dynstr_sec->get_index());
  dyn_sec->set_addr_align(addr_align);
  dyn_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_DYNAMIC));
  ELFIO::dynamic_section_accessor dyn_writer(writer, dyn_sec);
  dyn_writer.add_entry(ELFIO::DT_NEEDED, "libm.so.6");
  dyn_writer.add_entry(ELFIO::DT_SONAME, "libsynthetic.so");
  dyn_writer.add_entry(ELFIO::DT_INIT, 0x201000);
  dyn_writer.add_entry(ELFIO::DT_FLAGS, ELFIO::DF_BIND_NOW);
  if (versym_sec != nullptr && verneed_sec != nullptr) {
    dyn_writer.add_entry(ELFIO::DT_VERSYM, versym_sec->get_address());
    dyn_writer.add_entry(ELFIO::DT_VERNEED, verneed_sec->get_address());
    dyn_writer.add_entry(ELFIO::DT_VERNEEDNUM, 1);
  }
  dyn_writer.add_entry(ELFIO::DT_NULL, 0);

  // 5. .note.gnu.build-id with NT_GNU_ABI_TAG, NT_GNU_BUILD_ID, and custom
  // vendor note in an SHT_NOTE section + PT_NOTE segment.
  ELFIO::section* note_sec = writer.sections.add(".note.gnu.build-id");
  note_sec->set_type(ELFIO::SHT_NOTE);
  note_sec->set_flags(ELFIO::SHF_ALLOC);
  note_sec->set_addr_align(4);
  note_sec->set_address(0x200200);
  ELFIO::note_section_accessor note_writer(writer, note_sec);
  const ELFIO::Elf_Word abi_words[4] = {
      convertor(static_cast<ELFIO::Elf_Word>(0)),
      convertor(static_cast<ELFIO::Elf_Word>(3)),
      convertor(static_cast<ELFIO::Elf_Word>(2)),
      convertor(static_cast<ELFIO::Elf_Word>(0)),
  };
  note_writer.add_note(ELFIO::NT_GNU_ABI_TAG, "GNU",
                       reinterpret_cast<const char*>(abi_words),
                       sizeof(abi_words));
  constexpr absl::string_view kBuildIdBytes =
      "\x11\x22\x33\x44\x55\x66\x77\x88";
  note_writer.add_note(ELFIO::NT_GNU_BUILD_ID, "GNU", kBuildIdBytes.data(),
                       static_cast<ELFIO::Elf_Word>(kBuildIdBytes.size()));
  constexpr absl::string_view kCustomDesc = "synthetic_fw_v1";
  note_writer.add_note(0x42, "GOOG", kCustomDesc.data(),
                       static_cast<ELFIO::Elf_Word>(kCustomDesc.size()));

  ELFIO::segment* note_seg = writer.segments.add();
  note_seg->set_type(ELFIO::PT_NOTE);
  note_seg->set_virtual_address(0x200200);
  note_seg->set_physical_address(0x200200);
  note_seg->set_flags(ELFIO::PF_R);
  note_seg->set_align(4);
  note_seg->add_section_index(note_sec->get_index(),
                              note_sec->get_addr_align());

  // 6. .strtab and .symtab sections.
  ELFIO::section* str_sec = writer.sections.add(".strtab");
  str_sec->set_type(ELFIO::SHT_STRTAB);
  str_sec->set_addr_align(1);
  ELFIO::string_section_accessor str_writer(str_sec);

  ELFIO::section* sym_sec = writer.sections.add(".symtab");
  sym_sec->set_type(ELFIO::SHT_SYMTAB);
  sym_sec->set_link(str_sec->get_index());
  sym_sec->set_addr_align(addr_align);
  sym_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_SYMTAB));
  ELFIO::symbol_section_accessor sym_writer(writer, sym_sec);
  sym_writer.add_symbol(str_writer, "local_var", 0x202000, 8, ELFIO::STB_LOCAL,
                        ELFIO::STT_OBJECT, ELFIO::STV_INTERNAL,
                        data_sec->get_index());
  sym_writer.add_symbol(str_writer, "exported_api_fn", 0x201020, 64,
                        ELFIO::STB_GLOBAL, ELFIO::STT_FUNC,
                        ELFIO::STV_PROTECTED, text_sec->get_index());
  sym_writer.arrange_local_symbols();

  // 7. .debug_info (SHF_COMPRESSED) section prefixed with an endian-converted
  // Elf32_Chdr or Elf64_Chdr compression header.
  ELFIO::section* comp_sec = writer.sections.add(".debug_info");
  comp_sec->set_type(ELFIO::SHT_PROGBITS);
  comp_sec->set_flags(ELFIO::SHF_COMPRESSED);
  comp_sec->set_addr_align(addr_align);
  std::string comp_payload = MakeDeterministicBytes(48, 3003ULL);
  std::string comp_bytes;
  if (elf_class == ELFIO::ELFCLASS64) {
    ELFIO::Elf64_Chdr chdr{};
    chdr.ch_type = convertor(static_cast<ELFIO::Elf64_Word>(1));  // ZLIB
    chdr.ch_reserved = 0;
    chdr.ch_size = convertor(static_cast<ELFIO::Elf_Xword>(128));
    chdr.ch_addralign = convertor(addr_align);
    comp_bytes.resize(sizeof(chdr));
    std::memcpy(comp_bytes.data(), &chdr, sizeof(chdr));
  } else {
    ELFIO::Elf32_Chdr chdr{};
    chdr.ch_type = convertor(static_cast<ELFIO::Elf32_Word>(1));  // ZLIB
    chdr.ch_size = convertor(static_cast<ELFIO::Elf32_Word>(128));
    chdr.ch_addralign = convertor(static_cast<ELFIO::Elf32_Word>(addr_align));
    comp_bytes.resize(sizeof(chdr));
    std::memcpy(comp_bytes.data(), &chdr, sizeof(chdr));
  }
  comp_bytes.append(comp_payload);
  comp_sec->set_data(comp_bytes.data(),
                     static_cast<ELFIO::Elf_Word>(comp_bytes.size()));

  std::ostringstream oss(std::ios::out | std::ios::binary);
  QCHECK(writer.save(oss)) << "Failed to serialize synthetic ELF payload: "
                           << label;
  return ElfPayload{
      .name = std::string(label),
      .bytes = std::move(oss).str(),
      .file_path = "",
      .target_section_names = {".text", ".data", ".dynsym", ".dynstr", ".hash",
                               ".gnu.version", ".gnu.version_r", ".rel.dyn",
                               ".rela.dyn", ".dynamic", ".note.gnu.build-id",
                               ".symtab", ".strtab", ".debug_info"},
      .expect_symbol_versioning = (encoding == ELFIO::ELFDATA2LSB),
  };
}

// Returns the extended set of feature-rich synthetic payloads covering 64-bit
// little-endian, 32-bit little-endian, 64-bit big-endian, and 32-bit big-endian
// ELF binaries with dynamic symbols, SysV hash tables, relocations, dynamic
// tags, notes, compressed sections, and symbol versioning.
std::vector<ElfPayload> GetExtendedSyntheticPayloads() {
  return {
      BuildFeatureRichSyntheticElf(
          ELFIO::ELFCLASS64, ELFIO::ELFDATA2LSB,
          "Feature-Rich Synthetic ELF64 Little-Endian"),
      BuildFeatureRichSyntheticElf(
          ELFIO::ELFCLASS32, ELFIO::ELFDATA2LSB,
          "Feature-Rich Synthetic ELF32 Little-Endian"),
      BuildFeatureRichSyntheticElf(ELFIO::ELFCLASS64, ELFIO::ELFDATA2MSB,
                                   "Feature-Rich Synthetic ELF64 Big-Endian"),
      BuildFeatureRichSyntheticElf(ELFIO::ELFCLASS32, ELFIO::ELFDATA2MSB,
                                   "Feature-Rich Synthetic ELF32 Big-Endian"),
  };
}

class ElfComparisonTest : public testing::TestWithParam<ElfPayload> {};

TEST_P(ElfComparisonTest, FileHeaderMatches) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false))
      << "Payload: " << payload.name;

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(elf_bridge::FileHeader sa_hdr, sa_elf.GetFileHeader());

  ASSERT_OK(VerifyFileHeaderParity(mu_reader, sa_hdr))
      << "Payload: " << payload.name;
}

TEST_P(ElfComparisonTest, SectionHeadersAndPayloadsMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeaderTableWithStrings sa_shdrs_strs,
                       sa_elf.GetSectionHeadersWithStrTab());
  ASSERT_TRUE(sa_shdrs_strs.GetSectionHeaderTable().has_value());
  ASSERT_TRUE(sa_shdrs_strs.GetStringTable().has_value());

  const elf_bridge::SectionHeaderTable& sa_shdrs =
      *sa_shdrs_strs.GetSectionHeaderTable();
  const elf_bridge::StringTable& sa_strtab = *sa_shdrs_strs.GetStringTable();

  ASSERT_EQ(sa_shdrs.size(), mu_reader.sections.size());

  for (size_t i = 0; i < sa_shdrs.size(); ++i) {
    const ELFIO::section* mu_sec = mu_reader.sections[i];
    ASSERT_NE(mu_sec, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec,
                         sa_shdrs.GetSectionHeader(i));

    EXPECT_EQ(sa_sec.GetName(), mu_sec->get_name_string_offset())
        << "Payload: " << payload.name << ", section index " << i;
    ASSERT_OK_AND_ASSIGN(std::string sa_name, sa_strtab.Get(sa_sec.GetName()));
    EXPECT_EQ(sa_name, mu_sec->get_name())
        << "Payload: " << payload.name << ", section index " << i;
    ASSERT_OK(VerifySectionHeaderFieldsParity(
        *mu_sec, sa_sec, absl::StrCat("Section index ", i)))
        << "Payload: " << payload.name;

    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionData sa_data,
                         sa_elf.GetSectionData(sa_sec));
    ASSERT_OK(VerifySectionDataParity(mu_reader, *mu_sec, sa_data, i, sa_name))
        << "Payload: " << payload.name;
  }
}

TEST_P(ElfComparisonTest, StringTablesMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SectionHeaderTable> sa_shdrs,
                       sa_elf.GetSectionHeaders());
  ASSERT_TRUE(sa_shdrs.has_value());
  ASSERT_EQ(sa_shdrs->size(), mu_reader.sections.size());

  for (size_t i = 0; i < sa_shdrs->size(); ++i) {
    const ELFIO::section* mu_sec = mu_reader.sections[i];
    ASSERT_NE(mu_sec, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec,
                         sa_shdrs->GetSectionHeader(i));
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionType sa_type, sa_sec.GetType());
    if (sa_type != elf_bridge::SectionType::kStrTab ||
        (mu_sec->get_flags() & ELFIO::SHF_COMPRESSED) != 0) {
      continue;
    }

    ASSERT_OK_AND_ASSIGN(elf_bridge::StringTable sa_strtab,
                         sa_elf.GetSectionDataAsStrTab(sa_sec));
    ASSERT_OK(VerifyStringTableParity(*mu_sec, sa_strtab))
        << "Payload: " << payload.name << ", section index " << i;
  }
}

TEST_P(ElfComparisonTest, NamedSectionLookupsMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/true));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));

  for (const std::string& target_name : payload.target_section_names) {
    const ELFIO::section* mu_sec = mu_reader.sections[target_name];
    ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SectionHeader> sa_sec_opt,
                         sa_elf.GetSectionHeaderByName(target_name));

    ASSERT_EQ(sa_sec_opt.has_value(), mu_sec != nullptr)
        << "Payload: " << payload.name << ", section name: " << target_name;
    if (mu_sec != nullptr && sa_sec_opt.has_value()) {
      ASSERT_OK(
          VerifySectionHeaderFieldsParity(*mu_sec, *sa_sec_opt, target_name))
          << "Payload: " << payload.name;
      ASSERT_OK_AND_ASSIGN(elf_bridge::SectionData sa_data,
                           sa_elf.GetSectionData(*sa_sec_opt));
      ASSERT_OK(VerifySectionDataParity(mu_reader, *mu_sec, sa_data,
                                        mu_sec->get_index(), target_name))
          << "Payload: " << payload.name;
    }
  }

  constexpr absl::string_view kMissingSec = ".definitely_missing_section";
  EXPECT_EQ(mu_reader.sections[kMissingSec], nullptr);
  EXPECT_THAT(sa_elf.GetSectionHeaderByName(kMissingSec),
              IsOkAndHolds(Eq(std::nullopt)));
}

TEST_P(ElfComparisonTest, ProgramSegmentsMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SegmentTable> sa_segs_opt,
                       sa_elf.GetSegments());

  if (mu_reader.segments.size() == 0) {
    EXPECT_TRUE(!sa_segs_opt.has_value() || sa_segs_opt->empty());
    return;
  }

  ASSERT_TRUE(sa_segs_opt.has_value());
  ASSERT_EQ(sa_segs_opt->size(), mu_reader.segments.size());

  for (size_t i = 0; i < sa_segs_opt->size(); ++i) {
    const ELFIO::segment* mu_seg = mu_reader.segments[i];
    ASSERT_NE(mu_seg, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::ProgramHeader sa_phdr,
                         sa_segs_opt->GetProgramHeader(i));

    ASSERT_OK(VerifyProgramHeaderFieldsParity(*mu_seg, sa_phdr, i))
        << "Payload: " << payload.name;

    if (sa_phdr.GetFileSize() > 0 && mu_seg->get_data() != nullptr) {
      ASSERT_OK_AND_ASSIGN(std::string sa_seg_data,
                           sa_elf.GetSegmentData(sa_phdr));
      absl::string_view mu_seg_data(mu_seg->get_data(),
                                    mu_seg->get_file_size());
      EXPECT_EQ(sa_seg_data, mu_seg_data)
          << "Payload: " << payload.name << ", segment index " << i;
    }
  }
}

TEST_P(ElfComparisonTest, SymbolTablesMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));

  // 1. Static symbol table (.symtab).
  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_symtab_opt,
      sa_elf.GetSymbolTable());
  ASSERT_OK(VerifySymbolTableParity(mu_reader, mu_reader.sections[".symtab"],
                                    sa_symtab_opt, ".symtab"))
      << "Payload: " << payload.name;

  // 2. Dynamic symbol table (.dynsym).
  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_dynsym_opt,
      sa_elf.GetDynamicSymbolTable());
  ASSERT_OK(VerifySymbolTableParity(mu_reader, mu_reader.sections[".dynsym"],
                                    sa_dynsym_opt, ".dynsym"))
      << "Payload: " << payload.name;
}

TEST_P(ElfComparisonTest, CommonDataAndHashTableLookupsMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(elf_bridge::CommonElfData sa_common,
                       sa_elf.GetCommonData());

  const ELFIO::section* mu_symtab_sec = mu_reader.sections[".symtab"];
  ASSERT_EQ(sa_common.GetSymbolTable().has_value(), mu_symtab_sec != nullptr);
  ASSERT_EQ(sa_common.GetSymbolTableStrings().has_value(),
            mu_symtab_sec != nullptr);
  if (mu_symtab_sec != nullptr) {
    ASSERT_OK(VerifySymbolTableAndStringsParity(
        mu_reader, *mu_symtab_sec, *sa_common.GetSymbolTable(),
        *sa_common.GetSymbolTableStrings(), "CommonData .symtab"))
        << "Payload: " << payload.name;
  }

  const ELFIO::section* mu_dynsym_sec = mu_reader.sections[".dynsym"];
  ASSERT_EQ(sa_common.GetDynamicSymbolTable().has_value(),
            mu_dynsym_sec != nullptr);
  ASSERT_EQ(sa_common.GetDynamicSymbolTableStrings().has_value(),
            mu_dynsym_sec != nullptr);
  if (mu_dynsym_sec != nullptr) {
    ASSERT_OK(VerifySymbolTableAndStringsParity(
        mu_reader, *mu_dynsym_sec, *sa_common.GetDynamicSymbolTable(),
        *sa_common.GetDynamicSymbolTableStrings(), "CommonData .dynsym"))
        << "Payload: " << payload.name;
  }

  ASSERT_OK(VerifyDynamicTableParity(mu_reader, mu_reader.sections[".dynamic"],
                                     sa_common.GetDynamicTable(),
                                     sa_common.GetDynamicSymbolTableStrings()))
      << "Payload: " << payload.name;

  EXPECT_EQ(sa_common.GetGnuHashTable().has_value(),
            mu_reader.sections[".gnu.hash"] != nullptr);
  EXPECT_EQ(sa_common.GetSysVHashTable().has_value(),
            mu_reader.sections[".hash"] != nullptr);

  if (mu_dynsym_sec == nullptr || (!sa_common.GetGnuHashTable().has_value() &&
                                   !sa_common.GetSysVHashTable().has_value())) {
    return;
  }

  const elf_bridge::SymbolTable& sa_dynsyms =
      *sa_common.GetDynamicSymbolTable();
  const elf_bridge::StringTable& sa_dynstrs =
      *sa_common.GetDynamicSymbolTableStrings();
  ELFIO::const_symbol_section_accessor mu_dynsyms(mu_reader, mu_dynsym_sec);

  for (size_t i = 0; i < sa_dynsyms.size(); ++i) {
    ASSERT_OK_AND_ASSIGN(elf_bridge::Symbol sa_sym, sa_dynsyms.GetSymbol(i));
    ASSERT_OK_AND_ASSIGN(std::string sym_name,
                         sa_dynstrs.Get(sa_sym.GetName()));
    ASSERT_OK_AND_ASSIGN(elf_bridge::SymbolBind sa_bind, sa_sym.GetBinding());
    if (sym_name.empty() || sa_sym.IsUndefined() ||
        sa_bind == elf_bridge::SymbolBind::kLocal) {
      continue;
    }

    ELFIO::Elf64_Addr mu_value = 0;
    ELFIO::Elf_Xword mu_size = 0;
    unsigned char mu_bind = 0;
    unsigned char mu_type = 0;
    ELFIO::Elf_Half mu_shndx = 0;
    unsigned char mu_other = 0;
    ASSERT_TRUE(mu_dynsyms.get_symbol(sym_name, mu_value, mu_size, mu_bind,
                                      mu_type, mu_shndx, mu_other))
        << "Symbol: " << sym_name;

    if (sa_common.GetGnuHashTable().has_value()) {
      ASSERT_OK_AND_ASSIGN(
          std::optional<elf_bridge::SymbolFindResult> sa_found,
          sa_common.GetGnuHashTable()->Find(sym_name, sa_dynsyms, sa_dynstrs));
      ASSERT_TRUE(sa_found.has_value()) << "Symbol: " << sym_name;
      EXPECT_EQ(sa_found->GetIndex(), i) << "Symbol: " << sym_name;
      EXPECT_EQ(sa_found->GetSymbol().GetValue(), mu_value);
      EXPECT_EQ(sa_found->GetSymbol().GetSize(), mu_size);
      EXPECT_EQ(sa_found->GetSymbol().GetSectionHeaderIndex(), mu_shndx);
    }
    if (sa_common.GetSysVHashTable().has_value()) {
      ASSERT_OK_AND_ASSIGN(
          std::optional<elf_bridge::SymbolFindResult> sa_found,
          sa_common.GetSysVHashTable()->Find(sym_name, sa_dynsyms, sa_dynstrs));
      ASSERT_TRUE(sa_found.has_value()) << "Symbol: " << sym_name;
      EXPECT_EQ(sa_found->GetIndex(), i) << "Symbol: " << sym_name;
      EXPECT_EQ(sa_found->GetSymbol().GetValue(), mu_value);
      EXPECT_EQ(sa_found->GetSymbol().GetSize(), mu_size);
      EXPECT_EQ(sa_found->GetSymbol().GetSectionHeaderIndex(), mu_shndx);
    }
  }

  // Verify non-existent symbol lookup returns empty / false in both libraries.
  ELFIO::Elf64_Addr mu_value = 0;
  ELFIO::Elf_Xword mu_size = 0;
  unsigned char mu_bind = 0;
  unsigned char mu_type = 0;
  ELFIO::Elf_Half mu_shndx = 0;
  unsigned char mu_other = 0;
  constexpr absl::string_view kMissingSym = "__definitely_missing_symbol__";
  EXPECT_FALSE(mu_dynsyms.get_symbol(std::string(kMissingSym), mu_value,
                                     mu_size, mu_bind, mu_type, mu_shndx,
                                     mu_other));
  if (sa_common.GetGnuHashTable().has_value()) {
    EXPECT_THAT(
        sa_common.GetGnuHashTable()->Find(kMissingSym, sa_dynsyms, sa_dynstrs),
        IsOkAndHolds(Eq(std::nullopt)));
  }
  if (sa_common.GetSysVHashTable().has_value()) {
    EXPECT_THAT(
        sa_common.GetSysVHashTable()->Find(kMissingSym, sa_dynsyms, sa_dynstrs),
        IsOkAndHolds(Eq(std::nullopt)));
  }
}

TEST_P(ElfComparisonTest, DynamicSectionMatches) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::DynTable> sa_dyn_opt,
                       sa_elf.GetDynamic());
  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_dynsym_opt,
      sa_elf.GetDynamicSymbolTable());
  std::optional<elf_bridge::StringTable> sa_dynstr_opt =
      sa_dynsym_opt.has_value() ? std::optional<elf_bridge::StringTable>(
                                      sa_dynsym_opt->GetStringTable())
                                : std::nullopt;

  ASSERT_OK(VerifyDynamicTableParity(mu_reader, mu_reader.sections[".dynamic"],
                                     sa_dyn_opt, sa_dynstr_opt))
      << "Payload: " << payload.name;
}

TEST_P(ElfComparisonTest, RelocationsMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SectionHeaderTable> sa_shdrs,
                       sa_elf.GetSectionHeaders());
  ASSERT_TRUE(sa_shdrs.has_value());
  ASSERT_EQ(sa_shdrs->size(), mu_reader.sections.size());

  for (size_t i = 0; i < sa_shdrs->size(); ++i) {
    const ELFIO::section* mu_sec = mu_reader.sections[i];
    ASSERT_NE(mu_sec, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec,
                         sa_shdrs->GetSectionHeader(i));
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionType sa_type, sa_sec.GetType());

    if (sa_type == elf_bridge::SectionType::kRel) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::RelTable sa_rels,
                           sa_elf.GetSectionDataAsRels(sa_sec));
      ASSERT_OK(VerifyRelTableParity(mu_reader, *mu_sec, sa_rels))
          << "Payload: " << payload.name << ", section index " << i;
    } else if (sa_type == elf_bridge::SectionType::kRela) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::RelaTable sa_relas,
                           sa_elf.GetSectionDataAsRelas(sa_sec));
      ASSERT_OK(VerifyRelaTableParity(mu_reader, *mu_sec, sa_relas))
          << "Payload: " << payload.name << ", section index " << i;
    }
  }
}

TEST_P(ElfComparisonTest, NotesMatch) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SectionHeaderTable> sa_shdrs,
                       sa_elf.GetSectionHeaders());
  ASSERT_TRUE(sa_shdrs.has_value());

  // 1. SHT_NOTE sections.
  for (size_t i = 0; i < sa_shdrs->size(); ++i) {
    const ELFIO::section* mu_sec = mu_reader.sections[i];
    ASSERT_NE(mu_sec, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec,
                         sa_shdrs->GetSectionHeader(i));
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionType sa_type, sa_sec.GetType());
    if (sa_type != elf_bridge::SectionType::kNote) {
      continue;
    }

    ASSERT_OK_AND_ASSIGN(elf_bridge::NoteTable sa_notes,
                         sa_elf.GetSectionDataAsNotes(sa_sec));
    ELFIO::const_note_section_accessor mu_notes(mu_reader, mu_sec);
    ASSERT_OK(
        VerifyNoteTableParity(mu_reader, mu_notes, sa_notes,
                              absl::StrCat("Section ", mu_sec->get_name())))
        << "Payload: " << payload.name << ", section index " << i;
  }

  // 2. PT_NOTE segments.
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SegmentTable> sa_segs_opt,
                       sa_elf.GetSegments());
  if (!sa_segs_opt.has_value()) {
    return;
  }
  for (size_t i = 0; i < sa_segs_opt->size(); ++i) {
    const ELFIO::segment* mu_seg = mu_reader.segments[i];
    ASSERT_NE(mu_seg, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::ProgramHeader sa_phdr,
                         sa_segs_opt->GetProgramHeader(i));
    ASSERT_OK_AND_ASSIGN(elf_bridge::ProgramHeaderType sa_type,
                         sa_phdr.GetType());
    if (sa_type != elf_bridge::ProgramHeaderType::kNote) {
      continue;
    }

    ASSERT_OK_AND_ASSIGN(elf_bridge::NoteTable sa_notes,
                         sa_elf.GetSegmentDataAsNotes(sa_phdr));
    ELFIO::const_note_segment_accessor mu_notes(mu_reader, mu_seg);
    ASSERT_OK(VerifyNoteTableParity(mu_reader, mu_notes, sa_notes,
                                    absl::StrCat("Segment ", i)))
        << "Payload: " << payload.name;
  }
}

TEST_P(ElfComparisonTest, SymbolVersionTableMatches) {
  const ElfPayload& payload = GetParam();

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(stream, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfBytes sa_elf,
                       elf_bridge::ElfBytes::Parse(payload.bytes));
  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_dynsym_opt,
      sa_elf.GetDynamicSymbolTable());
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SymbolVersionTable> sa_svt_opt,
                       sa_elf.GetSymbolVersionTable());

  ASSERT_OK(VerifySymbolVersionTableParity(mu_reader, sa_dynsym_opt, sa_svt_opt,
                                           payload))
      << "Payload: " << payload.name;
}

TEST_P(ElfComparisonTest, FileStreamMatchesUnsafe) {
  const ElfPayload& payload = GetParam();
  std::string file_path = payload.file_path;
  if (file_path.empty()) {
    file_path = file::JoinPath(testing::TempDir(), "synthetic_stream_test.elf");
    ASSERT_OK(file::SetContents(file_path, payload.bytes, file::Defaults()));
  }

  ELFIO::elfio mu_reader;
  ASSERT_TRUE(mu_reader.load(file_path, /*is_lazy=*/false));

  ASSERT_OK_AND_ASSIGN(elf_bridge::ElfStream sa_stream,
                       elf_bridge::ElfStream::Open(file_path));

  // 1. File header parity.
  ASSERT_OK_AND_ASSIGN(elf_bridge::FileHeader sa_hdr,
                       sa_stream.GetFileHeader());
  ASSERT_OK(VerifyFileHeaderParity(mu_reader, sa_hdr))
      << "Payload: " << payload.name;

  // 2. Section header table, string tables, relocations, notes, and payloads.
  ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeaderTable sa_shdrs_only,
                       sa_stream.GetSectionHeaders());
  ASSERT_EQ(sa_shdrs_only.size(), mu_reader.sections.size());

  ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeaderTableWithStrings sa_shdrs_strs,
                       sa_stream.GetSectionHeadersWithStrTab());
  ASSERT_TRUE(sa_shdrs_strs.GetSectionHeaderTable().has_value());
  ASSERT_TRUE(sa_shdrs_strs.GetStringTable().has_value());
  const elf_bridge::SectionHeaderTable& sa_shdrs =
      *sa_shdrs_strs.GetSectionHeaderTable();
  const elf_bridge::StringTable& sa_shstrtab = *sa_shdrs_strs.GetStringTable();
  ASSERT_EQ(sa_shdrs.size(), mu_reader.sections.size());

  for (size_t i = 0; i < sa_shdrs.size(); ++i) {
    const ELFIO::section* mu_sec = mu_reader.sections[i];
    ASSERT_NE(mu_sec, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec_only,
                         sa_shdrs_only.GetSectionHeader(i));
    ASSERT_OK(VerifySectionHeaderFieldsParity(
        *mu_sec, sa_sec_only, absl::StrCat("Stream SectionHeader ", i)))
        << "Payload: " << payload.name;

    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionHeader sa_sec,
                         sa_shdrs.GetSectionHeader(i));
    ASSERT_OK_AND_ASSIGN(std::string sa_name,
                         sa_shstrtab.Get(sa_sec.GetName()));
    EXPECT_EQ(sa_name, mu_sec->get_name())
        << "Payload: " << payload.name << ", section index " << i;
    ASSERT_OK(VerifySectionHeaderFieldsParity(*mu_sec, sa_sec, sa_name))
        << "Payload: " << payload.name;

    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionData sa_data,
                         sa_stream.GetSectionData(sa_sec));
    ASSERT_OK(VerifySectionDataParity(mu_reader, *mu_sec, sa_data, i, sa_name))
        << "Payload: " << payload.name;

    ASSERT_OK_AND_ASSIGN(elf_bridge::SectionType sa_type, sa_sec.GetType());
    if (sa_type == elf_bridge::SectionType::kStrTab &&
        (mu_sec->get_flags() & ELFIO::SHF_COMPRESSED) == 0) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::StringTable sa_strtab,
                           sa_stream.GetSectionDataAsStrTab(sa_sec));
      ASSERT_OK(VerifyStringTableParity(*mu_sec, sa_strtab))
          << "Payload: " << payload.name << ", section index " << i;
    } else if (sa_type == elf_bridge::SectionType::kRel) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::RelTable sa_rels,
                           sa_stream.GetSectionDataAsRels(sa_sec));
      ASSERT_OK(VerifyRelTableParity(mu_reader, *mu_sec, sa_rels))
          << "Payload: " << payload.name << ", section index " << i;
    } else if (sa_type == elf_bridge::SectionType::kRela) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::RelaTable sa_relas,
                           sa_stream.GetSectionDataAsRelas(sa_sec));
      ASSERT_OK(VerifyRelaTableParity(mu_reader, *mu_sec, sa_relas))
          << "Payload: " << payload.name << ", section index " << i;
    } else if (sa_type == elf_bridge::SectionType::kNote) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::NoteTable sa_notes,
                           sa_stream.GetSectionDataAsNotes(sa_sec));
      ELFIO::const_note_section_accessor mu_notes(mu_reader, mu_sec);
      ASSERT_OK(VerifyNoteTableParity(mu_reader, mu_notes, sa_notes, sa_name))
          << "Payload: " << payload.name << ", section index " << i;
    }
  }

  for (const std::string& target_name : payload.target_section_names) {
    const ELFIO::section* mu_sec = mu_reader.sections[target_name];
    ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SectionHeader> sa_sec_opt,
                         sa_stream.GetSectionHeaderByName(target_name));
    ASSERT_EQ(sa_sec_opt.has_value(), mu_sec != nullptr)
        << "Payload: " << payload.name
        << ", stream section lookup: " << target_name;
    if (mu_sec != nullptr && sa_sec_opt.has_value()) {
      ASSERT_OK(
          VerifySectionHeaderFieldsParity(*mu_sec, *sa_sec_opt, target_name))
          << "Payload: " << payload.name;
      ASSERT_OK_AND_ASSIGN(elf_bridge::SectionData sa_data,
                           sa_stream.GetSectionData(*sa_sec_opt));
      ASSERT_OK(VerifySectionDataParity(mu_reader, *mu_sec, sa_data,
                                        mu_sec->get_index(), target_name))
          << "Payload: " << payload.name;
    }
  }
  constexpr absl::string_view kMissingSec = ".definitely_missing_section";
  EXPECT_EQ(mu_reader.sections[kMissingSec], nullptr);
  EXPECT_THAT(sa_stream.GetSectionHeaderByName(kMissingSec),
              IsOkAndHolds(Eq(std::nullopt)));

  // 3. Program segments and segment notes.
  ASSERT_OK_AND_ASSIGN(elf_bridge::SegmentTable sa_segs,
                       sa_stream.GetSegments());
  ASSERT_EQ(sa_segs.size(), mu_reader.segments.size());
  for (size_t i = 0; i < sa_segs.size(); ++i) {
    const ELFIO::segment* mu_seg = mu_reader.segments[i];
    ASSERT_NE(mu_seg, nullptr);
    ASSERT_OK_AND_ASSIGN(elf_bridge::ProgramHeader sa_phdr,
                         sa_segs.GetProgramHeader(i));
    ASSERT_OK(VerifyProgramHeaderFieldsParity(*mu_seg, sa_phdr, i))
        << "Payload: " << payload.name;

    ASSERT_OK_AND_ASSIGN(elf_bridge::ProgramHeaderType sa_type,
                         sa_phdr.GetType());
    if (sa_type == elf_bridge::ProgramHeaderType::kNote) {
      ASSERT_OK_AND_ASSIGN(elf_bridge::NoteTable sa_notes,
                           sa_stream.GetSegmentDataAsNotes(sa_phdr));
      ELFIO::const_note_segment_accessor mu_notes(mu_reader, mu_seg);
      ASSERT_OK(VerifyNoteTableParity(mu_reader, mu_notes, sa_notes,
                                      absl::StrCat("Stream Segment ", i)))
          << "Payload: " << payload.name;
    }
  }

  // 4. Symbol tables (.symtab and .dynsym).
  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_symtab_opt,
      sa_stream.GetSymbolTable());
  ASSERT_OK(VerifySymbolTableParity(mu_reader, mu_reader.sections[".symtab"],
                                    sa_symtab_opt, "Stream .symtab"))
      << "Payload: " << payload.name;

  ASSERT_OK_AND_ASSIGN(
      std::optional<elf_bridge::SymbolTableWithStrings> sa_dynsym_opt,
      sa_stream.GetDynamicSymbolTable());
  ASSERT_OK(VerifySymbolTableParity(mu_reader, mu_reader.sections[".dynsym"],
                                    sa_dynsym_opt, "Stream .dynsym"))
      << "Payload: " << payload.name;

  // 5. Dynamic tags and symbol version table.
  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::DynTable> sa_dyn_opt,
                       sa_stream.GetDynamic());
  std::optional<elf_bridge::StringTable> sa_dynstr_opt =
      sa_dynsym_opt.has_value() ? std::optional<elf_bridge::StringTable>(
                                      sa_dynsym_opt->GetStringTable())
                                : std::nullopt;
  ASSERT_OK(VerifyDynamicTableParity(mu_reader, mu_reader.sections[".dynamic"],
                                     sa_dyn_opt, sa_dynstr_opt))
      << "Payload: " << payload.name;

  ASSERT_OK_AND_ASSIGN(std::optional<elf_bridge::SymbolVersionTable> sa_svt_opt,
                       sa_stream.GetSymbolVersionTable());
  ASSERT_OK(VerifySymbolVersionTableParity(mu_reader, sa_dynsym_opt, sa_svt_opt,
                                           payload))
      << "Payload: " << payload.name;
}

INSTANTIATE_TEST_SUITE_P(StandardPayloads, ElfComparisonTest,
                         testing::ValuesIn(GetStandardPayloads()),
                         [](const testing::TestParamInfo<ElfPayload>& info) {
                           return absl::StrCat("Payload_", info.index);
                         });

INSTANTIATE_TEST_SUITE_P(ExtendedSyntheticPayloads, ElfComparisonTest,
                         testing::ValuesIn(GetExtendedSyntheticPayloads()),
                         [](const testing::TestParamInfo<ElfPayload>& info) {
                           return absl::StrCat("Extended_", info.index);
                         });

}  // namespace
}  // namespace elfio_migration
