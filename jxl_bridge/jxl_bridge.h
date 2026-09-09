#ifndef SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_
#define SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_

#include <cstddef>
#include <cstdint>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "rust/jxl_bridge_rs.h"

namespace safe_bindings::jxl_bridge {

// Type aliases for the Crubit-generated types.
// These are the primary types used in the public API.
using ColorType = jxl_bridge_rs::types::JxlBridgeColorType;
using DataType = jxl_bridge_rs::types::JxlBridgeDataType;
using DecoderOptions = jxl_bridge_rs::types::JxlBridgeDecoderOptions;
using BasicInfo = jxl_bridge_rs::types::JxlBridgeBasicInfo;
using FrameHeader = jxl_bridge_rs::types::JxlBridgeFrameHeader;
using FeedResult = jxl_bridge_rs::types::JxlBridgeFeedResult;
using ProcessResult = jxl_bridge_rs::types::JxlBridgeProcessResult;

// Returns the number of channels for a given color type.
inline uint32_t NumChannels(ColorType color_type) {
  switch (color_type) {
    case ColorType::Grayscale:
      return 1;
    case ColorType::GrayscaleAlpha:
      return 2;
    case ColorType::Rgb:
      return 3;
    case ColorType::Rgba:
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

// JxlDecoder provides a C++ API for decoding JPEG XL images using the jxl-rs
// Rust decoder. This replaces the functionality of third_party/jpegxl decoding
// with a safer Rust implementation.
// Note that the span is modified/consumed by the decoder.
//
// Streaming decoder:
//   JxlDecoder decoder;
//   absl::Span<const uint8_t> data = absl::MakeConstSpan(file_data);
//
//   ABSL_ASSIGN_OR_RETURN(FeedResult hdr_result, decoder.DecodeHeader(data));
//   ABSL_ASSIGN_OR_RETURN(BasicInfo info, decoder.GetBasicInfo());
//
//   const size_t frame_size =
//       info.width * info.height * NumChannels(ColorType::Rgb);
//   std::vector<uint8_t> output(frame_size);
//   ABSL_ASSIGN_OR_RETURN(
//       FeedResult frame_result,
//       decoder.DecodeFrame(data, ColorType::Rgb, DataType::U8,
//                           absl::MakeSpan(output)));
//
// Animations:
//   JxlDecoder decoder;
//   absl::Span<const uint8_t> data = absl::MakeConstSpan(file_data);
//   ABSL_ASSIGN_OR_RETURN(FeedResult hdr, decoder.DecodeHeader(data));
//   ABSL_ASSIGN_OR_RETURN(BasicInfo info, decoder.GetBasicInfo());
//   ABSL_RETURN_IF_ERROR(decoder.SetOutputFormat(ColorType::Rgb, DataType::U8));
//   const size_t frame_size =
//       info.width * info.height * NumChannels(ColorType::Rgb);
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
  //   - HeaderReady: header parsed, call GetBasicInfo() and SetOutputFormat()
  absl::StatusOr<FeedResult> DecodeHeader(absl::Span<const uint8_t>& data);

  // Decodes the next frame header from the input data.
  // Consumes bytes from `data` by advancing the span.
  // NOTE: This means the Span is modified!
  //
  // Requires SetOutputFormat() to have been called.
  // After success, call GetFrameHeader() to retrieve the header.
  // Returns:
  //   - NeedsMoreInput: not enough data yet, provide more and retry
  //   - FrameHeaderReady: frame header parsed, call GetFrameHeader()
  absl::StatusOr<FeedResult> DecodeFrameHeader(absl::Span<const uint8_t>& data);

  // Returns basic image info after the header has been parsed.
  // Returns FailedPrecondition if DecodeHeader hasn't returned HeaderReady.
  absl::StatusOr<BasicInfo> GetBasicInfo() const;

  // Returns the frame header for the next frame to be decoded.
  // Must be called after DecodeFrameHeader() has successfully parsed a
  // frame header. The result is cached per-frame.
  absl::StatusOr<FrameHeader> GetFrameHeader() const;

  // Sets the desired output pixel format.
  // Call after GetBasicInfo().
  absl::Status SetOutputFormat(ColorType color_type, DataType data_type);

  // Decodes the next frame from the input data into the output buffer.
  // Consumes bytes from `data` by advancing the span.
  // If the decoder is in HeaderDecoded state, this will also parse the frame
  // header internally before decoding pixels.
  // Must be called after the header has been parsed. If SetOutputFormat() has
  // not been called, it will be called with the given color_type/data_type.
  // Returns:
  //   - NeedsMoreInput: not enough data yet, provide more and retry
  //   - FrameReady: frame decoded, output buffer is filled
  //   - Done: last frame decoded, output buffer is filled
  // The output buffer must be pre-allocated with sufficient size:
  //   width * height * NumChannels(color_type) * BytesPerSample(data_type)
  absl::StatusOr<FeedResult> DecodeFrame(absl::Span<const uint8_t>& data,
                                         ColorType color_type,
                                         DataType data_type,
                                         absl::Span<uint8_t> output);

  // Overload: decodes into output using a previously set output format.
  // SetOutputFormat() must have been called before this.
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

// Convenience function: decode a single-frame JXL image to uint8 pixels.
// Returns a buffer of width * height * NumChannels(color_type) bytes.
// sample_limit is the maximum number of samples to decode. 0 means no limit.
// The limit counts the product of pixels and channels, so for example an image
// with 1 extra channel of size 1024x1024 has 4 million samples.
absl::StatusOr<std::vector<uint8_t>> DecodeJxl(
    absl::Span<const uint8_t> data, ColorType color_type,
    size_t sample_limit = 500 * 1024 * 1024);

// Convenience function: decode a single-frame JXL image to uint8 RGB pixels,
// allocating the output buffer.
// sample_limit is the maximum number of samples to decode. 0 means no limit.
// The limit counts the product of pixels and channels, so for example an image
// with 1 extra channel of size 1024x1024 has 4 million samples.
absl::StatusOr<std::vector<uint8_t>> DecodeJxlToRgb(
    absl::Span<const uint8_t> data, size_t sample_limit = 500 * 1024 * 1024);

// Convenience function: decode a single-frame JXL image to uint8 RGBA pixels,
// allocating the output buffer.
// sample_limit is the maximum number of samples to decode. 0 means no limit.
// The limit counts the product of pixels and channels, so for example an image
// with 1 extra channel of size 1024x1024 has 4 million samples.
absl::StatusOr<std::vector<uint8_t>> DecodeJxlToRgba(
    absl::Span<const uint8_t> data, size_t sample_limit = 500 * 1024 * 1024);

}  // namespace safe_bindings::jxl_bridge

#endif  // SAFE_BINDINGS_JXL_BRIDGE_JXL_BRIDGE_H_
