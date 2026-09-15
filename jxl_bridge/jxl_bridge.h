#ifndef SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_
#define SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_

#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "rust/jxl_bridge_rs.h"

namespace safe_bindings::jxl_bridge {

// Type aliases for the Crubit-generated types.
// These are the primary types used in the public API.
// Selects how decoded channels are interleaved into the output buffer.
using ChannelLayout = jxl_bridge_rs::types::JxlBridgeChannelLayout;
using DataType = jxl_bridge_rs::types::JxlBridgeDataType;
using DecoderOptions = jxl_bridge_rs::types::JxlBridgeDecoderOptions;
using BasicInfo = jxl_bridge_rs::types::JxlBridgeBasicInfo;
using FrameHeader = jxl_bridge_rs::types::JxlBridgeFrameHeader;
using FeedResult = jxl_bridge_rs::types::JxlBridgeFeedResult;
using ProcessResult = jxl_bridge_rs::types::JxlBridgeProcessResult;
using ExtraChannel = jxl_bridge_rs::types::JxlBridgeExtraChannel;
using ExtraChannelType = jxl_bridge_rs::types::JxlBridgeExtraChannelType;

// Returns true if the image has an extra channel of the given type.
// Use this to test for transparency (ExtraChannelType::Alpha) or for whether
// the image can be decoded as ChannelLayout::Cmyk (ExtraChannelType::Black).
inline bool HasExtraChannel(const BasicInfo& info, ExtraChannelType type) {
  for (const ExtraChannel& channel : info.extra_channels) {
    if (channel.channel_type == type) {
      return true;
    }
  }
  return false;
}

// Returns the number of channels for a given channel layout.
inline uint32_t NumChannels(ChannelLayout channel_layout) {
  switch (channel_layout) {
    case ChannelLayout::Grayscale:
      return 1;
    case ChannelLayout::GrayscaleAlpha:
      return 2;
    case ChannelLayout::Rgb:
      return 3;
    case ChannelLayout::Rgba:
      return 4;
    case ChannelLayout::Cmyk:
      return 4;
    default:
      return 3;  // Default to RGB.
  }
}

// Returns the number of bytes per sample for a given data type.
inline uint32_t BytesPerSample(DataType data_type) {
  switch (data_type) {
    case DataType::U8:
      return 1;
    case DataType::U16:
      return 2;
    case DataType::F32:
      return 4;
    default:
      return 1;  // Default to U8.
  }
}

// Returns the channel layout matching the image's own channels: Grayscale or
// Rgb for the color channels, the alpha-carrying variant when the image has an
// alpha channel, and Cmyk for images with a 4-channel color profile.
//
// Returns InvalidArgument if the image's channels cannot be expressed as a
// ChannelLayout.
//
// NOTE: ChannelLayout::Cmyk has no alpha variant, so alpha is dropped for CMYK
// images.
absl::StatusOr<ChannelLayout> ChannelLayoutForImage(const BasicInfo& info);

// JxlDecoder provides a C++ API for decoding JPEG XL images using the jxl-rs
// Rust decoder. This replaces the functionality of third_party/jpegxl decoding
// with a safer Rust implementation.
// Note that the span is modified/consumed by the decoder.
//
// For a single-frame image, prefer the DecodeJxl() convenience function below,
// which does all of the below and returns a DecodedImage.
//
// The decoder never converts between color spaces: the color channels are
// emitted in the image's own color space, which GetIccProfile() reports. The
// channel layout only selects which channels are interleaved into the output
// buffer and in what order.
//
// Streaming decoder:
//   JxlDecoder decoder;
//   absl::Span<const uint8_t> data = absl::MakeConstSpan(file_data);
//
//   ABSL_ASSIGN_OR_RETURN(FeedResult hdr_result, decoder.DecodeHeader(data));
//   ABSL_ASSIGN_OR_RETURN(BasicInfo info, decoder.GetBasicInfo());
//
//   // Or hardcode a layout, e.g. ChannelLayout::Rgba, to always get 4
//   // channels regardless of what the image itself contains.
//   ABSL_ASSIGN_OR_RETURN(ChannelLayout layout, ChannelLayoutForImage(info));
//   const size_t frame_size =
//       info.width * info.height * NumChannels(layout);
//   std::vector<uint8_t> output(frame_size);
//   ABSL_ASSIGN_OR_RETURN(
//       FeedResult frame_result,
//       decoder.DecodeFrame(data, layout, DataType::U8,
//                           absl::MakeSpan(output)));
//
//   // Only meaningful once the pixel layout is set, i.e. after DecodeFrame().
//   ABSL_ASSIGN_OR_RETURN(std::vector<uint8_t> icc, decoder.GetIccProfile());
//
// Animations:
//   JxlDecoder decoder;
//   absl::Span<const uint8_t> data = absl::MakeConstSpan(file_data);
//   ABSL_ASSIGN_OR_RETURN(FeedResult hdr, decoder.DecodeHeader(data));
//   ABSL_ASSIGN_OR_RETURN(BasicInfo info, decoder.GetBasicInfo());
//   ABSL_ASSIGN_OR_RETURN(ChannelLayout layout, ChannelLayoutForImage(info));
//   ABSL_RETURN_IF_ERROR(decoder.SetPixelLayout(layout, DataType::U8));
//   ABSL_ASSIGN_OR_RETURN(std::vector<uint8_t> icc, decoder.GetIccProfile());
//   const size_t frame_size =
//       info.width * info.height * NumChannels(layout);
//   std::vector<uint8_t> frame_buf(frame_size);
//   while (decoder.HasMoreFrames()) {
//     ABSL_ASSIGN_OR_RETURN(
//         FeedResult result,
//         decoder.DecodeFrame(data, absl::MakeSpan(frame_buf)));
//     // Process frame_buf...
//   }
class JxlDecoder final {
 public:
  // Creates a new JXL decoder with default options.
  JxlDecoder();

