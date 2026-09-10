#include "jxl_bridge.h"

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace safe_bindings::jxl_bridge {
namespace {

using ::testing::status::StatusIs;

TEST(JxlBridgeTest, HasJxlSignatureReturnsFalseForEmptyInput) {
  EXPECT_FALSE(HasJxlSignature(absl::Span<const uint8_t>()));
}

TEST(JxlBridgeTest, HasJxlSignatureReturnsFalseForNonJxl) {
  const uint8_t data[] = {0x01, 0x02, 0x03, 0x04};
  EXPECT_FALSE(HasJxlSignature(data));
}

TEST(JxlBridgeTest, HasJxlSignatureReturnsTrueForCodestream) {
  const uint8_t sig[] = {0xff, 0x0a, 0x00, 0x00};
  EXPECT_TRUE(HasJxlSignature(sig));
}

TEST(JxlBridgeTest, HasJxlSignatureReturnsTrueForContainer) {
  const uint8_t sig[] = {0x00, 0x00, 0x00, 0x0c, 'J',  'X',
                         'L',  ' ',  0x0d, 0x0a, 0x87, 0x0a};
  EXPECT_TRUE(HasJxlSignature(sig));
}

TEST(JxlBridgeTest, DecodeHeaderFailsOnInvalidData) {
  JxlDecoder decoder;
  const uint8_t bad_data[] = {0x01, 0x02, 0x03, 0x04, 0x05};
  absl::Span<const uint8_t> span(bad_data);
  EXPECT_THAT(decoder.DecodeHeader(span),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(JxlBridgeTest, DecodeFrameFailsWithoutHeader) {
  JxlDecoder decoder;
  std::vector<uint8_t> output(100);
  absl::Span<const uint8_t> empty_span;
  EXPECT_THAT(decoder.DecodeFrame(empty_span, ColorType::Rgb, DataType::U8,
                                  absl::MakeSpan(output)),
              StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(JxlBridgeTest, NumChannelsReturnsCorrectValues) {
  EXPECT_EQ(NumChannels(ColorType::Grayscale), 1);
  EXPECT_EQ(NumChannels(ColorType::GrayscaleAlpha), 2);
  EXPECT_EQ(NumChannels(ColorType::Rgb), 3);
  EXPECT_EQ(NumChannels(ColorType::Rgba), 4);
}

TEST(JxlBridgeTest, BytesPerSampleReturnsCorrectValues) {
  EXPECT_EQ(BytesPerSample(DataType::U8), 1);
  EXPECT_EQ(BytesPerSample(DataType::U16), 2);
  EXPECT_EQ(BytesPerSample(DataType::F32), 4);
}

}  // namespace
}  // namespace safe_bindings::jxl_bridge
