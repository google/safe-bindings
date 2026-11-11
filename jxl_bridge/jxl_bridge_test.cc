#include "jxl_bridge.h"

#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <vector>

#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace safe_bindings::jxl_bridge {
namespace {

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
  const absl::StatusOr<FeedResult> result = decoder.DecodeHeader(span);
  EXPECT_FALSE(result.ok());
  EXPECT_EQ(result.status().code(), absl::StatusCode::kInvalidArgument);
}

TEST(JxlBridgeTest, DecodeFrameFailsWithoutHeader) {
  JxlDecoder decoder;
  std::vector<uint8_t> output(100);
  absl::Span<const uint8_t> empty_span;
  const absl::StatusOr<FeedResult> result = decoder.DecodeFrame(
      empty_span, ChannelLayout::Rgb, DataType::U8, absl::MakeSpan(output));
  EXPECT_FALSE(result.ok());
  EXPECT_EQ(result.status().code(), absl::StatusCode::kFailedPrecondition);
}

TEST(JxlBridgeTest, NumChannelsReturnsCorrectValues) {
  EXPECT_EQ(NumChannels(ChannelLayout::Grayscale), 1);
  EXPECT_EQ(NumChannels(ChannelLayout::GrayscaleAlpha), 2);
  EXPECT_EQ(NumChannels(ChannelLayout::Rgb), 3);
  EXPECT_EQ(NumChannels(ChannelLayout::Rgba), 4);
  EXPECT_EQ(NumChannels(ChannelLayout::Cmyk), 4);
}

TEST(JxlBridgeTest, BytesPerSampleReturnsCorrectValues) {
  EXPECT_EQ(BytesPerSample(DataType::U8), 1);
  EXPECT_EQ(BytesPerSample(DataType::U16), 2);
  EXPECT_EQ(BytesPerSample(DataType::F32), 4);
}

// Minimal 1x1 RGB codestream (65 bytes).
constexpr uint8_t kBasicJxl[] = {
    0xff, 0x0a, 0x00, 0x90, 0x01, 0x00, 0x12, 0x88, 0x02, 0x00, 0xd4,
    0x00, 0x55, 0x0f, 0x00, 0x00, 0xa8, 0x50, 0x19, 0x65, 0xdc, 0xe0,
    0xe5, 0x5c, 0xcf, 0x97, 0x1f, 0x3a, 0x2c, 0xa6, 0x6d, 0x5c, 0x67,
    0x68, 0xab, 0x6d, 0x0b, 0x4b, 0x12, 0x45, 0xc6, 0xb1, 0x49, 0x3a,
    0x81, 0x43, 0x92, 0x58, 0x04, 0x36, 0x2e, 0x98, 0x07, 0x18, 0x00,
    0x86, 0x99, 0x03, 0x27, 0x33, 0x50, 0xe4, 0x4a, 0x12, 0x00,
};

// Minimal 8x8 RGB codestream (69 bytes).
constexpr uint8_t kNoise8x8Jxl[] = {
    0xff, 0x0a, 0x41, 0x40, 0x42, 0x71, 0x21, 0x18, 0x20, 0x08, 0x00, 0xe0,
    0x00, 0x00, 0x04, 0x30, 0x00, 0x01, 0x06, 0x1c, 0x90, 0xc0, 0x02, 0x00,
    0x30, 0x00, 0x50, 0x00, 0x50, 0x4a, 0x38, 0x41, 0xe6, 0xad, 0xdc, 0x29,
    0x52, 0xa8, 0x87, 0x39, 0x20, 0x93, 0x41, 0x26, 0x83, 0x9a, 0x8c, 0x79,
    0x8a, 0x90, 0x50, 0xe0, 0x93, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0x2a,
};

// 1x1 JXL header followed by a cropped 65536x65536 Modular frame with
// Flags::ENABLE_SPLINES (0x10), exercising u32 frame dimension multiplication
// across the Crubit FFI boundary (124 bytes, zero-padded tail).
constexpr uint8_t kCraftedSplineOverflowJxl[124] = {
    0xff, 0x0a, 0x00, 0x90, 0x01, 0xd8, 0x3f, 0x01, 0x00, 0x60,
    0x80, 0x5b, 0x00, 0x60, 0x80, 0x5b, 0x00, 0x80, 0x04, 0x10,
};

