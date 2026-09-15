#include "jxl_bridge.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
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

absl::StatusOr<std::vector<uint8_t>> JxlDecoder::GetIccProfile() {
  return decoder_.icc_profile();
}

absl::StatusOr<FrameHeader> JxlDecoder::GetFrameHeader() const {
  return decoder_.frame_header();
}

absl::Status JxlDecoder::SetPixelLayout(ChannelLayout channel_layout,
                                        DataType data_type) {
  return decoder_.set_pixel_layout(channel_layout, data_type);
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrame(
    absl::Span<const uint8_t>& data, ChannelLayout channel_layout,
    DataType data_type, absl::Span<uint8_t> output) {
  if (!decoder_.has_pixel_layout()) {
    ABSL_RETURN_IF_ERROR(SetPixelLayout(channel_layout, data_type));
  } else if (decoder_.output_channel_layout() != channel_layout ||
             decoder_.output_data_type() != data_type) {
    return absl::FailedPreconditionError(
        "DecodeFrame called with a different pixel layout than was previously "
        "set. Create a new decoder to change the pixel layout.");
  }
  return DecodeFrame(data, output);
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrame(
    absl::Span<const uint8_t>& data, absl::Span<uint8_t> output) {
  if (!decoder_.has_pixel_layout()) {
    return absl::FailedPreconditionError(
        "SetPixelLayout must be called before DecodeFrame");
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

absl::StatusOr<ChannelLayout> ChannelLayoutForImage(const BasicInfo& info) {
  const bool has_alpha = HasExtraChannel(info, ExtraChannelType::Alpha);
  switch (info.num_color_channels) {
    case 1:
      return has_alpha ? ChannelLayout::GrayscaleAlpha
                       : ChannelLayout::Grayscale;
    case 3:
      return has_alpha ? ChannelLayout::Rgba : ChannelLayout::Rgb;
    case 4:
      // A 4-channel color profile is CMYK. JPEG XL stores the K plane in a
      // Black extra channel, which ChannelLayout::Cmyk interleaves as the
      // fourth sample; without it the image cannot be laid out as CMYK.
      if (!HasExtraChannel(info, ExtraChannelType::Black)) {
        return absl::InvalidArgumentError(
            "Image has a 4-channel color profile but no Black extra channel");
      }
      return ChannelLayout::Cmyk;
    default:
      return absl::InvalidArgumentError(absl::StrCat(
          "Unsupported number of color channels: ", info.num_color_channels));
  }
}

absl::StatusOr<DecodedImage> DecodeJxl(
    absl::Span<const uint8_t> data, DataType data_type,
    std::optional<ChannelLayout> forced_layout, size_t sample_limit) {
  DecoderOptions options;
  options.sample_limit = sample_limit;
  JxlDecoder decoder(options);

  absl::Span<const uint8_t> remaining = data;
  ABSL_ASSIGN_OR_RETURN(FeedResult status, decoder.DecodeHeader(remaining));
  if (status != FeedResult::HeaderReady) {
    return absl::InvalidArgumentError("Failed to decode JXL header");
  }

  ABSL_ASSIGN_OR_RETURN(BasicInfo info, decoder.GetBasicInfo());

  if (info.width == 0 || info.height == 0) {
    return absl::InvalidArgumentError("Image has zero width or height");
  }

  ChannelLayout channel_layout;
  if (forced_layout.has_value()) {
    channel_layout = *forced_layout;
  } else {
    ABSL_ASSIGN_OR_RETURN(channel_layout, ChannelLayoutForImage(info));
  }

  // Use checked arithmetic to prevent integer overflow from malicious headers.
  // Width and height are both non-zero here, so the divisions are safe.
  size_t num_samples = static_cast<size_t>(info.width);
  if (info.height > SIZE_MAX / num_samples) {
    return absl::InvalidArgumentError(
        "Image dimensions too large: width * height overflows");
  }
  num_samples *= info.height;

  const size_t pixel_bytes =
      NumChannels(channel_layout) * BytesPerSample(data_type);
  if (pixel_bytes > SIZE_MAX / num_samples) {
    return absl::InvalidArgumentError(
        "Image buffer size overflows: dimensions * channels");
  }
  num_samples *= pixel_bytes;

  // sample_limit 0 is treated as no limit.
  if (sample_limit > 0 && num_samples > sample_limit) {
    return absl::OutOfRangeError("Image buffer size exceeds sample limit");
  }

  std::vector<uint8_t> pixels(num_samples);

  ABSL_ASSIGN_OR_RETURN(const FeedResult result,
                   decoder.DecodeFrame(remaining, channel_layout, data_type,
                                       absl::MakeSpan(pixels)));
  if (result != FeedResult::Done) {
    return absl::InvalidArgumentError("Failed to decode JXL frame");
  }

  // Read the profile only after DecodeFrame: setting the pixel layout can
  // change the decoder's output color profile.
  ABSL_ASSIGN_OR_RETURN(std::vector<uint8_t> icc_profile, decoder.GetIccProfile());

  DecodedImage image;
  image.pixels = std::move(pixels);
  image.channel_layout = channel_layout;
  image.info = std::move(info);
  image.icc_profile = std::move(icc_profile);
  return image;
}

}  // namespace safe_bindings::jxl_bridge