  // Creates a new JXL decoder with the specified options.
  explicit JxlDecoder(const DecoderOptions& options);

  ~JxlDecoder();

  JxlDecoder(JxlDecoder&&);
  JxlDecoder& operator=(JxlDecoder&&);

  JxlDecoder(const JxlDecoder&) = delete;
  JxlDecoder& operator=(const JxlDecoder&) = delete;

  // Decodes the image header from the input data.
  // Consumes bytes from `data` by advancing the span.
  // NOTE: This means the Span is modified!
  //
  // Returns the decoder status after processing:
  //   - NeedsMoreInput: provide more data and call again
  //   - HeaderReady: header parsed, call GetBasicInfo() and SetPixelLayout()
  absl::StatusOr<FeedResult> DecodeHeader(absl::Span<const uint8_t>& data);

  // Decodes the next frame header from the input data.
  // Consumes bytes from `data` by advancing the span.
  // NOTE: This means the Span is modified!
  //
  // Requires SetPixelLayout() to have been called.
  // After success, call GetFrameHeader() to retrieve the header.
  // Returns:
  //   - NeedsMoreInput: not enough data yet, provide more and retry
  //   - FrameHeaderReady: frame header parsed, call GetFrameHeader()
  absl::StatusOr<FeedResult> DecodeFrameHeader(absl::Span<const uint8_t>& data);

  // Returns basic image info after the header has been parsed.
  // Returns FailedPrecondition if DecodeHeader hasn't returned HeaderReady.
  absl::StatusOr<BasicInfo> GetBasicInfo() const;

  // Returns the ICC profile describing the color space of the decoded pixels.
  // Returns FailedPrecondition if DecodeHeader hasn't returned HeaderReady.
  //
  // SetPixelLayout() can change the output color profile, so call this after
  // the pixel layout has been set to obtain the profile the decoded pixels
  // will actually be in.
  //
  // Returns an empty profile if the output color encoding has no ICC
  // representation. The result is cached until the pixel layout changes.
  absl::StatusOr<std::vector<uint8_t>> GetIccProfile();