TEST(JxlBridgeTest, CatchesInternalRustOverflowPanicInSplineFrame) {
  const absl::StatusOr<DecodedImage> image =
      DecodeJxl(kCraftedSplineOverflowJxl, DataType::U8, std::nullopt, 0);
  EXPECT_FALSE(image.ok());
  EXPECT_TRUE(image.status().code() == absl::StatusCode::kInternal ||
              image.status().code() == absl::StatusCode::kInvalidArgument)
      << image.status();

  JxlDecoder decoder;
  absl::Span<const uint8_t> remaining(kCraftedSplineOverflowJxl);
  const absl::StatusOr<FeedResult> header_result =
      decoder.DecodeHeader(remaining);
  ASSERT_TRUE(header_result.ok()) << header_result.status();
  ASSERT_EQ(*header_result, FeedResult::HeaderReady);

  std::vector<uint8_t> output(3, 0);
  const absl::StatusOr<FeedResult> frame_result = decoder.DecodeFrame(
      remaining, ChannelLayout::Rgb, DataType::U8, absl::MakeSpan(output));
  EXPECT_FALSE(frame_result.ok());
  EXPECT_TRUE(frame_result.status().code() == absl::StatusCode::kInternal ||
              frame_result.status().code() ==
                  absl::StatusCode::kInvalidArgument)
      << frame_result.status();

  const absl::StatusOr<FeedResult> retry_result = decoder.DecodeFrame(
      remaining, ChannelLayout::Rgb, DataType::U8, absl::MakeSpan(output));
  EXPECT_FALSE(decoder.HasMoreFrames());
  EXPECT_FALSE(retry_result.ok());
}

TEST(JxlBridgeTest, DecodeJxlSampleLimitCountsSamplesNotBytes) {
  // 8x8 RGB has 192 samples (within sample_limit = 400), even though F32
  // output requires 768 bytes.
  const absl::StatusOr<DecodedImage> image =
      DecodeJxl(kNoise8x8Jxl, DataType::F32, std::nullopt, 400);
  ASSERT_TRUE(image.ok()) << image.status();
  EXPECT_EQ(image->info.width, 8u);
  EXPECT_EQ(image->info.height, 8u);
  EXPECT_EQ(image->channel_layout, ChannelLayout::Rgb);
  EXPECT_EQ(image->pixels.size(), 8u * 8u * 3u * 4u);

  const absl::StatusOr<DecodedImage> over_limit =
      DecodeJxl(kNoise8x8Jxl, DataType::F32, std::nullopt, 191);
  EXPECT_FALSE(over_limit.ok());
  EXPECT_EQ(over_limit.status().code(), absl::StatusCode::kOutOfRange);
}

TEST(JxlBridgeTest, RejectsInvalidOpenEnumValues) {
  const absl::StatusOr<DecodedImage> bad_data_type =
      DecodeJxl(kBasicJxl, static_cast<DataType>(99));
  EXPECT_FALSE(bad_data_type.ok());
  EXPECT_EQ(bad_data_type.status().code(), absl::StatusCode::kInvalidArgument);

  const absl::StatusOr<DecodedImage> bad_layout =
      DecodeJxl(kBasicJxl, DataType::U8, static_cast<ChannelLayout>(99));
  EXPECT_FALSE(bad_layout.ok());
  EXPECT_EQ(bad_layout.status().code(), absl::StatusCode::kInvalidArgument);

  JxlDecoder decoder;
  absl::Span<const uint8_t> remaining(kBasicJxl);
  const absl::StatusOr<FeedResult> header_result =
      decoder.DecodeHeader(remaining);
  ASSERT_TRUE(header_result.ok()) << header_result.status();
  ASSERT_EQ(*header_result, FeedResult::HeaderReady);

  const absl::Status invalid_layout_status =
      decoder.SetPixelLayout(static_cast<ChannelLayout>(99), DataType::U8);
  EXPECT_FALSE(invalid_layout_status.ok());
  EXPECT_EQ(invalid_layout_status.code(), absl::StatusCode::kInvalidArgument);

  const absl::Status invalid_type_status =
      decoder.SetPixelLayout(ChannelLayout::Rgb, static_cast<DataType>(99));
  EXPECT_FALSE(invalid_type_status.ok());
  EXPECT_EQ(invalid_type_status.code(), absl::StatusCode::kInvalidArgument);

  SampleFormat bad_endianness;
  bad_endianness.data_type = DataType::U16;
  bad_endianness.endianness = static_cast<Endianness>(99);
  const absl::Status invalid_endian_status =
      decoder.SetPixelLayout(ChannelLayout::Rgb, bad_endianness);
  EXPECT_FALSE(invalid_endian_status.ok());
  EXPECT_EQ(invalid_endian_status.code(), absl::StatusCode::kInvalidArgument);
}

TEST(JxlBridgeTest, SampleFormatIgnoresBitDepthForF32) {
  JxlDecoder decoder;
  absl::Span<const uint8_t> remaining(kBasicJxl);
  const absl::StatusOr<FeedResult> header_result =
      decoder.DecodeHeader(remaining);
  ASSERT_TRUE(header_result.ok()) << header_result.status();
  ASSERT_EQ(*header_result, FeedResult::HeaderReady);

  SampleFormat sample_format;
  sample_format.data_type = DataType::F32;
  sample_format.bit_depth = 64;
  const absl::Status layout_status =
      decoder.SetPixelLayout(ChannelLayout::Rgb, sample_format);
  ASSERT_TRUE(layout_status.ok()) << layout_status;

  std::vector<uint8_t> output(3 * sizeof(float), 0);
  const absl::StatusOr<FeedResult> frame_result =
      decoder.DecodeFrame(remaining, absl::MakeSpan(output));
  ASSERT_TRUE(frame_result.ok()) << frame_result.status();
  EXPECT_EQ(*frame_result, FeedResult::Done);
}

