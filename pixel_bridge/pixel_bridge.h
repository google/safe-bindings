#ifndef SAFE_BINDINGS_PIXEL_BRIDGE_H_
#define SAFE_BINDINGS_PIXEL_BRIDGE_H_

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <variant>
#include <vector>

#include "crubit/rust.h"
#include "absl/base/attributes.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::pixel_bridge {

// An enumeration over the types that back the value of a pixel.
using PixelType = ::rust::image::PixelType;

// An enumeration over supported color types.
using ColorType = ::rust::image::ColorType;

// Controls how 16-bit integer samples are narrowed to 8-bit integer samples:
// - `DownscaleMode::kTruncateHighByte`: retains the most significant byte
//   (`sample >> 8`), matching libpng's `png_set_strip_16`.
// - `DownscaleMode::kRound`: rounds `sample * 255 / 65535` via
//   `(sample + 128) / 257`, matching the Rust `image` crate's
//   `DynamicImage::to_*8` conversions.
using DownscaleMode = ::rust::image::DownscaleMode;

// Target pixel layout for `ImageDecoder::ReadSamplesConverted` and
// `ImageDecoder::ReadSamplesConvertedIntoRaw`.
//
// Supported conversions:
// - `PixelType::kUint8` <-> `PixelType::kUint16` (`uint8_t` widens to
//   `uint16_t` via `(v << 8) | v`; `uint16_t` narrows to `uint8_t` according
//   to `downscale_mode`). `PixelType::kFloat32` is supported only when both
//   the source and target pixel and color types match.
// - Any `ColorType` (`kL`, `kLa`, `kRgb`, `kRgba`) to any `ColorType`:
//   - Expanding `kL` / `kLa` to `kRgb` / `kRgba` replicates luminance across
//     R, G, and B.
//   - Adding alpha (`kL` -> `kLa`, `kL` -> `kRgba`, `kRgb` -> `kRgba`) sets
//     alpha to the maximum representable sample value (`255` or `65535`).
//   - Stripping alpha (`kLa` -> `kL`, `kLa` -> `kRgb`, `kRgba` -> `kRgb`)
//     drops the alpha channel (after applying `SetBackgroundColor`, if set).
//   - Narrowing `kRgb` / `kRgba` to `kL` / `kLa` computes rounded ITU-R
//     BT.709 / sRGB luma `(2126 * r + 7152 * g + 722 * b + 5000) / 10000`.
struct ConversionOptions final {
  ColorType color_type = ColorType::kRgba;
  PixelType pixel_type = PixelType::kUint8;
  DownscaleMode downscale_mode = DownscaleMode::kTruncateHighByte;
  // Byte stride between destination rows for `ReadSamplesConvertedIntoRaw`
  // when `GetHeight() > 1` (ignored by `ReadSamplesConvertedIntoRaw` when
  // `GetHeight() <= 1`). When 0, rows are tightly packed
  // (`width * channels * sizeof(sample)`). When non-zero and
  // `GetHeight() > 1`, must be at least the tightly packed row size.
  // `ReadSamplesConverted` requires `row_bytes` to be 0 or equal to the
  // tightly packed row size.
  size_t row_bytes = 0;
  // When true and `GetHeight() > 1`, writes source row 0 at destination row
  // index `height - 1` and steps subsequent rows toward index 0 (supporting
  // negative `row_bytes` bottom-up buffers in C++).
  bool rows_bottom_up = false;
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
  kJxl,
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
  // Reads and converts samples according to `options`, returning a tightly
  // packed `std::vector<uint8_t>`, `std::vector<uint16_t>`, or
  // `std::vector<float>` matching `options.pixel_type`.
  absl::StatusOr<Samples> ReadSamplesConverted(
      const ConversionOptions& options) &&;
  // Reads and converts samples into `buffer` according to `options` (including
  // optional `options.row_bytes` padding between rows and
  // `options.rows_bottom_up` row ordering). Multi-byte samples are
  // written in native-endian byte order without requiring `buffer` or
  // `options.row_bytes` to be aligned to `sizeof(uint16_t)`. Prefer this
  // overload with a pre-allocated buffer when decoding large images to avoid
  // allocating and zero-initializing a fresh `std::vector` per call.
  absl::Status ReadSamplesConvertedIntoRaw(absl::Span<uint8_t> buffer,
                                           const ConversionOptions& options) &&;
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

#endif  // SAFE_BINDINGS_PIXEL_BRIDGE_H_