  // Returns the frame header for the next frame to be decoded.
  // Must be called after DecodeFrameHeader() has successfully parsed a
  // frame header. The result is cached per-frame.
  absl::StatusOr<FrameHeader> GetFrameHeader() const;

  // Sets how decoded pixels are written into the output buffer: which channels
  // are interleaved (`channel_layout`) and how each sample is stored
  // (`data_type`). Call after GetBasicInfo().
  //
  // This only describes the caller's buffer. In particular the channel layout
  // does not convert between color spaces: the color channels are always
  // emitted in the image's own color space, which GetIccProfile() reports.
  absl::Status SetPixelLayout(ChannelLayout channel_layout, DataType data_type);

  // Decodes the next frame from the input data into the output buffer.
  // Consumes bytes from `data` by advancing the span.
  // If the decoder is in HeaderDecoded state, this will also parse the frame
  // header internally before decoding pixels.
  // Must be called after the header has been parsed. If SetPixelLayout() has
  // not been called, it will be called with the given channel_layout/data_type.
  // Returns:
  //   - NeedsMoreInput: not enough data yet, provide more and retry
  //   - FrameReady: frame decoded, output buffer is filled
  //   - Done: last frame decoded, output buffer is filled
  // The output buffer must be pre-allocated with sufficient size:
  //   width * height * NumChannels(channel_layout) * BytesPerSample(data_type)
  absl::StatusOr<FeedResult> DecodeFrame(absl::Span<const uint8_t>& data,
                                         ChannelLayout channel_layout,
                                         DataType data_type,
                                         absl::Span<uint8_t> output);

  // Overload: decodes into output using a previously set pixel layout.
  // SetPixelLayout() must have been called before this.
  absl::StatusOr<FeedResult> DecodeFrame(absl::Span<const uint8_t>& data,
                                         absl::Span<uint8_t> output);

  // Returns true if there are more frames to decode (for animations).
  bool HasMoreFrames() const;

 private:
  jxl_bridge_rs::decoder::JxlBridgeDecoder decoder_;
};

// Checks if the given data starts with a valid JXL signature.
bool HasJxlSignature(absl::Span<const uint8_t> data);

// Convenience function: decode a JXL image header without creating a decoder.
absl::StatusOr<BasicInfo> DecodeJxlHeader(absl::Span<const uint8_t> data);

// A decoded single-frame image.
struct DecodedImage {
  // Interleaved samples: info.width * info.height *
  // NumChannels(channel_layout) * BytesPerSample(data_type) bytes.
  std::vector<uint8_t> pixels;
  // The layout `pixels` are interleaved in.
  ChannelLayout channel_layout = ChannelLayout::Rgb;
  // Header information for the decoded image.
  BasicInfo info;
  // ICC profile describing the color space of `pixels`. Empty if the image's
  // color encoding has no ICC representation, in which case callers may fall
  // back to assuming sRGB.
  std::vector<uint8_t> icc_profile;
};

// Convenience function: decode a single-frame JXL image.
// If no channel layout is specified, returns the layout matching the image's
// own channels (see ChannelLayoutForImage).
//
// sample_limit is the maximum number of samples to decode. 0 means no limit.
// The limit counts the product of pixels and channels, so for example an image
// with 1 extra channel of size 1024x1024 has 4 million samples.
absl::StatusOr<DecodedImage> DecodeJxl(
    absl::Span<const uint8_t> data, DataType data_type = DataType::U8,
    std::optional<ChannelLayout> forced_layout = std::nullopt,
    size_t sample_limit = 500 * 1024 * 1024);

}  // namespace safe_bindings::jxl_bridge

#endif  // SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_
