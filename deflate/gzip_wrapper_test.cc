#include "security/deflate/gzip_wrapper.h"

#include <string>
#include <utility>

#include "devtools/build/runtime/get_runfiles_dir.h"
#include "file/base/helpers.h"
#include "security/deflate/flate2.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "third_party/absl/log/check.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/strings/cord.h"
#include "third_party/absl/strings/str_cat.h"
#include "third_party/absl/strings/string_view.h"
#include "util/gzip/gzipstring.h"

namespace {

using ::security::deflate::VecU8Wrapper;
using ::testing::status::StatusIs;

std::string ReadTestFile(absl::string_view path) {
  const std::string full_path = devtools_build::GetDataDependencyFilepath(path);
  std::string content;
  CHECK_OK(file::GetContents(full_path, &content));
  return content;
}

TEST(GzipWrapperTest, GzipCompress) {
  std::string input = absl::StrCat("hello world", std::string(500, 'a'));
  ASSERT_OK_AND_ASSIGN(security::deflate::VecU8Wrapper compressed_wrapper,
                       security::deflate::CompressGzip(input, 9));
  absl::Cord compressed = std::move(compressed_wrapper).as_cord();
  EXPECT_GT(compressed.size(), 0);
  EXPECT_LT(compressed.size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipCordToString(compressed, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, GzipCompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  ASSERT_OK_AND_ASSIGN(security::deflate::VecU8Wrapper compressed_wrapper,
                       security::deflate::CompressGzip(input, 9));
  absl::Cord compressed = std::move(compressed_wrapper).as_cord();
  EXPECT_GT(compressed.size(), 0);
  EXPECT_LT(compressed.size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipCordToString(compressed, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, GzipIncompressible) {
  std::string original = "abc";

  absl::StatusOr<VecU8Wrapper> compressed_wrapper =
      security::deflate::CompressGzip(original, 6);

  ASSERT_OK(compressed_wrapper);
  absl::Cord output = (*std::move(compressed_wrapper)).as_cord();
  EXPECT_GT(output.size(), original.size());
}

TEST(GzipWrapperTest, GzipCompressFailBadCompressionLevel) {
  std::string original = absl::StrCat("abc", std::string(500, 'a'));

  absl::StatusOr<VecU8Wrapper> result =
      security::deflate::CompressGzip(original, -1);
  EXPECT_THAT(result, StatusIs(absl::StatusCode::kInvalidArgument));

  result = security::deflate::CompressGzip(original, 10);
  EXPECT_THAT(result, StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(GzipWrapperTest, GzipDecompressCord) {
  std::string input = absl::StrCat("abc", std::string(500, 'a'));
  absl::Cord compressed;

  ASSERT_TRUE(GzipStringToCord(input, &compressed));

  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decompressed_wrapper,
                       security::deflate::UncompressGzip(compressed.Flatten()));
  absl::Cord decompressed = std::move(decompressed_wrapper).as_cord();
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, GzipDecompressCordLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  absl::Cord compressed;

  ASSERT_TRUE(GzipStringToCord(input, &compressed));

  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decompressed_wrapper,
                       security::deflate::UncompressGzip(compressed.Flatten()));
  absl::Cord decompressed = std::move(decompressed_wrapper).as_cord();
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, GzipDecompressCordFailNonGzip) {
  absl::Cord compressed("not a gzipped string");

  absl::StatusOr<VecU8Wrapper> decompressed_wrapper =
      security::deflate::UncompressGzip(compressed.Flatten());
  EXPECT_THAT(decompressed_wrapper,
              StatusIs(absl::StatusCode::kInvalidArgument));
}
TEST(GzipWrapperTest, DecompressStringGzip) {
  std::string input = absl::StrCat("abc", std::string(500, 'a'));
  std::string compressed;

  GzipString(input, &compressed);

  ASSERT_OK_AND_ASSIGN(
      VecU8Wrapper decompressed_wrapper,
      security::deflate::UncompressGzip(absl::string_view(compressed)));

  std::string decompressed(decompressed_wrapper.as_string_view());
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, GzipDecompressStringLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;

  GzipString(input, &compressed);

  ASSERT_OK_AND_ASSIGN(
      VecU8Wrapper decompressed_wrapper,
      security::deflate::UncompressGzip(absl::string_view(compressed)));

  std::string decompressed(decompressed_wrapper.as_string_view());
  EXPECT_EQ(decompressed, input);
}

TEST(GzipWrapperTest, DecompressStringFailNonGzip) {
  std::string compressed = "not a gzipped string";

  absl::StatusOr<VecU8Wrapper> decompressed_wrapper =
      security::deflate::UncompressGzip(compressed);
  EXPECT_THAT(decompressed_wrapper,
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(GzipWrapperTest, DecompressStringFailTruncated) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;

  GzipString(input, &compressed);
  compressed = compressed.substr(0, compressed.size() / 2);
  absl::StatusOr<VecU8Wrapper> decompressed_wrapper =
      security::deflate::UncompressGzip(compressed);
  EXPECT_THAT(decompressed_wrapper,
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(GzipWrapperTest, CompressDecompressEmptyString) {
  std::string input = "";
  ASSERT_OK_AND_ASSIGN(security::deflate::VecU8Wrapper compressed_wrapper,
                       security::deflate::CompressGzip(input, 9));
  absl::Cord compressed = std::move(compressed_wrapper).as_cord();
  EXPECT_GT(compressed.size(), 0);
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decompressed_wrapper,
                       security::deflate::UncompressGzip(compressed.Flatten()));

  EXPECT_EQ(decompressed_wrapper.as_string_view(), input);
}

}  // namespace
