#include "tests/payloads.h"

#include <cstddef>
#include <cstdint>
#include <ios>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "devtools/build/runtime/get_runfiles_dir.h"
#include "file/base/helpers.h"
#include "file/base/options.h"
#include "file/base/path.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_format.h"
#include "absl/strings/str_join.h"
#include "absl/strings/string_view.h"
#include "third_party/elfio/elfio/elf_types.hpp"
#include "third_party/elfio/elfio/elfio.hpp"
#include "third_party/elfio/elfio/elfio_section.hpp"
#include "third_party/elfio/elfio/elfio_segment.hpp"
#include "third_party/elfio/elfio/elfio_strings.hpp"
#include "third_party/elfio/elfio/elfio_symbols.hpp"

namespace elfio_migration {

std::string MakeDeterministicBytes(size_t size, uint64_t seed) {
  std::string out(size, '\0');
  uint64_t state = seed ^ 0x9E3779B97F4A7C15ULL;
  for (size_t i = 0; i < size; ++i) {
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    out[i] = static_cast<char>((state * 0x2545F4914F6CDD1DULL) & 0xFF);
  }
  return out;
}

ElfPayload BuildSyntheticElf(SyntheticElfParams params) {
  QCHECK(params.elf_class == ELFIO::ELFCLASS32 ||
         params.elf_class == ELFIO::ELFCLASS64)
      << "Unsupported ELF class: " << static_cast<int>(params.elf_class);
  QCHECK(params.encoding == ELFIO::ELFDATA2LSB ||
         params.encoding == ELFIO::ELFDATA2MSB)
      << "Unsupported ELF encoding: " << static_cast<int>(params.encoding);
  QCHECK_GE(params.num_alloc_sections, 0);
  QCHECK_GE(params.num_named_blob_sections, 0);
  QCHECK_GE(params.num_symbols, 0);
  // Reserve headroom below ELFIO::SHN_LORESERVE (0xff00) for mandatory and
  // optional sections (.shstrtab, .bss, metadata, .strtab, .symtab).
  QCHECK_LT(params.num_alloc_sections + params.num_named_blob_sections + 6,
            ELFIO::SHN_LORESERVE);
  if (params.num_symbols > 0) {
    QCHECK_GT(params.num_alloc_sections, 0)
        << "Synthetic symbols require at least one allocatable section.";
  }

  ELFIO::elfio writer;
  writer.create(params.elf_class, params.encoding);
  writer.set_os_abi(ELFIO::ELFOSABI_LINUX);
  writer.set_type(params.elf_type);
  writer.set_machine(params.machine);
  writer.set_entry(params.entry);

  std::vector<std::string> target_sections;
  target_sections.reserve(
      static_cast<size_t>(params.num_alloc_sections) +
      (params.include_bss ? 1 : 0) +
      (params.num_named_blob_sections > 0
           ? 1 + static_cast<size_t>(params.num_named_blob_sections)
           : 0) +
      (params.num_symbols > 0 ? 2 : 0));
  ELFIO::Elf64_Addr current_vaddr = params.entry;
  constexpr ELFIO::Elf_Xword kPageAlign = 0x1000;

  // 1. Create allocatable sections (.text, .rodata, .data, and module sections)
  // and wrap each in a PT_LOAD segment (matching darwinn/kelvin/tensor_profiler
  // patterns).
  std::vector<ELFIO::section*> alloc_sections;
  alloc_sections.reserve(params.num_alloc_sections);
  for (int i = 0; i < params.num_alloc_sections; ++i) {
    std::string sec_name;
    ELFIO::Elf_Xword sec_flags = ELFIO::SHF_ALLOC;
    ELFIO::Elf_Word seg_flags = ELFIO::PF_R;
    if (i == 0) {
      sec_name = ".text";
      sec_flags |= ELFIO::SHF_EXECINSTR;
      seg_flags |= ELFIO::PF_X;
    } else if (i == 1) {
      sec_name = ".rodata";
    } else if (i == 2) {
      sec_name = ".data";
      sec_flags |= ELFIO::SHF_WRITE;
      seg_flags |= ELFIO::PF_W;
    } else {
      sec_name = absl::StrFormat(".text.mod_%d", i);
      sec_flags |= ELFIO::SHF_EXECINSTR;
      seg_flags |= ELFIO::PF_X;
    }

    ELFIO::section* sec = writer.sections.add(sec_name);
    sec->set_type(ELFIO::SHT_PROGBITS);
    sec->set_flags(sec_flags);
    sec->set_addr_align(16);
    sec->set_address(current_vaddr);
    std::string payload = MakeDeterministicBytes(
        params.alloc_section_size, static_cast<uint64_t>(i + 1) * 101ULL);
    sec->set_data(payload.data(), static_cast<ELFIO::Elf_Word>(payload.size()));
    alloc_sections.push_back(sec);
    target_sections.push_back(std::move(sec_name));

    ELFIO::segment* seg = writer.segments.add();
    seg->set_type(ELFIO::PT_LOAD);
    seg->set_virtual_address(current_vaddr);
    seg->set_physical_address(current_vaddr);
    seg->set_flags(seg_flags);
    seg->set_align(kPageAlign);
    seg->add_section_index(sec->get_index(), sec->get_addr_align());

    current_vaddr +=
        ((params.alloc_section_size + kPageAlign - 1) / kPageAlign) *
        kPageAlign;
  }

  // Optional .bss (SHT_NOBITS) section to exercise SHT_NOBITS handling.
  if (params.include_bss) {
    ELFIO::section* bss = writer.sections.add(".bss");
    bss->set_type(ELFIO::SHT_NOBITS);
    bss->set_flags(ELFIO::SHF_ALLOC | ELFIO::SHF_WRITE);
    bss->set_addr_align(16);
    bss->set_address(current_vaddr);
    bss->set_size(1024);
    target_sections.push_back(".bss");
  }

  // 2. Create named metadata + firmware image sections (matching libgqdiag's
  // pkg_parser pattern: "metadata" JSON + named binary image sections).
  if (params.num_named_blob_sections > 0) {
    ELFIO::section* meta = writer.sections.add("metadata");
    meta->set_type(ELFIO::SHT_PROGBITS);
    meta->set_addr_align(1);
    std::vector<std::string> image_entries;
    image_entries.reserve(params.num_named_blob_sections);
    for (int i = 0; i < params.num_named_blob_sections; ++i) {
      image_entries.push_back(absl::StrFormat("\"fw_img_%d\"", i));
    }
    std::string meta_json =
        absl::StrFormat(R"({"package_version":"1.0","images":[%s]})",
                        absl::StrJoin(image_entries, ","));
    meta->set_data(meta_json.data(),
                   static_cast<ELFIO::Elf_Word>(meta_json.size()));
    target_sections.push_back("metadata");

    for (int i = 0; i < params.num_named_blob_sections; ++i) {
      std::string blob_name = absl::StrFormat("fw_img_%d", i);
      ELFIO::section* blob_sec = writer.sections.add(blob_name);
      blob_sec->set_type(ELFIO::SHT_PROGBITS);
      blob_sec->set_addr_align(4);
      std::string blob_data =
          MakeDeterministicBytes(params.named_blob_section_size,
                                 static_cast<uint64_t>(i + 1) * 997ULL);
      blob_sec->set_data(blob_data.data(),
                         static_cast<ELFIO::Elf_Word>(blob_data.size()));
      target_sections.push_back(std::move(blob_name));
    }
  }

  // 3. Create .strtab and .symtab with `params.num_symbols` symbols (matching
  // tensor_profiler / postmark / mpact_sim symbol table extraction patterns).
  if (params.num_symbols > 0) {
    ELFIO::section* str_sec = writer.sections.add(".strtab");
    str_sec->set_type(ELFIO::SHT_STRTAB);
    str_sec->set_addr_align(1);
    ELFIO::string_section_accessor str_writer(str_sec);
    target_sections.push_back(".strtab");

    ELFIO::section* sym_sec = writer.sections.add(".symtab");
    sym_sec->set_type(ELFIO::SHT_SYMTAB);
    sym_sec->set_info(1);
    sym_sec->set_link(str_sec->get_index());
    sym_sec->set_addr_align(8);
    sym_sec->set_entry_size(writer.get_default_entry_size(ELFIO::SHT_SYMTAB));
    ELFIO::symbol_section_accessor sym_writer(writer, sym_sec);
    target_sections.push_back(".symtab");

    for (int i = 0; i < params.num_symbols; ++i) {
      ELFIO::section* target_sec =
          alloc_sections[static_cast<size_t>(i) % alloc_sections.size()];
      unsigned char bind = (i % 4 == 0) ? ELFIO::STB_LOCAL : ELFIO::STB_GLOBAL;
      unsigned char type = (i % 3 == 0) ? ELFIO::STT_OBJECT : ELFIO::STT_FUNC;
      std::string sym_name = (type == ELFIO::STT_OBJECT)
                                 ? absl::StrFormat("symbol_obj_%04d", i)
                                 : absl::StrFormat("symbol_fn_%04d", i);
      ELFIO::Elf64_Addr sym_offset =
          (params.alloc_section_size > 0)
              ? (static_cast<ELFIO::Elf64_Addr>(i) * 16) %
                    params.alloc_section_size
              : 0;
      ELFIO::Elf64_Addr sym_addr = target_sec->get_address() + sym_offset;
      ELFIO::Elf_Xword sym_size = 16 + (i % 64);
      sym_writer.add_symbol(str_writer, sym_name.c_str(), sym_addr, sym_size,
                            bind, type, ELFIO::STV_DEFAULT,
                            target_sec->get_index());
    }
    sym_writer.arrange_local_symbols();
  }

  std::ostringstream oss(std::ios::out | std::ios::binary);
  QCHECK(writer.save(oss));
  return ElfPayload{
      .name = std::move(params.label),
      .bytes = std::move(oss).str(),
      .file_path = "",
      .target_section_names = std::move(target_sections),
  };
}

absl::StatusOr<ElfPayload> LoadRealTestdataElf(absl::string_view filename,
                                               absl::string_view label) {
  std::string path = devtools_build::GetDataDependencyFilepath(file::JoinPath(
      "google3/security/ise_memory_safety/elf/testdata", filename));
  std::string contents;
  absl::Status status = file::GetContents(path, &contents, file::Defaults());
  if (!status.ok()) {
    return status;
  }
  return ElfPayload{
      .name = std::string(label),
      .bytes = std::move(contents),
      .file_path = std::move(path),
      .target_section_names = {".text", ".rodata", ".data", ".symtab",
                               ".strtab", ".gnu.version", ".gnu.version_r"},
  };
}

std::vector<ElfPayload> GetStandardPayloads() {
  std::vector<ElfPayload> payloads;
  payloads.reserve(9);

  // 1. Real compiled ELF test binaries from //security/ise_memory_safety/elf.
  constexpr std::pair<absl::string_view, absl::string_view> kRealTestFiles[] = {
      {"elf_cc.elf", "Real ELF64 x86-64 (elf_cc.elf)"},
      {"elf_cc_32.elf", "Real ELF32 x86 (elf_cc_32.elf)"},
      {"elf_cc_sysv.elf", "Real ELF64 SysV (elf_cc_sysv.elf)"},
  };
  for (const auto& [file, label] : kRealTestFiles) {
    absl::StatusOr<ElfPayload> loaded = LoadRealTestdataElf(file, label);
    QCHECK_OK(loaded.status());
    payloads.push_back(*std::move(loaded));
  }

  // 2. Small Firmware Package (libgqdiag pattern: ~32 KB, metadata + 3 images).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Small Firmware ELF64 (4 alloc, 3 fw blobs, 32 syms)",
      .elf_class = ELFIO::ELFCLASS64,
      .encoding = ELFIO::ELFDATA2LSB,
      .elf_type = ELFIO::ET_EXEC,
      .machine = ELFIO::EM_RISCV,
      .entry = 0x80000000,
      .num_alloc_sections = 4,
      .alloc_section_size = 2048,
      .num_named_blob_sections = 3,
      .named_blob_section_size = 4096,
      .num_symbols = 32,
      .include_bss = true,
  }));

  // 3. Medium Embedded Binary (darwinn / kelvin / tensor_profiler pattern:
  // ~272 KB, 16 alloc sections, 256 symbols).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Medium Embedded ELF64 (16 alloc, 4 fw blobs, 256 syms)",
      .elf_class = ELFIO::ELFCLASS64,
      .encoding = ELFIO::ELFDATA2LSB,
      .elf_type = ELFIO::ET_EXEC,
      .machine = ELFIO::EM_X86_64,
      .entry = 0x401000,
      .num_alloc_sections = 16,
      .alloc_section_size = 12288,
      .num_named_blob_sections = 4,
      .named_blob_section_size = 16384,
      .num_symbols = 256,
      .include_bss = true,
  }));

  // 4. Symbol-Heavy Profiler Binary (tensor_profiler / postmark pattern:
  // ~915 KB, 24 alloc sections, 2000 symbols).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Symbol-Heavy ELF64 (24 alloc, 2000 syms)",
      .elf_class = ELFIO::ELFCLASS64,
      .encoding = ELFIO::ELFDATA2LSB,
      .elf_type = ELFIO::ET_DYN,
      .machine = ELFIO::EM_X86_64,
      .entry = 0x10000,
      .num_alloc_sections = 24,
      .alloc_section_size = 32768,
      .num_named_blob_sections = 2,
      .named_blob_section_size = 32768,
      .num_symbols = 2000,
      .include_bss = true,
  }));

  // 5. Large Firmware Package (libgqdiag multi-MB package pattern:
  // ~4 MB, 8 named firmware blobs of 512 KB each).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Large Firmware ELF64 (4 alloc, 8x512KB blobs, 64 syms)",
      .elf_class = ELFIO::ELFCLASS64,
      .encoding = ELFIO::ELFDATA2LSB,
      .elf_type = ELFIO::ET_EXEC,
      .machine = ELFIO::EM_RISCV,
      .entry = 0x80000000,
      .num_alloc_sections = 4,
      .alloc_section_size = 16384,
      .num_named_blob_sections = 8,
      .named_blob_section_size = 512 * 1024,
      .num_symbols = 64,
      .include_bss = true,
  }));

  // 6. 32-bit little-endian synthetic ELF (ELFCLASS32, EM_386).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Synthetic ELF32 Little-Endian x86",
      .elf_class = ELFIO::ELFCLASS32,
      .encoding = ELFIO::ELFDATA2LSB,
      .elf_type = ELFIO::ET_EXEC,
      .machine = ELFIO::EM_386,
      .entry = 0x08048000,
      .num_alloc_sections = 4,
      .alloc_section_size = 1024,
      .num_named_blob_sections = 2,
      .named_blob_section_size = 2048,
      .num_symbols = 32,
      .include_bss = true,
  }));

  // 7. 64-bit big-endian synthetic ELF (ELFCLASS64, ELFDATA2MSB, EM_PPC64).
  payloads.push_back(BuildSyntheticElf(SyntheticElfParams{
      .label = "Synthetic ELF64 Big-Endian PPC64",
      .elf_class = ELFIO::ELFCLASS64,
      .encoding = ELFIO::ELFDATA2MSB,
      .elf_type = ELFIO::ET_EXEC,
      .machine = ELFIO::EM_PPC64,
      .entry = 0x10000000,
      .num_alloc_sections = 4,
      .alloc_section_size = 1024,
      .num_named_blob_sections = 2,
      .named_blob_section_size = 2048,
      .num_symbols = 32,
      .include_bss = true,
  }));

  return payloads;
}

}  // namespace elfio_migration
