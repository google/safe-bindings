// Benchmarks comparing the memory-unsafe C++ ELF library (//third_party/elfio)
// against the memory-safe Rust/Crubit C++ wrapper
// (//security/ise_memory_safety/elf:elf_bridge) with identical parameters and
// ELF payloads across representative Google3 consumer workloads.
//
// Compatible with go/benchy (`c++.google3` framework via
// //testing/base/public:gunit_main).
//
// Example benchy invocations:
//   1. Side-by-side execution of all Unsafe and Safe benchmarks:
//      benchy //security/ise_memory_safety/elf/tests:benchmark
//
//   2. Direct A/B benchstat delta table (Unsafe as old vs Safe as new):
//      benchy --reference=local \
//        --reference_arg=--bench_use_safe_elf=false \
//        --experiment_arg=--bench_use_safe_elf=true \
//        --benchmark_filter='^BM_AB_' \
//        //security/ise_memory_safety/elf/tests:benchmark

#include "testing/base/public/benchmark.h"

#include <array>
#include <cstddef>
#include <cstdint>
#include <ios>
#include <optional>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "dump.h"
#include "elf_bridge.h"
#include "tests/payloads.h"
#include <gtest/gtest.h>
#include "absl/base/no_destructor.h"
#include "absl/flags/flag.h"
#include "absl/log/check.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "third_party/elfio/elfio/elf_types.hpp"
#include "third_party/elfio/elfio/elfio.hpp"
#include "third_party/elfio/elfio/elfio_dump.hpp"
#include "third_party/elfio/elfio/elfio_section.hpp"
#include "third_party/elfio/elfio/elfio_segment.hpp"
#include "third_party/elfio/elfio/elfio_symbols.hpp"

ABSL_FLAG(bool, bench_use_safe_elf, false,
          "If true, BM_AB_* benchmarks execute security/ise_memory_safety/elf; "
          "otherwise they execute third_party/elfio.");

