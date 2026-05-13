#ifndef SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_H_
#define SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_H_

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <variant>
#include <vector>

#include "rust.h"
#include "absl/base/attributes.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::pixel_bridge {

// An enumeration over the types that back the value of a pixel.
enum class PixelType {
  kUint8,
  kUint16,
  kFloat32,
};

// An enumeration over supported color types.
enum class ColorType {
  // Luminance.
  kL,
  // Luminance with alpha channel.
  kLa,
  // Pixel contains R, G and B channels.
  kRgb,
  // Pixel contains R, G and B channels with an alpha channel.
  kRgba,
};

// An enumeration over all possible image formats.
enum class Format {
  kPng,
  kJpeg,
  kGif,
  kWebP,
  kTiff,
  kBmp,
  kIco,
};

// An enumeration over chroma subsampling types.
enum class ChromaSubsampling {
  kUnknown,
  kCs444,
  kCs422,
  kCs420,
  kCs411,
  kCs410,
};

// Information about strides (offset to the next sample).
struct Strides final {
  // Add this to an index to get to the next sample in x-direction.
  uintptr_t width;
  // Add this to an index to get to the next sample in y-direction.
  uintptr_t height;
  // Add this to an index to get to the sample in the next channel.
  uintptr_t channels;
};

class Frame final {
 public:
  // Frames should not be copied, because the data it carries is too big for
  // accidental copies, and just consuming it should satisfy the intended
  // use-cases.
  Frame(Frame&&) noexcept = default;
  Frame& operator=(Frame&&) noexcept = default;
  ~Frame() = default;

  // Delay of the current frame in ms. Returns 0 if the frame has been moved
  // from.
  uint64_t GetDelayMs() const;

  // Get the image data of the frame.
  std::string GetImage();

  // Get a reference to the image of the frame.
  absl::string_view GetImageRef() ABSL_ATTRIBUTE_LIFETIME_BOUND;

 private:
  friend class Frames;
  explicit Frame(rust::image::Frame frame);
  rust::image::Frame frame_;
};

class Frames final {
 public:
  // Frames should not be copied, because the data it carries is too big for
  // accidental copies, and just consuming it should satisfy the intended
  // use-cases.
  Frames(Frames&&) noexcept = default;
  Frames& operator=(Frames&&) noexcept = default;
  ~Frames() = default;

  // Returns the current frame and advances the underlying iterator.
  // The returned string_view lives until the next call of this method.
  std::optional<Frame> GetCurrentFrameAndAdvance();

 private:
  friend class ImageDecoder;
  explicit Frames(rust::image::Frames);
  rust::image::Frames frames_;
};

class ImageDecoder final {
 public:
  using Samples = std::variant<std::vector<uint8_t>, std::vector<uint16_t>,
                               std::vector<float>>;
  // ImageDecoder is movable.
  ImageDecoder(ImageDecoder&&) noexcept = default;
  ImageDecoder& operator=(ImageDecoder&&) noexcept = default;

  // ImageDecoder is not copyable.
  ImageDecoder(const ImageDecoder&) = delete;
  ImageDecoder& operator=(const ImageDecoder&) = delete;

  ~ImageDecoder() = default;