TEST(JxlBridgeTest, RequiredOutputSizePreventsUint32AndSizeTOverflow) {
  const absl::StatusOr<size_t> valid_size =
      RequiredOutputSize(10, 20, ChannelLayout::Rgba, DataType::U16);
  ASSERT_TRUE(valid_size.ok()) << valid_size.status();
  EXPECT_EQ(*valid_size, size_t{10} * 20 * 4 * 2);

  BasicInfo basic_info;
  basic_info.width = 8;
  basic_info.height = 8;
  const absl::StatusOr<size_t> info_size =
      RequiredOutputSize(basic_info, ChannelLayout::Rgb);
  ASSERT_TRUE(info_size.ok()) << info_size.status();
  EXPECT_EQ(*info_size, size_t{8} * 8 * 3);

  FrameHeader frame_header;
  frame_header.width = 16;
  frame_header.height = 4;
  const absl::StatusOr<size_t> frame_size = RequiredOutputSize(
      frame_header, ChannelLayout::GrayscaleAlpha, DataType::F32);
  ASSERT_TRUE(frame_size.ok()) << frame_size.status();
  EXPECT_EQ(*frame_size, size_t{16} * 4 * 2 * 4);

  // 65536 * 65536 * 4 wraps uint32_t to 0, and 65536 * 16385 * 4 wraps uint32_t
  // to 262144.
  const absl::StatusOr<size_t> wrap_zero =
      RequiredOutputSize(65536, 65536, ChannelLayout::Rgba, DataType::U8);
  const absl::StatusOr<size_t> wrap_small =
      RequiredOutputSize(65536, 16385, ChannelLayout::Rgba, DataType::U8);
  if constexpr (sizeof(size_t) >= 8) {
    ASSERT_TRUE(wrap_zero.ok()) << wrap_zero.status();
    EXPECT_EQ(*wrap_zero, size_t{65536} * 65536 * 4);
    ASSERT_TRUE(wrap_small.ok()) << wrap_small.status();
    EXPECT_EQ(*wrap_small, size_t{65536} * 16385 * 4);

    // 2^31 * 2^31 = 2^62 <= PTRDIFF_MAX (2^63 - 1) for 1 byte/pixel, whereas
    // 2 bytes/pixel reaches 2^63 > PTRDIFF_MAX.
    const absl::StatusOr<size_t> within_ptrdiff = RequiredOutputSize(
        0x80000000u, 0x80000000u, ChannelLayout::Grayscale, DataType::U8);
    ASSERT_TRUE(within_ptrdiff.ok()) << within_ptrdiff.status();
    EXPECT_EQ(*within_ptrdiff, size_t{1} << 62);

    const absl::StatusOr<size_t> exceeds_ptrdiff = RequiredOutputSize(
        0x80000000u, 0x80000000u, ChannelLayout::GrayscaleAlpha, DataType::U8);
    EXPECT_FALSE(exceeds_ptrdiff.ok());
    EXPECT_EQ(exceeds_ptrdiff.status().code(),
              absl::StatusCode::kInvalidArgument);
  } else {
    EXPECT_FALSE(wrap_zero.ok());
    EXPECT_EQ(wrap_zero.status().code(), absl::StatusCode::kInvalidArgument);
    EXPECT_FALSE(wrap_small.ok());
    EXPECT_EQ(wrap_small.status().code(), absl::StatusCode::kInvalidArgument);

    const absl::StatusOr<size_t> exceeds_ptrdiff = RequiredOutputSize(
        65536, 32768, ChannelLayout::Grayscale, DataType::U8);
    EXPECT_FALSE(exceeds_ptrdiff.ok());
    EXPECT_EQ(exceeds_ptrdiff.status().code(),
              absl::StatusCode::kInvalidArgument);
  }

  const absl::StatusOr<size_t> max_overflow = RequiredOutputSize(
      0xFFFFFFFFu, 0xFFFFFFFFu, ChannelLayout::Rgba, DataType::F32);
  EXPECT_FALSE(max_overflow.ok());
  EXPECT_EQ(max_overflow.status().code(), absl::StatusCode::kInvalidArgument);

  EXPECT_EQ(RequiredOutputSize(0, 100, ChannelLayout::Rgb, DataType::U8)
                .status()
                .code(),
            absl::StatusCode::kInvalidArgument);
  EXPECT_EQ(RequiredOutputSize(100, 0, ChannelLayout::Rgb, DataType::U8)
                .status()
                .code(),
            absl::StatusCode::kInvalidArgument);
  EXPECT_EQ(RequiredOutputSize(10, 10, static_cast<ChannelLayout>(99))
                .status()
                .code(),
            absl::StatusCode::kInvalidArgument);
  EXPECT_EQ(
      RequiredOutputSize(10, 10, ChannelLayout::Rgb, static_cast<DataType>(99))
          .status()
          .code(),
      absl::StatusCode::kInvalidArgument);
}

}  // namespace
}  // namespace safe_bindings::jxl_bridge
