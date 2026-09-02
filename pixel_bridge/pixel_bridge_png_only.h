#ifndef SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_PNG_ONLY_H_
#define SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_PNG_ONLY_H_

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <variant>
#include <vector>

#include "pixel_bridge.h"  // For common types like Format
#include "rust/pixel_bridge_png_only_rs.h"
#include "absl/base/attributes.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::pixel_bridge::png_only {

namespace pixel_bridge_rs = ::pixel_bridge_png_only_rs;

class Frame final {
 public:
  Frame(Frame&&) noexcept = default;
  Frame& operator=(Frame&&) noexcept = default;
  ~Frame() = default;

  uint64_t GetDelayMs() const;
  std::string GetImage();
  absl::string_view GetImageRef() ABSL_ATTRIBUTE_LIFETIME_BOUND;

 private:
  friend class Frames;
  explicit Frame(rust::image::Frame frame);
  rust::image::Frame frame_;
};

class Frames final {
 public:
  Frames(Frames&&) noexcept = default;
  Frames& operator=(Frames&&) noexcept = default;
  ~Frames() = default;

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
  ImageDecoder(ImageDecoder&&) noexcept = default;
  ImageDecoder& operator=(ImageDecoder&&) noexcept = default;
  ImageDecoder(const ImageDecoder&) = delete;
  ImageDecoder& operator=(const ImageDecoder&) = delete;
  ~ImageDecoder() = default;

  absl::StatusOr<Samples> ReadSamples() &&;
  absl::Status ReadSamplesIntoRaw(absl::Span<uint8_t> buffer) &&;
  absl::Status ReadSamplesInto(absl::Span<uint8_t> buffer) &&;
  absl::Status ReadSamplesInto(absl::Span<uint16_t> buffer) &&;
  absl::Status ReadSamplesInto(absl::Span<float> buffer) &&;
  uint64_t GetWidth();
  uint64_t GetHeight();
  PixelType GetPixelType();
  ColorType GetColorType();
  Strides GetStrides();
  Format GetFormat() const;
  bool IsCmyk() const;
  bool HasPalette() const;
  std::optional<uint8_t> GetInputBitDepth() const;
  ChromaSubsampling GetChromaSubsampling();
  absl::StatusOr<std::optional<std::string>> GetIccProfile();
  absl::StatusOr<std::optional<std::string>> GetExifMetadata();
  absl::StatusOr<std::optional<std::string>> GetXmpMetadata();
  absl::StatusOr<std::optional<std::string>> GetExtendedXmpGuid();
  absl::StatusOr<std::optional<std::string>> GetExtendedXmpMetadata();
  absl::StatusOr<std::optional<std::string>> GetIptcMetadata();
  void SetBackgroundColor(std::array<uint8_t, 4> color_8bit,
                          std::array<uint16_t, 4> color_16bit);
  void SetLimits(uint64_t max_alloc);
  bool IsAnimated() const;
  absl::StatusOr<Frames> GetAllFrames() &&;

 private:
  friend class ImageReader;
  explicit ImageDecoder(rust::image::ImageDecoder);
  rust::image::ImageDecoder decoder_;
};

class ImageReader final {
 public:
  explicit ImageReader(absl::string_view input);
  static absl::StatusOr<ImageReader> NewFromFile(absl::string_view filepath);
  ImageReader(ImageReader&&) noexcept = default;
  ImageReader& operator=(ImageReader&&) noexcept = default;
  ImageReader(const ImageReader&) = delete;
  ImageReader& operator=(const ImageReader&) = delete;
  ~ImageReader() = default;
  void SetFormat(Format format);
  void SetJpegStrictMode(bool strict_mode);
  void SetPngIgnoreChecksums(bool ignore_checksums);
  absl::StatusOr<ImageDecoder> IntoDecoder() &&;

 private:
  explicit ImageReader(rust::reader::ImageReader reader);
  rust::reader::ImageReader reader_;
};

}  // namespace security::pixel_bridge::png_only

#endif  // SECURITY_ISE_MEMORY_SAFETY_IMAGE_PROCESSING_PIXEL_BRIDGE_PIXEL_BRIDGE_PNG_ONLY_H_