  // Returns the bytes of this buffer.
  absl::StatusOr<Samples> ReadSamples() &&;
  // Reads samples into the provided raw byte buffer.
  // The buffer must be large enough to hold the decoded image.
  absl::Status ReadSamplesIntoRaw(absl::Span<uint8_t> buffer) &&;
  // Read samples into the provided buffer.
  // The buffer must be the correct size and match the pixel type of the image.
  absl::Status ReadSamplesInto(absl::Span<uint8_t> buffer) &&;
  // Read samples into the provided buffer.
  // The buffer must be the correct size and match the pixel type of the image.
  absl::Status ReadSamplesInto(absl::Span<uint16_t> buffer) &&;
  // Read samples into the provided buffer.
  // The buffer must be the correct size and match the pixel type of the image.
  absl::Status ReadSamplesInto(absl::Span<float> buffer) &&;
  // Returns the width of the image in this buffer.
  uint64_t GetWidth();
  // Returns the height of the image in this buffer.
  uint64_t GetHeight();
  // Returns the pixel type of the image data produced by this decoder.
  PixelType GetPixelType();
  // Returns the color type of the image data produced by this decoder.
  ColorType GetColorType();
  // Returns the stride information of the image data produced by this decoder.
  Strides GetStrides();
  // Returns the format of the image.
  Format GetFormat() const;
  // Whether the image is originally a CMYK image (this class will convert
  // it and return an RGB anyways).
  bool IsCmyk() const;
  // Whether the input image used an indexed color palette.
  // Note: Currently only relevant for PNG images.
  bool HasPalette() const;
  // Returns the bit depth of an image. Note that this only indicates the
  // depth as encoded and may not necessarily indicate the bit depth at which
  // an image would be decoded at via the decoder (e.g. if a codec's support
  // for that depth is unimplemented). It also does not necessarily indicate
  // whether an image is decodable. Returns `std::nullopt` if this cannot be
  // determined for a given image (does not mean the image is invalid).
  std::optional<uint8_t> GetInputBitDepth() const;
  // Returns the ICC color profile embedded in the image, or
  // std::nullopt if the image does not have one. For formats that
  // don’t support embedded profiles this function should always return
  // std::nullopt. Fails if the ICC profile is not parsable.
  absl::StatusOr<std::optional<std::string>> GetIccProfile();
  // Returns the Exif metadata embedded in the image, or
  // std::nullopt if the image does not have one. For formats that
  // don’t support embedded metadata this function should always return
  // std::nullopt. Fails if the Exif metadata is not parsable.
  absl::StatusOr<std::optional<std::string>> GetExifMetadata();
  // Returns the XMP metadata embedded in the image, or
  // std::nullopt if the image does not have one. For formats that
  // don’t support embedded metadata this function should always return
  // std::nullopt. Fails if the XMP metadata is not parsable.
  absl::StatusOr<std::optional<std::string>> GetXmpMetadata();
  // Returns the IPTC metadata embedded in the image, or
  // std::nullopt if the image does not have one. For formats that
  // don’t support embedded metadata this function should always return
  // std::nullopt. Fails if the IPTC metadata is not parsable.
  absl::StatusOr<std::optional<std::string>> GetIptcMetadata();
  // Sets the background color for transparent pixels if available. Since this
  // can decode to multitude of different bit-depths, this allows overriding for
  // 8-bit and 16-bit respectively. This leaves the input untouched if no alpha
  // channel is present.
  void SetBackgroundColor(std::array<uint8_t, 4> color_8bit,
                          std::array<uint16_t, 4> color_16bit);
  // Sets the memory limit for the decoder.
  void SetLimits(uint64_t max_alloc);
  // Whether this decoder is able to return a series of animated frames.
  bool IsAnimated() const;
  // Returns all animated frames of the image. This only returns data for
  // decoders, where `is_animated` returns true.
  absl::StatusOr<Frames> GetAllFrames() &&;

 private:
  friend class ImageReader;
  explicit ImageDecoder(rust::image::ImageDecoder);
  rust::image::ImageDecoder decoder_;
};

// A multi-format image reader.
//
// Wraps an input reader to facilitate automatic detection of an image’s format,
// appropriate decoding method, and dispatches into the set of supported
// `ImageDecoder` implementations.
class ImageReader final {
 public:
  explicit ImageReader(absl::string_view input);
  static absl::StatusOr<ImageReader> NewFromFile(absl::string_view filepath);

  // ImageReader is movable.
  ImageReader(ImageReader&&) noexcept = default;
  ImageReader& operator=(ImageReader&&) noexcept = default;

  // ImageReader is not copyable.
  ImageReader(const ImageReader&) = delete;
  ImageReader& operator=(const ImageReader&) = delete;

  ~ImageReader() = default;

  // Supply the format as which to interpret the read image.
  void SetFormat(Format format);
  // Convert the reader into a decoder.
  //
  // Guesses the format and constructs the correct decoder for the format.
  absl::StatusOr<ImageDecoder> IntoDecoder() &&;

 private:
  explicit ImageReader(rust::reader::ImageReader reader);
  rust::reader::ImageReader reader_;
};

}  // namespace security::pixel_bridge

#endif  // SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_H_
