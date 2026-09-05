#include <string>
#include <utility>

#include "flate2.h"
#include "gzip_wrapper.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/strings/cord.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"

namespace security::deflate {
namespace {

std::string CreateSampleCompressedGzip() {
  std::string input =
      absl::StrCat("Hello FFI Memory Safety! ", std::string(200, 'X'));
  auto wrapper = CompressGzip(input, 6);
  CHECK_OK(wrapper.status());
  return std::string(std::move(wrapper).value().as_string_view());
}

// ============================================================================
// PROOFS OF ABSENCE: Baseline Safe Lifecycle Invariants (Enabled)
// ============================================================================

TEST(DeflateMemorySafetyTest, Proof_Of_Absence_Lvalue_VecU8Wrapper_Safe) {
  std::string compressed = CreateSampleCompressedGzip();
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decompressed_wrapper,
                       UncompressGzip(compressed));

  absl::string_view view = decompressed_wrapper.as_string_view();
  EXPECT_FALSE(view.empty());
  EXPECT_THAT(view, testing::StartsWith("Hello FFI Memory Safety!"));
}

TEST(DeflateMemorySafetyTest, Proof_Of_Absence_Rvalue_Cord_Extraction_Safe) {
  std::string compressed = CreateSampleCompressedGzip();
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decompressed_wrapper,
                       UncompressGzip(compressed));

  absl::Cord cord = std::move(decompressed_wrapper).as_cord();
  EXPECT_FALSE(cord.empty());
  EXPECT_THAT(std::string(cord),
              testing::StartsWith("Hello FFI Memory Safety!"));
}

TEST(DeflateMemorySafetyTest, Proof_Of_Absence_ReadDecoder_Lvalue_Safe) {
  std::string compressed = CreateSampleCompressedGzip();
  auto decoder = read::GzDecoder::create(compressed);
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper wrapper, decoder.read_to_end());

  absl::string_view view = wrapper.as_string_view();
  EXPECT_FALSE(view.empty());
  EXPECT_THAT(view, testing::StartsWith("Hello FFI Memory Safety!"));
}

// ============================================================================
// PASS 1: REPRODUCIBLE MEMORY SAFETY HAZARDS (Disabled Under Baseline CI)
// ============================================================================

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wreturn-stack-address"
#pragma clang diagnostic ignored "-Wdangling-gsl"

TEST(DeflateMemorySafetyTest,
     DISABLED_Hazard_H3_UncompressGzip_TemporaryChaining_UAF) {
  std::string compressed = CreateSampleCompressedGzip();

  absl::string_view dangling_view;
  {
    auto get_chained_view = [&]() -> absl::string_view {
      return UncompressGzip(compressed)->as_string_view();
    };
    dangling_view = get_chained_view();
  }

  volatile char c = dangling_view[0];
  (void)c;
}

TEST(DeflateMemorySafetyTest,
     DISABLED_Hazard_H3_CompressGzip_TemporaryChaining_UAF) {
  std::string input = "Temporary Chaining Compress Hazard Payload";

  absl::string_view dangling_view;
  {
    auto get_chained_view = [&]() -> absl::string_view {
      return CompressGzip(input, 6)->as_string_view();
    };
    dangling_view = get_chained_view();
  }

  volatile char c = dangling_view[0];
  (void)c;
}

TEST(DeflateMemorySafetyTest,
     DISABLED_Hazard_H3_ReadGzDecoder_TemporaryChaining_UAF) {
  std::string compressed = CreateSampleCompressedGzip();
  auto decoder = read::GzDecoder::create(compressed);

  absl::string_view dangling_view;
  {
    auto get_chained_view = [&]() -> absl::string_view {
      return decoder.read_to_end()->as_string_view();
    };
    dangling_view = get_chained_view();
  }

  volatile char c = dangling_view[0];
  (void)c;
}

TEST(DeflateMemorySafetyTest,
     DISABLED_Hazard_H3_WriteGzEncoder_Finish_TemporaryChaining_UAF) {
  std::string input = "Write GzEncoder Finish Chaining Hazard";
  auto encoder = write::GzEncoder::create(Compression::best());
  ASSERT_OK(encoder.write_all(input));

  absl::string_view dangling_view;
  {
    auto get_chained_view = [&]() -> absl::string_view {
      return std::move(encoder).finish()->as_string_view();
    };
    dangling_view = get_chained_view();
  }

  volatile char c = dangling_view[0];
  (void)c;
}

#pragma clang diagnostic pop

}  // namespace
}  // namespace security::deflate
