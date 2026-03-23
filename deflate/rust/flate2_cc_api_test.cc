#include <string>
#include <utility>

#include "base/rust/rust_vec_u8.h"
#include "devtools/build/runtime/get_runfiles_dir.h"
#include "file/base/helpers.h"
#include "security/deflate/rust/flate2_rs.h"
#include "security/ise_memory_safety/crubit_helpers/string_conversions.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "third_party/absl/log/check.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/strings/str_cat.h"
#include "third_party/absl/strings/string_view.h"
#include "util/gzip/gzipstring.h"

namespace {
using ::security::crubit_helpers::StringViewFromVecU8;
using ::testing::status::StatusIs;

std::string ReadTestFile(absl::string_view path) {
  const std::string full_path = devtools_build::GetDataDependencyFilepath(path);
  std::string content;
  CHECK_OK(file::GetContents(full_path, &content));
  return content;
}

absl::StatusOr<std::string> Flate2ReaderCompress(absl::string_view input) {
  auto encoder =
      flate2_rs::read::GzEncoder::create(input, flate2_rs::Compression::best());
  flate2_rs::ResultVecU8 encode_result = encoder.read_to_end();
  if (!encode_result.is_ok()) {
    return absl::InternalError(std::move(encode_result).unwrap_err());
  }
  rust_vec_u8::VecU8 flate_vec8 = std::move(encode_result).unwrap();
  return std::string(StringViewFromVecU8(flate_vec8));
}

absl::StatusOr<std::string> Flate2ReaderDecompress(
    absl::string_view compressed) {
  auto decoder = flate2_rs::read::GzDecoder::create(compressed);
  flate2_rs::ResultVecU8 decode_result = decoder.read_to_end();
  if (!decode_result.is_ok()) {
    return absl::InternalError(std::move(decode_result).unwrap_err());
  }
  rust_vec_u8::VecU8 decoded_vec8 = std::move(decode_result).unwrap();
  return std::string(StringViewFromVecU8(decoded_vec8));
}

absl::StatusOr<std::string> Flate2WriterCompress(absl::string_view input) {
  auto encoder =
      flate2_rs::write::GzEncoder::create(flate2_rs::Compression::best());
  flate2_rs::ResultUnit write_result = encoder.write_all(input);
  if (!write_result.is_ok()) {
    return absl::InternalError(std::move(write_result).unwrap_err());
  }
  flate2_rs::ResultVecU8 encode_result = std::move(encoder).finish();
  if (!encode_result.is_ok()) {
    return absl::InternalError(std::move(encode_result).unwrap_err());
  }
  rust_vec_u8::VecU8 flate_vec8 = std::move(encode_result).unwrap();
  return std::string(StringViewFromVecU8(flate_vec8));
}

absl::StatusOr<std::string> Flate2WriterDecompress(
    absl::string_view compressed) {
  auto decoder = flate2_rs::write::GzDecoder::create();
  flate2_rs::ResultUnit write_result = decoder.write_all(compressed);
  if (!write_result.is_ok()) {
    return absl::InternalError(std::move(write_result).unwrap_err());
  }
  flate2_rs::ResultVecU8 decode_result = std::move(decoder).finish();
  if (!decode_result.is_ok()) {
    return absl::InternalError(std::move(decode_result).unwrap_err());
  }
  rust_vec_u8::VecU8 decoded_vec8 = std::move(decode_result).unwrap();
  return std::string(StringViewFromVecU8(decoded_vec8));
}

TEST(Flate2CcApiTest, ReaderGzipCompress) {
  std::string input = "hello world";
  input.append(500, 'a');
  absl::StatusOr<std::string> gzip_status = Flate2ReaderCompress(input);
  ASSERT_TRUE(gzip_status.ok()) << gzip_status.status();
  std::string gzip_data = *gzip_status;

  EXPECT_GT(gzip_data.size(), 0);
  EXPECT_LT(gzip_data.size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipString(gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, ReaderGzipCompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  absl::StatusOr<std::string> gzip_status = Flate2ReaderCompress(input);
  ASSERT_TRUE(gzip_status.ok()) << gzip_status.status();
  std::string gzip_data = *gzip_status;
  EXPECT_GT(gzip_data.size(), 0);
  EXPECT_LT(gzip_data.size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipString(gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, ReaderGzipDecompress) {
  std::string input = "abc";
  input.append(500, 'a');
  std::string compressed;
  GzipString(input, &compressed);

  absl::StatusOr<std::string> gzip_status = Flate2ReaderDecompress(compressed);
  ASSERT_TRUE(gzip_status.ok()) << gzip_status.status();
  EXPECT_EQ(*gzip_status, input);
}

TEST(Flate2CcApiTest, ReaderGzipDecompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);

  absl::StatusOr<std::string> gzip_status = Flate2ReaderDecompress(compressed);
  ASSERT_TRUE(gzip_status.ok()) << gzip_status.status();
  EXPECT_EQ(*gzip_status, input);
}

TEST(Flate2CcApiTest, WriterGzipCompress) {
  std::string input = absl::StrCat("hello world", std::string(500, 'a'));

  absl::StatusOr<std::string> gzip_data = Flate2WriterCompress(input);
  ASSERT_OK(gzip_data);
  EXPECT_GT(gzip_data->size(), 0);
  EXPECT_LT(gzip_data->size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipString(*gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, WriterGzipCompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  absl::StatusOr<std::string> gzip_data = Flate2WriterCompress(input);
  ASSERT_OK(gzip_data);
  EXPECT_GT(gzip_data->size(), 0);
  EXPECT_LT(gzip_data->size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipString(*gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, WriterGzipDecompress) {
  std::string input = absl::StrCat("abc", std::string(500, 'a'));
  std::string compressed;
  GzipString(input, &compressed);

  absl::StatusOr<std::string> gzip_data = Flate2WriterDecompress(compressed);
  ASSERT_OK(gzip_data);
  EXPECT_EQ(*gzip_data, input);
}

TEST(Flate2CcApiTest, WriterGzipDecompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);

  absl::StatusOr<std::string> gzip_data = Flate2WriterDecompress(compressed);
  ASSERT_OK(gzip_data);
  EXPECT_EQ(*gzip_data, input);
}

TEST(Flate2CcApiTest, ReaderGzipDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  absl::StatusOr<std::string> reader_decode_result =
      Flate2ReaderDecompress(input);
  EXPECT_THAT(reader_decode_result, StatusIs(absl::StatusCode::kInternal));
}

TEST(Flate2CcApiTest, WriterGzipDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  absl::StatusOr<std::string> gzip_data = Flate2WriterDecompress(input);
  EXPECT_THAT(gzip_data, StatusIs(absl::StatusCode::kInternal));
}
}  // namespace
