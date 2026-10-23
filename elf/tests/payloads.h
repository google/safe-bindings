#ifndef SAFE_BINDINGS_ELF_TESTS_PAYLOADS_H_
#define SAFE_BINDINGS_ELF_TESTS_PAYLOADS_H_

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "third_party/elfio/elfio/elf_types.hpp"

namespace elfio_migration {

// Parameters controlling synthetic ELF binary generation so that both
// third_party/elfio (Memory-Unsafe) and security/ise_memory_safety/elf (Safe
// Alternative) are evaluated on identical ELF structures and byte payloads.
struct SyntheticElfParams {
  std::string label;
  unsigned char elf_class = ELFIO::ELFCLASS64;
  unsigned char encoding = ELFIO::ELFDATA2LSB;
  ELFIO::Elf_Half elf_type = ELFIO::ET_EXEC;
  ELFIO::Elf_Half machine = ELFIO::EM_X86_64;
  ELFIO::Elf64_Addr entry = 0x401000;
  int num_alloc_sections = 4;
  ELFIO::Elf_Word alloc_section_size = 4096;
  int num_named_blob_sections = 3;
  ELFIO::Elf_Word named_blob_section_size = 4096;
  // Number of synthetic symbols to emit into `.symtab` and `.strtab`. Requires
  // `num_alloc_sections > 0` when positive.
  int num_symbols = 32;
  bool include_bss = true;
};

struct ElfPayload {
  std::string name;
  std::string bytes;
  std::string file_path;  // Populated when backed by a runfile on disk.
  std::vector<std::string> target_section_names;
  bool expect_symbol_versioning = false;
};

// Generates deterministic pseudo-random bytes for section contents.
std::string MakeDeterministicBytes(size_t size, uint64_t seed);

// Builds an in-memory ELF binary according to `params` using ELFIO's writer.
// Terminates via `QCHECK` if `params` specifies an unsupported `elf_class` or
// `encoding`, negative or out-of-range section/symbol counts, positive
// `num_symbols` with zero `num_alloc_sections`, or if ELF serialization fails.
ElfPayload BuildSyntheticElf(SyntheticElfParams params);

// Loads a real compiled ELF binary from `filename` relative to
// `//security/ise_memory_safety/elf/testdata`. Returns a non-OK `absl::Status`
// if the runfile cannot be read.
absl::StatusOr<ElfPayload> LoadRealTestdataElf(absl::string_view filename,
                                               absl::string_view label);

// Returns the standard suite of 9 ELF payloads (3 real compiled testdata
// binaries and 6 synthetic binaries) used by both the benchmark and the
// comparison tests. Terminates via `QCHECK_OK` if any real testdata binary
// cannot be loaded.
std::vector<ElfPayload> GetStandardPayloads();

}  // namespace elfio_migration

#endif  // SAFE_BINDINGS_ELF_TESTS_PAYLOADS_H_
