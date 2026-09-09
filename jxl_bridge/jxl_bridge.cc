#include "jxl_bridge.h"

#include <cstddef>
#include <cstdint>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "rust/jxl_bridge_rs.h"

namespace safe_bindings::jxl_bridge {

JxlDecoder::JxlDecoder()
    : decoder_(jxl_bridge_rs::decoder::JxlBridgeDecoder::new_()) {}

JxlDecoder::JxlDecoder(const DecoderOptions& options)
    : decoder_(
          jxl_bridge_rs::decoder::JxlBridgeDecoder::new_with_options(options)) {
}

JxlDecoder::~JxlDecoder() = default;

JxlDecoder::JxlDecoder(JxlDecoder&&) = default;
JxlDecoder& JxlDecoder::operator=(JxlDecoder&&) = default;

absl::StatusOr<FeedResult> JxlDecoder::DecodeHeader(
    absl::Span<const uint8_t>& data) {
  ABSL_ASSIGN_OR_RETURN(auto result, decoder_.decode_header(data));
  data.remove_prefix(result.consumed);
  return result.status;
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrameHeader(
    absl::Span<const uint8_t>& data) {
  ABSL_ASSIGN_OR_RETURN(auto result, decoder_.decode_frame_header(data));
  data.remove_prefix(result.consumed);
  return result.status;
}

absl::StatusOr<BasicInfo> JxlDecoder::GetBasicInfo() const {
  return decoder_.basic_info();
}

absl::StatusOr<FrameHeader> JxlDecoder::GetFrameHeader() const {
  return decoder_.frame_header();
}

absl::Status JxlDecoder::SetOutputFormat(ColorType color_type,
                                         DataType data_type) {
  return decoder_.set_output_format(color_type, data_type);
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrame(
    absl::Span<const uint8_t>& data, ColorType color_type, DataType data_type,
    absl::Span<uint8_t> output) {
  if (!decoder_.has_output_format()) {
    ABSL_RETURN_IF_ERROR(SetOutputFormat(color_type, data_type));
  } else if (decoder_.output_color_type() != color_type ||
             decoder_.output_data_type() != data_type) {
    return absl::FailedPreconditionError(
        "DecodeFrame called with a different format than was previously set. "
        "Create a new decoder to change the output format.");
  }
  return DecodeFrame(data, output);
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrame(
    absl::Span<const uint8_t>& data, absl::Span<uint8_t> output) {
  if (!decoder_.has_output_format()) {
    return absl::FailedPreconditionError(
        "SetOutputFormat must be called before DecodeFrame");
  }
  ABSL_ASSIGN_OR_RETURN(auto result, decoder_.decode_frame(data, output));
  data.remove_prefix(result.consumed);
  return result.status;
}

bool JxlDecoder::HasMoreFrames() const { return decoder_.has_more_frames(); }

// Standalone functions:

bool HasJxlSignature(absl::Span<const uint8_t> data) {
  return jxl_bridge_rs::decoder::has_jxl_signature(data);
}

absl::StatusOr<BasicInfo> DecodeJxlHeader(absl::Span<const uint8_t> data) {
  JxlDecoder decoder;
  absl::Span<const uint8_t> remaining = data;
  ABSL_ASSIGN_OR_RETURN(FeedResult status, decoder.DecodeHeader(remaining));
  if (status != FeedResult::HeaderReady) {
    return absl::InvalidArgumentError("Failed to decode JXL header");
  }
  return decoder.GetBasicInfo();
}

absl::StatusOr<std::vector<uint8_t>> DecodeJxl(absl::Span<const uint8_t> data,
                                               ColorType color_type,
                                               size_t sample_limit) {
  DecoderOptions options;
  options.sample_limit = sample_limit;
  JxlDecoder decoder(options);

  absl::Span<const uint8_t> remaining = data;
  ABSL_ASSIGN_OR_RETURN(FeedResult status, decoder.DecodeHeader(remaining));
  if (status != FeedResult::HeaderReady) {
    return absl::InvalidArgumentError("Failed to decode JXL header");
  }

  ABSL_ASSIGN_OR_RETURN(const BasicInfo info, decoder.GetBasicInfo());

  if (info.width == 0 || info.height == 0) {
    return absl::InvalidArgumentError("Image has zero width or height");
  }

  // Use checked arithmetic to prevent integer overflow from malicious headers.
  size_t buffer_size = static_cast<size_t>(info.width);
  if (buffer_size != 0 && info.height > SIZE_MAX / buffer_size) {
    return absl::InvalidArgumentError(
        "Image dimensions too large: width * height overflows");
  }
  buffer_size *= info.height;

  const size_t pixel_bytes = static_cast<size_t>(NumChannels(color_type)) *
                             BytesPerSample(DataType::U8);

  if (buffer_size != 0 && pixel_bytes > SIZE_MAX / buffer_size) {
    return absl::InvalidArgumentError(
        "Image buffer size overflows: dimensions * channels * bps");
  }
  buffer_size *= pixel_bytes;
  if (buffer_size > sample_limit) {
    return absl::OutOfRangeError("Image buffer size exceeds sample limit");
  }
  std::vector<uint8_t> output(buffer_size);

  ABSL_ASSIGN_OR_RETURN(const FeedResult result,
                   decoder.DecodeFrame(remaining, color_type, DataType::U8,
                                       absl::MakeSpan(output)));
  if (result != FeedResult::Done) {
    return absl::InvalidArgumentError("Failed to decode JXL frame");
  }

  return output;
}

absl::StatusOr<std::vector<uint8_t>> DecodeJxlToRgb(
    absl::Span<const uint8_t> data, size_t sample_limit) {
  return DecodeJxl(data, ColorType::Rgb, sample_limit);
}

absl::StatusOr<std::vector<uint8_t>> DecodeJxlToRgba(
    absl::Span<const uint8_t> data, size_t sample_limit) {
  return DecodeJxl(data, ColorType::Rgba, sample_limit);
}

}  // namespace safe_bindings::jxl_bridge
