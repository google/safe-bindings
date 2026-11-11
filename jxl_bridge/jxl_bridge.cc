#include "jxl_bridge.h"

#include <cstddef>
#include <cstdint>
#include <limits>
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
namespace {

bool IsValidChannelLayout(ChannelLayout channel_layout) {
  switch (channel_layout) {
    case ChannelLayout::Grayscale:
    case ChannelLayout::GrayscaleAlpha:
    case ChannelLayout::Rgb:
    case ChannelLayout::Rgba:
    case ChannelLayout::Cmyk:
      return true;
    default:
      return false;
  }
}

bool IsValidDataType(DataType data_type) {
  switch (data_type) {
    case DataType::U8:
    case DataType::U16:
    case DataType::F32:
      return true;
    default:
      return false;
  }
}

absl::StatusOr<size_t> CheckedSampleCount(uint32_t width, uint32_t height,
                                          ChannelLayout channel_layout) {
  if (width == 0 || height == 0) {
    return absl::InvalidArgumentError("Image has zero width or height");
  }
  if (!IsValidChannelLayout(channel_layout)) {
    return absl::InvalidArgumentError("Invalid channel layout");
  }
  const size_t width_sz = static_cast<size_t>(width);
  const size_t height_sz = static_cast<size_t>(height);
  if (height_sz > std::numeric_limits<size_t>::max() / width_sz) {
    return absl::InvalidArgumentError(
        "Image dimensions too large: width * height overflows");
  }
  const size_t num_pixels = width_sz * height_sz;
  const size_t num_channels = static_cast<size_t>(NumChannels(channel_layout));
  if (num_channels > std::numeric_limits<size_t>::max() / num_pixels) {
    return absl::InvalidArgumentError(
        "Image sample count overflows: dimensions * channels");
  }
  return num_pixels * num_channels;
}

}  // namespace

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

absl::Status JxlDecoder::SetPixelLayout(ChannelLayout channel_layout,
                                        const SampleFormat& sample_format) {
  return decoder_.set_pixel_layout_with_format(channel_layout, sample_format);
}

absl::StatusOr<FeedResult> JxlDecoder::DecodeFrame(
    absl::Span<const uint8_t>& data, ChannelLayout channel_layout,
    DataType data_type, absl::Span<uint8_t> output) {
  if (!IsValidChannelLayout(channel_layout)) {
    return absl::InvalidArgumentError("Invalid channel layout");
  }
  if (!IsValidDataType(data_type)) {
    return absl::InvalidArgumentError("Invalid data type");
  }
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

absl::StatusOr<size_t> RequiredOutputSize(uint32_t width, uint32_t height,
                                          ChannelLayout channel_layout,
                                          DataType data_type) {
  if (!IsValidDataType(data_type)) {
    return absl::InvalidArgumentError("Invalid data type");
  }
  ABSL_ASSIGN_OR_RETURN(const size_t num_samples,
                   CheckedSampleCount(width, height, channel_layout));
  const size_t bytes_per_sample =
      static_cast<size_t>(BytesPerSample(data_type));
  if (bytes_per_sample > std::numeric_limits<size_t>::max() / num_samples) {
    return absl::InvalidArgumentError(
        "Image buffer size overflows: samples * bytes_per_sample");
  }
  const size_t required_bytes = num_samples * bytes_per_sample;
  const size_t max_addressable_bytes =
      static_cast<size_t>(std::numeric_limits<ptrdiff_t>::max());
  if (required_bytes > max_addressable_bytes) {
    return absl::InvalidArgumentError(
        "Image buffer size exceeds addressable slice range");
  }
  return required_bytes;
}

absl::StatusOr<DecodedImage> DecodeJxl(
    absl::Span<const uint8_t> data, DataType data_type,
    std::optional<ChannelLayout> forced_layout, size_t sample_limit) {
  if (!IsValidDataType(data_type)) {
    return absl::InvalidArgumentError("Invalid data type");
  }
  if (forced_layout.has_value() && !IsValidChannelLayout(*forced_layout)) {
    return absl::InvalidArgumentError("Invalid channel layout");
  }

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

  ABSL_ASSIGN_OR_RETURN(const size_t num_samples,
                   CheckedSampleCount(info.width, info.height, channel_layout));
  // sample_limit 0 is treated as no limit.
  if (sample_limit > 0 && num_samples > sample_limit) {
    return absl::OutOfRangeError("Image sample count exceeds sample limit");
  }

  ABSL_ASSIGN_OR_RETURN(
      const size_t required_bytes,
      RequiredOutputSize(info.width, info.height, channel_layout, data_type));
  std::vector<uint8_t> pixels(required_bytes);

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
