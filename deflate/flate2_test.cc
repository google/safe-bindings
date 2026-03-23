#include "security/deflate/flate2.h"

#include <string>
#include <utility>

#include "devtools/build/runtime/get_runfiles_dir.h"
#include "file/base/helpers.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "third_party/absl/log/check.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/strings/cord.h"
#include "third_party/absl/strings/str_cat.h"
#include "third_party/absl/strings/string_view.h"
#include "util/gzip/gzipstring.h"

namespace security::deflate {

using ::testing::status::StatusIs;

std::string ReadTestFile(absl::string_view path) {
  const std::string full_path = devtools_build::GetDataDependencyFilepath(path);
  std::string content;
  CHECK_OK(file::GetContents(full_path, &content));
  return content;
}

TEST(Flate2CcApiTest, ReaderGzipCompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  auto encoder = read::GzEncoder::create(input, Compression::best());
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper gzip_data_wrapper, encoder.read_to_end());
  absl::Cord gzip_data = std::move(gzip_data_wrapper).as_cord();

  std::string decompressed;
  ASSERT_TRUE(GunzipCordToString(gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, ReaderGzipDecompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);

  auto decoder = read::GzDecoder::create(compressed);
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decoded_data_wrapper,
                       decoder.read_to_end());
  absl::Cord decoded_data = std::move(decoded_data_wrapper).as_cord();
  EXPECT_EQ(decoded_data, input);
}

TEST(Flate2CcApiTest, ReaderMultiGzDecompress) {
  std::string input1 = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed1;
  GzipString(input1, &compressed1);
  std::string input2 = "another string";
  std::string compressed2;
  GzipString(input2, &compressed2);
  std::string compressed = absl::StrCat(compressed1, compressed2);

  auto decoder = read::MultiGzDecoder::create(compressed);
  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decoded_data_wrapper,
                       decoder.read_to_end());
  absl::Cord decoded_data = std::move(decoded_data_wrapper).as_cord();
  EXPECT_EQ(decoded_data, absl::StrCat(input1, input2));
}

TEST(Flate2CcApiTest, WriterGzipCompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  auto encoder = write::GzEncoder::create(Compression::best());

  ASSERT_OK(encoder.write_all(input.substr(0, input.size() / 2)));
  ASSERT_OK(encoder.write_all(input.substr(input.size() / 2)));

  ASSERT_OK_AND_ASSIGN(VecU8Wrapper gzip_data_wrapper,
                       std::move(encoder).finish());
  absl::Cord gzip_data = std::move(gzip_data_wrapper).as_cord();
  EXPECT_GT(gzip_data.size(), 0);
  EXPECT_LT(gzip_data.size(), input.size());

  std::string decompressed;
  ASSERT_TRUE(GunzipCordToString(gzip_data, &decompressed));
  EXPECT_EQ(decompressed, input);
}

TEST(Flate2CcApiTest, WriterGzipDecompressLarge) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);

  auto decoder = write::GzDecoder::create();
  ASSERT_OK(decoder.write_all(compressed.substr(0, compressed.size() / 2)));
  ASSERT_OK(decoder.write_all(compressed.substr(compressed.size() / 2)));

  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decoded_data_wrapper,
                       std::move(decoder).finish());
  absl::Cord decoded_data = std::move(decoded_data_wrapper).as_cord();
  EXPECT_EQ(decoded_data, input);
}

TEST(Flate2CcApiTest, WriterMultiGzDecompress) {
  std::string input1 = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed1;
  GzipString(input1, &compressed1);
  std::string input2 = "another string";
  std::string compressed2;
  GzipString(input2, &compressed2);

  auto decoder = write::MultiGzDecoder::create();
  ASSERT_OK(decoder.write_all(compressed1));
  ASSERT_OK(decoder.write_all(compressed2));

  ASSERT_OK_AND_ASSIGN(VecU8Wrapper decoded_data_wrapper,
                       std::move(decoder).finish());
  absl::Cord decoded_data = std::move(decoded_data_wrapper).as_cord();
  EXPECT_EQ(decoded_data, absl::StrCat(input1, input2));
}

TEST(Flate2CcApiTest, ReaderGzipDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  auto reader_decoder = read::GzDecoder::create(input);
  absl::StatusOr<VecU8Wrapper> reader_decode_status =
      reader_decoder.read_to_end();
  ASSERT_THAT(reader_decode_status,
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(Flate2CcApiTest, ReaderMultiGzDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  auto reader_decoder = read::MultiGzDecoder::create(input);
  absl::StatusOr<VecU8Wrapper> reader_decode_status =
      reader_decoder.read_to_end();
  ASSERT_THAT(reader_decode_status,
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(Flate2CcApiTest, ReaderGzipDecompressFailTruncated) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);
  compressed = compressed.substr(0, compressed.size() / 2);

  auto reader_decoder = read::GzDecoder::create(compressed);
  absl::StatusOr<VecU8Wrapper> reader_decode_status =
      reader_decoder.read_to_end();
  ASSERT_THAT(reader_decode_status,
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(Flate2CcApiTest, WriterGzipDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  auto writer_decoder = write::GzDecoder::create();
  absl::Status write_result = writer_decoder.write_all(input);
  ASSERT_THAT(write_result, StatusIs(absl::StatusCode::kInvalidArgument));
  ASSERT_THAT(std::move(writer_decoder).finish(),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(Flate2CcApiTest, WriterMultiGzDecompressFailNonGzip) {
  std::string input = "not a gzipped string";

  auto decoder = write::MultiGzDecoder::create();
  absl::Status write_result = decoder.write_all(input);
  ASSERT_THAT(write_result, StatusIs(absl::StatusCode::kInvalidArgument));
  ASSERT_THAT(std::move(decoder).finish(),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(Flate2CcApiTest, WriterGzipDecompressFailTruncated) {
  std::string input = ReadTestFile(
      "/google3/util/compression/flate/internal/testdata/world192.txt");
  std::string compressed;
  GzipString(input, &compressed);
  compressed = compressed.substr(0, compressed.size() / 2);

  auto decoder = write::GzDecoder::create();

  ASSERT_OK(decoder.write_all(compressed));
  ASSERT_THAT(std::move(decoder).finish(),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

}  // namespace security::deflate