namespace {

using ::elfio_migration::ElfPayload;
using ::elfio_migration::GetStandardPayloads;

// Short, whitespace-free payload identifiers corresponding to the 9 entries
// returned by GetStandardPayloads(), suitable for benchy / benchstat output.
constexpr std::array<absl::string_view, 9> kPayloadArgNames = {
    "Real_ELF64_x86_64",         // 0: elf_cc.elf
    "Real_ELF32_x86",            // 1: elf_cc_32.elf
    "Real_ELF64_SysV",           // 2: elf_cc_sysv.elf
    "Small_Firmware_32KB",       // 3: Synthetic small firmware
    "Medium_Embedded_272KB",     // 4: Synthetic medium embedded
    "Symbol_Heavy_915KB",        // 5: Synthetic symbol-heavy
    "Large_Firmware_4MB",        // 6: Synthetic large firmware
    "Synthetic_ELF32_LE_x86",    // 7: Synthetic ELF32 little-endian x86
    "Synthetic_ELF64_BE_PPC64",  // 8: Synthetic ELF64 big-endian PPC64
};

// Number of leading entries in GetStandardPayloads() backed by real compiled
// ELF files on disk in //security/ise_memory_safety/elf:testdata_elfs.
constexpr size_t kRealFilePayloadCount = 3;

const std::vector<ElfPayload>& GetCachedPayloads() {
  static const absl::NoDestructor<std::vector<ElfPayload>> kPayloads(
      GetStandardPayloads());
  QCHECK_EQ(kPayloads->size(), kPayloadArgNames.size());
  for (size_t i = 0; i < kPayloads->size(); ++i) {
    QCHECK_EQ(!(*kPayloads)[i].file_path.empty(), i < kRealFilePayloadCount)
        << "at payload index " << i;
  }
  return *kPayloads;
}

const ElfPayload& GetPayloadForState(const benchmark::State& state) {
  const std::vector<ElfPayload>& payloads = GetCachedPayloads();
  int index = state.range(0);
  QCHECK_GE(index, 0);
  QCHECK_LT(static_cast<size_t>(index), payloads.size());
  return payloads[static_cast<size_t>(index)];
}

// Note: `BENCHMARK(...)->Apply(...)` callbacks execute during C++ static
// initialization before `InitGoogle()`, where `file::GetContents()` is
// disallowed (go/no_file_or_rpc_during_init). Registration therefore uses
// compile-time bounds and `GetCachedPayloads()` validates the invariants on
// first use after `InitGoogle()`.
void RegisterAllPayloads(testing::Benchmark* b) {
  for (size_t i = 0; i < kPayloadArgNames.size(); ++i) {
    b->Arg(static_cast<int>(i), std::string(kPayloadArgNames[i]));
  }
}

void RegisterFilePayloads(testing::Benchmark* b) {
  for (size_t i = 0; i < kRealFilePayloadCount; ++i) {
    b->Arg(static_cast<int>(i), std::string(kPayloadArgNames[i]));
  }
}

// Normalizes reported throughput across all benchmark workloads to the total
// input ELF binary size (`payload.bytes.size()`) per iteration so that
// side-by-side and A/B benchstat comparisons use a consistent byte basis.
void FinalizeThroughput(benchmark::State& state, const ElfPayload& payload) {
  state.SetBytesProcessed(static_cast<int64_t>(state.iterations()) *
                          static_cast<int64_t>(payload.bytes.size()));
}

// =============================================================================
// Workload 1: Named Section Lookup & Payload Read (libgqdiag pattern)
// Both libraries parse the same in-memory ELF payload and look up the same list
// of target section names, reading the section data bytes for each.
// =============================================================================
uint64_t RunUnsafeNamedSectionLookup(const ElfPayload& payload, bool is_lazy) {
  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  QCHECK(reader.load(stream, is_lazy));
  uint64_t checksum = reader.get_entry();
  for (const std::string& name : payload.target_section_names) {
    ELFIO::section* sec = reader.sections[name];
    if (sec != nullptr && sec->get_type() != ELFIO::SHT_NOBITS &&
        sec->get_size() > 0) {
      const char* section_bytes = sec->get_data();
      if (section_bytes != nullptr) {
        checksum ^= static_cast<uint64_t>(sec->get_size());
        checksum += static_cast<unsigned char>(section_bytes[0]);
        checksum +=
            static_cast<unsigned char>(section_bytes[sec->get_size() - 1]);
      }
    }
  }
  return checksum;
}

uint64_t RunSafeNamedSectionLookup(const ElfPayload& payload) {
  absl::StatusOr<elf_bridge::ElfBytes> elf =
      elf_bridge::ElfBytes::Parse(payload.bytes);
  QCHECK_OK(elf.status());
  absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
  QCHECK_OK(header.status());
  uint64_t checksum = header->GetEntry();
  for (const std::string& name : payload.target_section_names) {
    absl::StatusOr<std::optional<elf_bridge::SectionHeader>> section_header =
        elf->GetSectionHeaderByName(name);
    if (section_header.ok() && section_header->has_value()) {
      absl::StatusOr<elf_bridge::SectionType> section_type =
          (*section_header)->GetType();
      if (section_type.ok() &&
          *section_type != elf_bridge::SectionType::kNoBits &&
          (*section_header)->GetSize() > 0) {
        absl::StatusOr<elf_bridge::SectionData> section_data =
            elf->GetSectionData(**section_header);
        if (section_data.ok()) {
          absl::string_view section_bytes = section_data->GetData();
          if (!section_bytes.empty()) {
            checksum ^= static_cast<uint64_t>(section_bytes.size());
            checksum += static_cast<unsigned char>(section_bytes.front());
            checksum += static_cast<unsigned char>(section_bytes.back());
          }
        }
      }
    }
  }
  return checksum;
}

// =============================================================================
// Workload 2: Full Section & Symbol Table Extraction
// (tensor_profiler/trace_parser/memory & postmark pattern)
// Both libraries parse the same in-memory ELF payload, inspect the ELF header,
// iterate all SHF_ALLOC sections + read their payloads, and iterate all symbols
// in .symtab and .dynsym resolving each symbol's name, value, size, and type.
// =============================================================================
uint64_t RunUnsafeFullSectionAndSymbolExtraction(const ElfPayload& payload,
                                                 bool is_lazy) {
  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  QCHECK(reader.load(stream, is_lazy));
  uint64_t checksum = static_cast<uint64_t>(reader.get_type()) ^
                      static_cast<uint64_t>(reader.get_machine()) ^
                      reader.get_entry();

  uint64_t sym_checksum = 0;
  const ELFIO::Elf_Half num_secs = reader.sections.size();
  for (ELFIO::Elf_Half i = 0; i < num_secs; ++i) {
    const ELFIO::section* sec = reader.sections[i];
    if (sec == nullptr) continue;
    ELFIO::Elf_Word sec_type = sec->get_type();
    if ((sec->get_flags() & ELFIO::SHF_ALLOC) != 0) {
      checksum ^= sec->get_address() + sec->get_size();
      if (sec_type != ELFIO::SHT_NOBITS && sec->get_size() > 0) {
        const char* section_bytes = sec->get_data();
        if (section_bytes != nullptr) {
          checksum += static_cast<unsigned char>(section_bytes[0]);
        }
      }
    }
    if (sec_type == ELFIO::SHT_SYMTAB || sec_type == ELFIO::SHT_DYNSYM) {
      ELFIO::const_symbol_section_accessor syms(reader, sec);
      const ELFIO::Elf_Xword sym_num = syms.get_symbols_num();
      for (ELFIO::Elf_Xword j = 0; j < sym_num; ++j) {
        std::string name;
        ELFIO::Elf64_Addr value = 0;
        ELFIO::Elf_Xword size = 0;
        unsigned char bind = 0;
        unsigned char type = 0;
        ELFIO::Elf_Half section_index = 0;
        unsigned char other = 0;
        if (syms.get_symbol(j, name, value, size, bind, type, section_index,
                            other)) {
          sym_checksum ^=
              value + size + name.size() + type + bind + section_index;
        }
      }
    }
  }
  checksum ^= sym_checksum;
  return checksum;
}

uint64_t RunSafeFullSectionAndSymbolExtraction(const ElfPayload& payload) {
  absl::StatusOr<elf_bridge::ElfBytes> elf =
      elf_bridge::ElfBytes::Parse(payload.bytes);
  QCHECK_OK(elf.status());
  absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
  QCHECK_OK(header.status());
  absl::StatusOr<elf_bridge::ElfMachine> machine = header->GetMachine();
  uint64_t checksum =
      static_cast<uint64_t>(header->GetType()) ^
      (machine.ok() ? static_cast<uint64_t>(static_cast<int>(*machine))
                    : 0ULL) ^
      header->GetEntry();

  absl::StatusOr<std::optional<elf_bridge::SectionHeaderTable>> section_table =
      elf->GetSectionHeaders();
  if (section_table.ok() && section_table->has_value()) {
    const size_t num_secs = (*section_table)->size();
    for (size_t i = 0; i < num_secs; ++i) {
      absl::StatusOr<elf_bridge::SectionHeader> section_header =
          (*section_table)->GetSectionHeader(i);
      if (!section_header.ok()) continue;
      if ((section_header->GetFlags() & ELFIO::SHF_ALLOC) != 0) {
        checksum ^= section_header->GetAddress() + section_header->GetSize();
        absl::StatusOr<elf_bridge::SectionType> section_type =
            section_header->GetType();
        if (section_type.ok() &&
            *section_type != elf_bridge::SectionType::kNoBits &&
            section_header->GetSize() > 0) {
          absl::StatusOr<elf_bridge::SectionData> section_data =
              elf->GetSectionData(*section_header);
          if (section_data.ok() && !section_data->GetData().empty()) {
            checksum += static_cast<unsigned char>(section_data->GetData()[0]);
          }
        }
      }
    }
  }

  auto process_symtab = [&](const elf_bridge::SymbolTableWithStrings& sym_str) {
    const elf_bridge::SymbolTable& symtab = sym_str.GetSymbolTable();
    const elf_bridge::StringTable& strtab = sym_str.GetStringTable();
    const size_t num_symbols = symtab.size();
    for (size_t j = 0; j < num_symbols; ++j) {
      absl::StatusOr<elf_bridge::Symbol> symbol = symtab.GetSymbol(j);
      if (!symbol.ok()) continue;
      absl::StatusOr<std::string> symbol_name = strtab.Get(symbol->GetName());
      absl::StatusOr<elf_bridge::SymbolType> symbol_type = symbol->GetType();
      absl::StatusOr<elf_bridge::SymbolBind> symbol_bind = symbol->GetBinding();
      if (symbol_name.ok() && symbol_type.ok() && symbol_bind.ok()) {
        checksum ^= symbol->GetValue() + symbol->GetSize() +
                    symbol_name->size() +
                    static_cast<uint64_t>(static_cast<int>(*symbol_type)) +
                    static_cast<uint64_t>(static_cast<int>(*symbol_bind)) +
                    symbol->GetSectionHeaderIndex();
      }
    }
  };

  absl::StatusOr<std::optional<elf_bridge::SymbolTableWithStrings>> symtab =
      elf->GetSymbolTable();
  if (symtab.ok() && symtab->has_value()) {
    process_symtab(**symtab);
  }
  absl::StatusOr<std::optional<elf_bridge::SymbolTableWithStrings>> dynsym =
      elf->GetDynamicSymbolTable();
  if (dynsym.ok() && dynsym->has_value()) {
    process_symtab(**dynsym);
  }
  return checksum;
}

// =============================================================================
// Workload 3: Program Segment (PT_LOAD) Iteration & Payload Read
// (darwinn/simulator & kelvin/sim pattern)
// Both libraries parse the same in-memory ELF payload, iterate all program
// headers, filter PT_LOAD segments, and read the segment payload bytes.
// =============================================================================
uint64_t RunUnsafeSegmentLoad(const ElfPayload& payload, bool is_lazy) {
  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  QCHECK(reader.load(stream, is_lazy));
  uint64_t checksum = reader.get_entry();
  const ELFIO::Elf_Half seg_num = reader.segments.size();
  for (ELFIO::Elf_Half i = 0; i < seg_num; ++i) {
    const ELFIO::segment* seg = reader.segments[i];
    if (seg == nullptr || seg->get_type() != ELFIO::PT_LOAD) continue;
    ELFIO::Elf_Xword file_size = seg->get_file_size();
    checksum ^= seg->get_virtual_address() + seg->get_physical_address() +
                file_size + seg->get_memory_size();
    if (file_size > 0) {
      const char* segment_bytes = seg->get_data();
      if (segment_bytes != nullptr) {
        checksum += static_cast<unsigned char>(segment_bytes[0]);
        checksum += static_cast<unsigned char>(segment_bytes[file_size - 1]);
      }
    }
  }
  return checksum;
}

uint64_t RunSafeSegmentLoad(const ElfPayload& payload) {
  absl::StatusOr<elf_bridge::ElfBytes> elf =
      elf_bridge::ElfBytes::Parse(payload.bytes);
  QCHECK_OK(elf.status());
  absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
  QCHECK_OK(header.status());
  uint64_t checksum = header->GetEntry();
  absl::StatusOr<std::optional<elf_bridge::SegmentTable>> segments =
      elf->GetSegments();
  if (segments.ok() && segments->has_value()) {
    const size_t seg_num = (*segments)->size();
    for (size_t i = 0; i < seg_num; ++i) {
      absl::StatusOr<elf_bridge::ProgramHeader> program_header =
          (*segments)->GetProgramHeader(i);
      if (!program_header.ok()) continue;
      absl::StatusOr<elf_bridge::ProgramHeaderType> segment_type =
          program_header->GetType();
      if (!segment_type.ok() ||
          *segment_type != elf_bridge::ProgramHeaderType::kLoad) {
        continue;
      }
      uint64_t file_size = program_header->GetFileSize();
      checksum ^= program_header->GetVirtualAddress() +
                  program_header->GetPhysicalAddress() + file_size +
                  program_header->GetMemorySize();
      if (file_size > 0) {
        absl::StatusOr<std::string> segment_bytes =
            elf->GetSegmentData(*program_header);
        if (segment_bytes.ok() && !segment_bytes->empty()) {
          checksum += static_cast<unsigned char>(segment_bytes->front());
          checksum += static_cast<unsigned char>(segment_bytes->back());
        }
      }
    }
  }
  return checksum;
}

// =============================================================================
// Workload 4: File-Backed Streaming Parse (ElfStream::Open vs elfio::load)
// Both libraries open the same ELF file path on disk in lazy/streaming mode and
// read the ELF header, section headers, and symbol table.
// =============================================================================
uint64_t RunUnsafeFileStreamParse(const ElfPayload& payload) {
  QCHECK(!payload.file_path.empty());
  ELFIO::elfio reader;
  QCHECK(reader.load(payload.file_path, /*is_lazy=*/true));
  const ELFIO::Elf_Half num_secs = reader.sections.size();
  uint64_t checksum = reader.get_entry() ^ num_secs;
  for (ELFIO::Elf_Half i = 0; i < num_secs; ++i) {
    const ELFIO::section* sec = reader.sections[i];
    if (sec == nullptr) continue;
    checksum ^= sec->get_address() + sec->get_size();
    if (sec->get_type() == ELFIO::SHT_SYMTAB) {
      ELFIO::const_symbol_section_accessor syms(reader, sec);
      const ELFIO::Elf_Xword sym_num = syms.get_symbols_num();
      for (ELFIO::Elf_Xword j = 0; j < sym_num; ++j) {
        std::string name;
        ELFIO::Elf64_Addr value = 0;
        ELFIO::Elf_Xword size = 0;
        unsigned char bind = 0;
        unsigned char type = 0;
        ELFIO::Elf_Half section_index = 0;
        unsigned char other = 0;
        if (syms.get_symbol(j, name, value, size, bind, type, section_index,
                            other)) {
          checksum ^= value + size + name.size();
        }
      }
    }
  }
  return checksum;
}

uint64_t RunSafeFileStreamParse(const ElfPayload& payload) {
  QCHECK(!payload.file_path.empty());
  absl::StatusOr<elf_bridge::ElfStream> stream =
      elf_bridge::ElfStream::Open(payload.file_path);
  QCHECK_OK(stream.status());
  absl::StatusOr<elf_bridge::FileHeader> header = stream->GetFileHeader();
  QCHECK_OK(header.status());
  absl::StatusOr<elf_bridge::SectionHeaderTable> section_headers =
      stream->GetSectionHeaders();
  QCHECK_OK(section_headers.status());
  const size_t num_secs = section_headers->size();
  uint64_t checksum = header->GetEntry() ^ num_secs;
  for (size_t i = 0; i < num_secs; ++i) {
    absl::StatusOr<elf_bridge::SectionHeader> section_header =
        section_headers->GetSectionHeader(i);
    if (section_header.ok()) {
      checksum ^= section_header->GetAddress() + section_header->GetSize();
    }
  }
  absl::StatusOr<std::optional<elf_bridge::SymbolTableWithStrings>> symtab_opt =
      stream->GetSymbolTable();
  if (symtab_opt.ok() && symtab_opt->has_value()) {
    const elf_bridge::SymbolTable& symtab = (*symtab_opt)->GetSymbolTable();
    const elf_bridge::StringTable& strtab = (*symtab_opt)->GetStringTable();
    const size_t num_symbols = symtab.size();
    for (size_t j = 0; j < num_symbols; ++j) {
      absl::StatusOr<elf_bridge::Symbol> symbol = symtab.GetSymbol(j);
      if (!symbol.ok()) continue;
      absl::StatusOr<std::string> symbol_name = strtab.Get(symbol->GetName());
      if (symbol_name.ok()) {
        checksum ^=
            symbol->GetValue() + symbol->GetSize() + symbol_name->size();
      }
    }
  }
  return checksum;
}

// =============================================================================
// Workload 5: Human-Readable ELF Dump (eis pattern)
// Both libraries parse the same in-memory ELF payload and format the file
// header, section headers, segment headers, and symbol tables to an ostream.
// =============================================================================
size_t RunUnsafeDump(const ElfPayload& payload) {
  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  QCHECK(reader.load(stream, /*is_lazy=*/true));
  std::ostringstream out;
  ELFIO::dump::header(out, reader);
  ELFIO::dump::section_headers(out, reader);
  ELFIO::dump::segment_headers(out, reader);
  ELFIO::dump::symbol_tables(out, reader);
  return std::move(out).str().size();
}

size_t RunSafeDump(const ElfPayload& payload) {
  absl::StatusOr<elf_bridge::ElfBytes> elf =
      elf_bridge::ElfBytes::Parse(payload.bytes);
  QCHECK_OK(elf.status());
  std::ostringstream out;
  elf_bridge::DumpFileHeader(out, *elf);
  elf_bridge::DumpSectionHeaders(out, *elf);
  elf_bridge::DumpSegmentHeaders(out, *elf);
  elf_bridge::DumpSymbolTables(out, *elf);
  return std::move(out).str().size();
}

// =============================================================================
// Side-by-Side Benchmarks (BM_*_Unsafe* and BM_*_Safe)
// =============================================================================

void BM_NamedSectionLookup_UnsafeLazy(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunUnsafeNamedSectionLookup(payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_NamedSectionLookup_UnsafeLazy)->Apply(RegisterAllPayloads);

void BM_NamedSectionLookup_UnsafeEager(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunUnsafeNamedSectionLookup(payload, /*is_lazy=*/false);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_NamedSectionLookup_UnsafeEager)->Apply(RegisterAllPayloads);

void BM_NamedSectionLookup_Safe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunSafeNamedSectionLookup(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_NamedSectionLookup_Safe)->Apply(RegisterAllPayloads);

void BM_FullSectionAndSymbolExtraction_Unsafe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum =
        RunUnsafeFullSectionAndSymbolExtraction(payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_FullSectionAndSymbolExtraction_Unsafe)->Apply(RegisterAllPayloads);

void BM_FullSectionAndSymbolExtraction_Safe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunSafeFullSectionAndSymbolExtraction(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_FullSectionAndSymbolExtraction_Safe)->Apply(RegisterAllPayloads);

void BM_ProgramSegmentRead_Unsafe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunUnsafeSegmentLoad(payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_ProgramSegmentRead_Unsafe)->Apply(RegisterAllPayloads);

void BM_ProgramSegmentRead_Safe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunSafeSegmentLoad(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_ProgramSegmentRead_Safe)->Apply(RegisterAllPayloads);

void BM_FileStreamParse_Unsafe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunUnsafeFileStreamParse(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_FileStreamParse_Unsafe)->Apply(RegisterFilePayloads);

void BM_FileStreamParse_Safe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    uint64_t checksum = RunSafeFileStreamParse(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_FileStreamParse_Safe)->Apply(RegisterFilePayloads);

void BM_ElfDump_Unsafe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    size_t dump_bytes = RunUnsafeDump(payload);
    testing::DoNotOptimize(dump_bytes);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_ElfDump_Unsafe)->Apply(RegisterAllPayloads);

void BM_ElfDump_Safe(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  for (auto s : state) {
    size_t dump_bytes = RunSafeDump(payload);
    testing::DoNotOptimize(dump_bytes);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_ElfDump_Safe)->Apply(RegisterAllPayloads);

// =============================================================================
// Unified A/B Benchmarks (BM_AB_*) for benchy old-vs-new delta comparison
// Toggle implementation via --bench_use_safe_elf={false,true}.
// =============================================================================

void BM_AB_NamedSectionLookup(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  const bool use_safe = absl::GetFlag(FLAGS_bench_use_safe_elf);
  for (auto s : state) {
    uint64_t checksum =
        use_safe ? RunSafeNamedSectionLookup(payload)
                 : RunUnsafeNamedSectionLookup(payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_AB_NamedSectionLookup)->Apply(RegisterAllPayloads);

void BM_AB_FullSectionAndSymbolExtraction(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  const bool use_safe = absl::GetFlag(FLAGS_bench_use_safe_elf);
  for (auto s : state) {
    uint64_t checksum = use_safe
                            ? RunSafeFullSectionAndSymbolExtraction(payload)
                            : RunUnsafeFullSectionAndSymbolExtraction(
                                  payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_AB_FullSectionAndSymbolExtraction)->Apply(RegisterAllPayloads);

void BM_AB_ProgramSegmentRead(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  const bool use_safe = absl::GetFlag(FLAGS_bench_use_safe_elf);
  for (auto s : state) {
    uint64_t checksum = use_safe
                            ? RunSafeSegmentLoad(payload)
                            : RunUnsafeSegmentLoad(payload, /*is_lazy=*/true);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_AB_ProgramSegmentRead)->Apply(RegisterAllPayloads);

void BM_AB_FileStreamParse(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  const bool use_safe = absl::GetFlag(FLAGS_bench_use_safe_elf);
  for (auto s : state) {
    uint64_t checksum = use_safe ? RunSafeFileStreamParse(payload)
                                 : RunUnsafeFileStreamParse(payload);
    testing::DoNotOptimize(checksum);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_AB_FileStreamParse)->Apply(RegisterFilePayloads);

void BM_AB_ElfDump(benchmark::State& state) {
  const ElfPayload& payload = GetPayloadForState(state);
  const bool use_safe = absl::GetFlag(FLAGS_bench_use_safe_elf);
  for (auto s : state) {
    size_t dump_bytes =
        use_safe ? RunSafeDump(payload) : RunUnsafeDump(payload);
    testing::DoNotOptimize(dump_bytes);
  }
  FinalizeThroughput(state, payload);
}
BENCHMARK(BM_AB_ElfDump)->Apply(RegisterAllPayloads);

TEST(BenchmarkSmokeTest, InMemoryWorkloadsExecuteOnAllPayloads) {
  for (const ElfPayload& payload : GetCachedPayloads()) {
    EXPECT_EQ(RunUnsafeNamedSectionLookup(payload, /*is_lazy=*/true),
              RunSafeNamedSectionLookup(payload))
        << payload.name;
    EXPECT_EQ(RunUnsafeNamedSectionLookup(payload, /*is_lazy=*/false),
              RunSafeNamedSectionLookup(payload))
        << payload.name;
    EXPECT_EQ(RunUnsafeFullSectionAndSymbolExtraction(payload,
                                                      /*is_lazy=*/true),
              RunSafeFullSectionAndSymbolExtraction(payload))
        << payload.name;
    EXPECT_EQ(RunUnsafeSegmentLoad(payload, /*is_lazy=*/true),
              RunSafeSegmentLoad(payload))
        << payload.name;
    EXPECT_GT(RunUnsafeDump(payload), 0) << payload.name;
    EXPECT_GT(RunSafeDump(payload), 0) << payload.name;
  }
}

TEST(BenchmarkSmokeTest, FileStreamWorkloadsExecuteOnRealPayloads) {
  const std::vector<ElfPayload>& payloads = GetCachedPayloads();
  for (size_t i = 0; i < kRealFilePayloadCount; ++i) {
    ASSERT_FALSE(payloads[i].file_path.empty()) << payloads[i].name;
    EXPECT_EQ(RunUnsafeFileStreamParse(payloads[i]),
              RunSafeFileStreamParse(payloads[i]))
        << payloads[i].name;
  }
}

}  // namespace
