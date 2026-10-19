#include "tests/payloads.h"

#include <cstddef>
#include <ios>
#include <sstream>
#include <string>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/strings/string_view.h"
#include "third_party/elfio/elfio/elf_types.hpp"
#include "third_party/elfio/elfio/elfio.hpp"
#include "third_party/elfio/elfio/elfio_section.hpp"
#include "third_party/elfio/elfio/elfio_symbols.hpp"

namespace elfio_migration {
namespace {

using ::testing::AnyOf;
using ::testing::Contains;
using ::testing::ElementsAre;
using ::testing::IsEmpty;
using ::testing::Not;
using ::testing::StartsWith;
using ::testing::status::StatusIs;

TEST(PayloadsTest, MakeDeterministicBytesIsReproducibleAndSeedDependent) {
  EXPECT_THAT(MakeDeterministicBytes(0, 42), IsEmpty());

  std::string first = MakeDeterministicBytes(64, 42);
  std::string second = MakeDeterministicBytes(64, 42);
  std::string different_seed = MakeDeterministicBytes(64, 99);

  EXPECT_EQ(first.size(), 64);
  EXPECT_EQ(first, second);
  EXPECT_NE(first, different_seed);
}

TEST(PayloadsTest, BuildSyntheticElfPopulatesBssAndDynamicMetadataJson) {
  ElfPayload payload = BuildSyntheticElf(SyntheticElfParams{
      .label = "Test Firmware ELF",
      .num_alloc_sections = 4,
      .alloc_section_size = 512,
      .num_named_blob_sections = 3,
      .named_blob_section_size = 256,
      .num_symbols = 0,
      .include_bss = true,
  });

  EXPECT_THAT(payload.target_section_names,
              ElementsAre(".text", ".rodata", ".data", ".text.mod_3", ".bss",
                          "metadata", "fw_img_0", "fw_img_1", "fw_img_2"));

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  ASSERT_TRUE(reader.load(stream, /*is_lazy=*/false));

  const ELFIO::section* mod_sec = reader.sections[".text.mod_3"];
  ASSERT_NE(mod_sec, nullptr);
  EXPECT_EQ(mod_sec->get_type(), ELFIO::SHT_PROGBITS);
  EXPECT_EQ(mod_sec->get_flags(), ELFIO::SHF_ALLOC | ELFIO::SHF_EXECINSTR);

  const ELFIO::section* bss_sec = reader.sections[".bss"];
  ASSERT_NE(bss_sec, nullptr);
  EXPECT_EQ(bss_sec->get_type(), ELFIO::SHT_NOBITS);
  EXPECT_EQ(bss_sec->get_size(), 1024);

  const ELFIO::section* meta_sec = reader.sections["metadata"];
  ASSERT_NE(meta_sec, nullptr);
  ASSERT_NE(meta_sec->get_data(), nullptr);
  absl::string_view meta_json(meta_sec->get_data(), meta_sec->get_size());
  EXPECT_EQ(
      meta_json,
      R"({"package_version":"1.0","images":["fw_img_0","fw_img_1","fw_img_2"]})");
}

TEST(PayloadsTest,
     BuildSyntheticElfPartitionsLocalSymbolsAndHandlesZeroAllocSize) {
  ElfPayload payload = BuildSyntheticElf(SyntheticElfParams{
      .label = "Zero Alloc Size Symbol Test",
      .num_alloc_sections = 2,
      .alloc_section_size = 0,
      .num_named_blob_sections = 0,
      .num_symbols = 8,
      .include_bss = false,
  });

  EXPECT_THAT(payload.target_section_names,
              ElementsAre(".text", ".rodata", ".strtab", ".symtab"));

  std::istringstream stream(payload.bytes, std::ios::in | std::ios::binary);
  ELFIO::elfio reader;
  ASSERT_TRUE(reader.load(stream, /*is_lazy=*/false));

  const ELFIO::section* sym_sec = reader.sections[".symtab"];
  ASSERT_NE(sym_sec, nullptr);
  // Symbol 0 is the null symbol, plus i = 0 and i = 4 are STB_LOCAL (total 3
  // local symbols before the first non-local symbol at index 3).
  ASSERT_EQ(sym_sec->get_info(), 3);

  ELFIO::const_symbol_section_accessor symbols(reader, sym_sec);
  ASSERT_EQ(symbols.get_symbols_num(), 9);

  for (ELFIO::Elf_Xword idx = 1; idx < sym_sec->get_info(); ++idx) {
    std::string name;
    ELFIO::Elf64_Addr value = 0;
    ELFIO::Elf_Xword size = 0;
    unsigned char bind = 0;
    unsigned char type = 0;
    ELFIO::Elf_Half section_index = 0;
    unsigned char other = 0;
    ASSERT_TRUE(symbols.get_symbol(idx, name, value, size, bind, type,
                                   section_index, other))
        << "at symbol index " << idx;

    EXPECT_EQ(bind, ELFIO::STB_LOCAL) << "at symbol index " << idx;
    EXPECT_THAT(type, AnyOf(ELFIO::STT_OBJECT, ELFIO::STT_FUNC))
        << "at symbol index " << idx;
    EXPECT_THAT(name, StartsWith(type == ELFIO::STT_OBJECT ? "symbol_obj_"
                                                           : "symbol_fn_"))
        << "at symbol index " << idx;

    const ELFIO::section* target_sec = reader.sections[section_index];
    ASSERT_NE(target_sec, nullptr) << "at symbol index " << idx;
    EXPECT_EQ(value, target_sec->get_address()) << "at symbol index " << idx;
  }

  for (ELFIO::Elf_Xword idx = sym_sec->get_info();
       idx < symbols.get_symbols_num(); ++idx) {
    std::string name;
    ELFIO::Elf64_Addr value = 0;
    ELFIO::Elf_Xword size = 0;
    unsigned char bind = 0;
    unsigned char type = 0;
    ELFIO::Elf_Half section_index = 0;
    unsigned char other = 0;
    ASSERT_TRUE(symbols.get_symbol(idx, name, value, size, bind, type,
                                   section_index, other))
        << "at symbol index " << idx;

    EXPECT_EQ(bind, ELFIO::STB_GLOBAL) << "at symbol index " << idx;
    EXPECT_THAT(type, AnyOf(ELFIO::STT_OBJECT, ELFIO::STT_FUNC))
        << "at symbol index " << idx;
    EXPECT_THAT(name, StartsWith(type == ELFIO::STT_OBJECT ? "symbol_obj_"
                                                           : "symbol_fn_"))
        << "at symbol index " << idx;

    const ELFIO::section* target_sec = reader.sections[section_index];
    ASSERT_NE(target_sec, nullptr) << "at symbol index " << idx;
    EXPECT_EQ(value, target_sec->get_address()) << "at symbol index " << idx;
  }
}

TEST(PayloadsTest, LoadRealTestdataElfLoadsValidBinaryAndFailsOnMissingFile) {
  ASSERT_OK_AND_ASSIGN(ElfPayload payload,
                       LoadRealTestdataElf("elf_cc.elf", "Real ELF64"));
  EXPECT_EQ(payload.name, "Real ELF64");
  EXPECT_THAT(payload.bytes, Not(IsEmpty()));
  EXPECT_THAT(payload.file_path, Not(IsEmpty()));
  EXPECT_THAT(payload.target_section_names, Contains(".gnu.version"));
  EXPECT_THAT(payload.target_section_names, Contains(".gnu.version_r"));

  EXPECT_THAT(LoadRealTestdataElf("non_existent_binary.elf", "Missing"),
              StatusIs(absl::StatusCode::kNotFound));
}

TEST(PayloadsTest, GetStandardPayloadsReturnsAllNineExpectedPayloads) {
  std::vector<ElfPayload> payloads = GetStandardPayloads();
  ASSERT_EQ(payloads.size(), 9);

  for (size_t i = 0; i < 3; ++i) {
    EXPECT_THAT(payloads[i].bytes, Not(IsEmpty())) << "at index " << i;
    EXPECT_THAT(payloads[i].file_path, Not(IsEmpty())) << "at index " << i;
  }
  for (size_t i = 3; i < payloads.size(); ++i) {
    EXPECT_THAT(payloads[i].bytes, Not(IsEmpty())) << "at index " << i;
    EXPECT_THAT(payloads[i].file_path, IsEmpty()) << "at index " << i;
  }

  std::istringstream stream32(payloads[7].bytes,
                              std::ios::in | std::ios::binary);
  ELFIO::elfio reader32;
  ASSERT_TRUE(reader32.load(stream32, /*is_lazy=*/true));
  EXPECT_EQ(reader32.get_class(), ELFIO::ELFCLASS32);
  EXPECT_EQ(reader32.get_encoding(), ELFIO::ELFDATA2LSB);

  std::istringstream stream_be(payloads[8].bytes,
                               std::ios::in | std::ios::binary);
  ELFIO::elfio reader_be;
  ASSERT_TRUE(reader_be.load(stream_be, /*is_lazy=*/true));
  EXPECT_EQ(reader_be.get_class(), ELFIO::ELFCLASS64);
  EXPECT_EQ(reader_be.get_encoding(), ELFIO::ELFDATA2MSB);
}

}  // namespace
}  // namespace elfio_migration
