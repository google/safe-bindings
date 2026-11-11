#include "pixel_bridge.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/match.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::pixel_bridge {

namespace {

using ::security::crubit_helpers::StringViewFromVecU8;

rust::reader::Format ToRustFormat(Format format) {
  rust::reader::Format result;
  switch (format) {
    case Format::kPng:
      result.tag = rust::reader::Format::Tag::Png;
      break;
    case Format::kJpeg:
      result.tag = rust::reader::Format::Tag::Jpeg;
      break;
    case Format::kGif:
      result.tag = rust::reader::Format::Tag::Gif;
      break;
    case Format::kWebP:
      result.tag = rust::reader::Format::Tag::WebP;
      break;
    case Format::kTiff:
      result.tag = rust::reader::Format::Tag::Tiff;
      break;
    case Format::kBmp:
      result.tag = rust::reader::Format::Tag::Bmp;
      break;
    case Format::kIco:
      result.tag = rust::reader::Format::Tag::Ico;
      break;
    case Format::kJxl:
      result.tag = rust::reader::Format::Tag::Jxl;
      break;
  }
  return result;
}

Format FromRustFormat(rust::reader::Format format) {
  switch (format.tag) {
    case rust::reader::Format::Tag::Png:
      return Format::kPng;
    case rust::reader::Format::Tag::Jpeg:
      return Format::kJpeg;
    case rust::reader::Format::Tag::Gif:
      return Format::kGif;
    case rust::reader::Format::Tag::WebP:
      return Format::kWebP;
    case rust::reader::Format::Tag::Tiff:
      return Format::kTiff;
    case rust::reader::Format::Tag::Bmp:
      return Format::kBmp;
    case rust::reader::Format::Tag::Ico:
      return Format::kIco;
    case rust::reader::Format::Tag::Jxl:
      return Format::kJxl;
  }
}

absl::string_view StringViewFromSpanU8(absl::Span<const uint8_t> data) {
  static_assert(sizeof(const char) == sizeof(const uint8_t),
                "If the size of char and uint8_t differ, then the size of the "
                "uint8_t array is not the number of chars we should copy, as "
                "we do a few lines below.");
  // The standard should actually guarantee that the alignment of `char` is the
  // lowest possible alignment value, 1. But let's double-check that we and the
  // compiler are on the same page.
  static_assert(alignof(const char) <= alignof(const uint8_t),
                "We need to keep the pointer to `vec`'s data aligned after the "
                "conversion to char.");
  return absl::string_view(reinterpret_cast<const char*>(data.data()),
                           data.size());
}

std::string FormatToString(Format format) {
  switch (format) {
    case Format::kPng:
      return "png";
    case Format::kJpeg:
      return "jpeg";
    case Format::kGif:
      return "gif";
    case Format::kWebP:
      return "webp";
    case Format::kTiff:
      return "tiff";
    case Format::kBmp:
      return "bmp";
    case Format::kIco:
      return "ico";
    case Format::kJxl:
      return "jxl";
  }
}

absl::StatusOr<std::optional<std::string>> GetMetadata(
    rs_std::Result<rs_std::Option<rust::vec_u8::VecU8>,
                   rust::vec_u8::VecU8>
        result_stat,
    Format format) {
  if (!result_stat.has_value()) {
    rust::vec_u8::VecU8 err_msg_vec = std::move(result_stat).err();
    absl::string_view err_msg = StringViewFromVecU8(err_msg_vec);
    static_cast<void>(format);
    if (absl::StartsWith(err_msg, "Failed to decode image: Limits(")) {
      return absl::InvalidArgumentError(err_msg);
    }
    // NOTE: b/351976355 - Find a better error code for this Rust error.
    return absl::InternalError(err_msg);
  }
  std::optional<rust::vec_u8::VecU8> result_opt =
      std::move(result_stat).value();
  if (!result_opt.has_value()) {
    return std::nullopt;
  }
  return std::string(StringViewFromVecU8(std::move(result_opt).value()));
}

constexpr uint64_t NumChannels(ColorType color_type) {
  switch (color_type) {
    case ColorType::kL:
      return 1;
    case ColorType::kLa:
      return 2;
    case ColorType::kRgb:
      return 3;
    case ColorType::kRgba:
      return 4;
  }
}

constexpr uint64_t BytesPerSample(PixelType pixel_type) {
  switch (pixel_type) {
    case PixelType::kUint8:
      return sizeof(uint8_t);
    case PixelType::kUint16:
      return sizeof(uint16_t);
    case PixelType::kFloat32:
      return sizeof(float);
  }
}

// Upper bound on allocations while parsing headers and metadata in
// `ImageReader::IntoDecoder()` (e.g. PNG iCCP decompression), regardless of the
// limit set by the caller.
constexpr uint64_t kMaxHeaderAlloc = uint64_t{64} * 1024 * 1024;

constexpr std::optional<uint64_t> CheckedMulU64(uint64_t lhs, uint64_t rhs) {
  if (lhs != 0 && rhs > std::numeric_limits<uint64_t>::max() / lhs) {
    return std::nullopt;
  }
  return lhs * rhs;
}

inline absl::Status ToStatus(rust::image::Status status) {
  if (status.has_value()) {
    return absl::OkStatus();
  }
  return absl::InternalError(StringViewFromVecU8(std::move(status).err()));
}

}  // namespace

Frame::Frame(rust::image::Frame frame) : frame_(std::move(frame)) {}

uint64_t Frame::GetDelayMs() const { return frame_.curr_delay_ms(); }

std::string Frame::GetImage() { return std::string(GetImageRef()); }

absl::string_view Frame::GetImageRef() {
  absl::Span<const uint8_t> image_data = frame_.image_ref();
  return StringViewFromSpanU8(image_data);
}

Frames::Frames(rust::image::Frames frames)
    : frames_(std::move(frames)) {}

std::optional<Frame> Frames::GetCurrentFrameAndAdvance() {
  std::optional<rust::image::Frame> result =
      frames_.curr_frame_and_advance();
  if (!result.has_value()) {
    return std::nullopt;
  }
  return Frame(std::move(result).value());
}

ImageDecoder::ImageDecoder(rust::image::ImageDecoder decoder)
    : decoder_(std::move(decoder)) {}

absl::Status ImageDecoder::CheckUsable() const {
  if (!decoder_.has_value()) {
    return absl::FailedPreconditionError(
        "ImageDecoder was moved from or already consumed");
  }
  return limits_error_;
}

rust::image::ImageDecoder ImageDecoder::TakeDecoder() {
  rust::image::ImageDecoder decoder = *std::move(decoder_);
  decoder_.reset();
  return decoder;
}

uint64_t ImageDecoder::GetWidth() const {
  if (!decoder_.has_value()) return 0;
  return decoder_->width();
}

uint64_t ImageDecoder::GetHeight() const {
  if (!decoder_.has_value()) return 0;
  return decoder_->height();
}

absl::StatusOr<ImageDecoder::Samples> ImageDecoder::ReadSamples() && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;

  absl::Status limit_status = ToStatus(decoder_->check_read_limits());
  if (!limit_status.ok()) {
    return limit_status;
  }

  const uint64_t width = GetWidth();
  const uint64_t height = GetHeight();
  const PixelType pixel_type = GetPixelType();
  const uint64_t channels = NumChannels(GetColorType());
  const uint64_t bytes_per_sample = BytesPerSample(pixel_type);

  const std::optional<uint64_t> pixel_count = CheckedMulU64(width, height);
  const std::optional<uint64_t> sample_count =
      pixel_count.has_value() ? CheckedMulU64(*pixel_count, channels)
                              : std::nullopt;
  const std::optional<uint64_t> total_bytes =
      sample_count.has_value() ? CheckedMulU64(*sample_count, bytes_per_sample)
                               : std::nullopt;
  if (!total_bytes.has_value() ||
      *total_bytes >
          static_cast<uint64_t>(std::numeric_limits<ptrdiff_t>::max()) ||
      *total_bytes >
          static_cast<uint64_t>(std::numeric_limits<size_t>::max())) {
    return absl::InvalidArgumentError(
        "Failed to decode image: image dimensions overflow buffer size");
  }
  if (max_alloc_.has_value() && *total_bytes > *max_alloc_) {
    return absl::InvalidArgumentError(
        "Failed to decode image: Limits(LimitError { kind: InsufficientMemory "
        "})");
  }

  const size_t num_samples = static_cast<size_t>(*sample_count);
  absl::Status status;

  switch (pixel_type) {
    case PixelType::kUint8: {
      std::vector<uint8_t> result(num_samples);
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kUint16: {
      std::vector<uint16_t> result(num_samples);
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kFloat32: {
      std::vector<float> result(num_samples);
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
  }
}

absl::Status ImageDecoder::ReadSamplesIntoRaw(absl::Span<uint8_t> buffer) && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  absl::Status status = ToStatus(TakeDecoder().read_u8_slice(buffer));
  return status;
}

absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint8_t> buffer) && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  CHECK_EQ(decoder_->pixel_type().tag,
           rust::image::PixelType::Tag::U8);
  absl::Status status = ToStatus(TakeDecoder().read_u8_slice(buffer));
  return status;
}

absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint16_t> buffer) && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  if (reinterpret_cast<uintptr_t>(buffer.data()) % alignof(uint16_t) != 0) {
    return absl::InvalidArgumentError("Buffer is not aligned for uint16_t");
  }
  CHECK_EQ(decoder_->pixel_type().tag,
           rust::image::PixelType::Tag::U16);
  absl::Status status = ToStatus(TakeDecoder().read_u16_slice(buffer));
  return status;
}

absl::Status ImageDecoder::ReadSamplesInto(absl::Span<float> buffer) && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  if (reinterpret_cast<uintptr_t>(buffer.data()) % alignof(float) != 0) {
    return absl::InvalidArgumentError("Buffer is not aligned for float");
  }
  CHECK_EQ(decoder_->pixel_type().tag,
           rust::image::PixelType::Tag::F32);
  absl::Status status = ToStatus(TakeDecoder().read_f32_slice(buffer));
  return status;
}

void ImageDecoder::SetBackgroundColor(std::array<uint8_t, 4> color_8bit,
                                      std::array<uint16_t, 4> color_16bit) {
  if (!decoder_.has_value()) return;
  uint32_t color_8_bit = (static_cast<uint32_t>(color_8bit[3]) << 24) |
                         (static_cast<uint32_t>(color_8bit[2]) << 16) |
                         (static_cast<uint32_t>(color_8bit[1]) << 8) |
                         (static_cast<uint32_t>(color_8bit[0]));
  uint64_t color_16_bit = (static_cast<uint64_t>(color_16bit[3]) << 48) |
                          (static_cast<uint64_t>(color_16bit[2]) << 32) |
                          (static_cast<uint64_t>(color_16bit[1]) << 16) |
                          (static_cast<uint64_t>(color_16bit[0]));
  decoder_->set_background_sustitution(color_8_bit, color_16_bit);
}

void ImageDecoder::SetLimits(uint64_t max_alloc) {
  max_alloc_ = max_alloc;
  if (!decoder_.has_value()) return;
  absl::Status status = ToStatus(decoder_->set_limits(max_alloc));
  limits_error_ = std::move(status);
}

bool ImageDecoder::IsAnimated() const {
  if (!decoder_.has_value()) return false;
  return decoder_->is_animated();
}

absl::StatusOr<Frames> ImageDecoder::GetAllFrames() && {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  rs_std::Result<rust::image::Frames, rust::vec_u8::VecU8>
      result = TakeDecoder().all_frames();

  if (!result.has_value()) {
    rust::vec_u8::VecU8 err_msg_vec = std::move(result).err();
    absl::string_view err_msg = StringViewFromVecU8(err_msg_vec);
    if (absl::StartsWith(err_msg, "Rust panic caught") ||
        absl::StartsWith(err_msg, "Decoder is in invalid")) {
      return absl::InternalError(err_msg);
    }
    return absl::InvalidArgumentError(err_msg);
  }

  return Frames(std::move(result).value());
}

PixelType ImageDecoder::GetPixelType() const {
  if (!decoder_.has_value()) return PixelType::kUint8;
  switch (decoder_->pixel_type().tag) {
    case rust::image::PixelType::Tag::U8:
      return PixelType::kUint8;
    case rust::image::PixelType::Tag::U16:
      return PixelType::kUint16;
    case rust::image::PixelType::Tag::F32:
      return PixelType::kFloat32;
  }
}

ColorType ImageDecoder::GetColorType() const {
  if (!decoder_.has_value()) return ColorType::kRgb;
  switch (decoder_->color_type().tag) {
    case rust::image::ColorType::Tag::L:
      return ColorType::kL;
    case rust::image::ColorType::Tag::La:
      return ColorType::kLa;
    case rust::image::ColorType::Tag::Rgb:
      return ColorType::kRgb;
    case rust::image::ColorType::Tag::Rgba:
      return ColorType::kRgba;
  }
}

Strides ImageDecoder::GetStrides() const {
  if (!decoder_.has_value()) return Strides{0, 0, 0};
  rust::image::Strides strides = decoder_->strides();
  return Strides{
      .width = strides.width,
      .height = strides.height,
      .channels = strides.channels,
  };
}

Format ImageDecoder::GetFormat() const {
  if (!decoder_.has_value()) return Format::kPng;
  return FromRustFormat(decoder_->format());
}

bool ImageDecoder::IsCmyk() const {
  if (!decoder_.has_value()) return false;
  return decoder_->is_cmyk();
}

bool ImageDecoder::HasPalette() const {
  if (!decoder_.has_value()) return false;
  return decoder_->has_palette();
}

std::optional<uint8_t> ImageDecoder::GetInputBitDepth() const {
  if (!decoder_.has_value()) return std::nullopt;
  return decoder_->bit_depth();
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetIccProfile() {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  return GetMetadata(decoder_->icc_profile(), GetFormat());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetExifMetadata() {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  return GetMetadata(decoder_->exif_metadata(), GetFormat());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetXmpMetadata() {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  return GetMetadata(decoder_->xmp_metadata(), GetFormat());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetIptcMetadata() {
  if (absl::Status s = CheckUsable(); !s.ok()) return s;
  return GetMetadata(decoder_->iptc_metadata(), GetFormat());
}

absl::StatusOr<ImageReader> ImageReader::NewFromFile(
    absl::string_view filepath) {
  rs_std::Result<rust::reader::ImageReader,
                 rust::vec_u8::VecU8>
      reader = rust::reader::ImageReader::new_from_file(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(filepath.data()),
              filepath.size()));
  if (!reader.has_value()) {
    // NOTE: b/351976355 - Find a better error code for this Rust error.
    return absl::InternalError(StringViewFromVecU8(std::move(reader).err()));
  }
  return ImageReader(std::move(reader).value());
}

ImageReader::ImageReader(absl::string_view input) {
  // The standard should actually guarantee that the alignment of `char` is the
  // lowest possible alignment value, 1. But let's double-check that we and the
  // compiler are on the same page.
  static_assert(alignof(const char) <= alignof(const uint8_t),
                "We need to keep the pointer to `vec`'s data aligned after the "
                "conversion to char.");
  reader_ = rust::reader::ImageReader::new_in_memory(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(input.data()),
                                input.size()));
}

ImageReader::ImageReader(rust::reader::ImageReader reader)
    : reader_(std::move(reader)) {}

void ImageReader::SetFormat(Format format) {
  reader_.set_format(ToRustFormat(format));
}

void ImageReader::SetLimits(uint64_t max_alloc) { max_alloc_ = max_alloc; }

absl::StatusOr<ImageDecoder> ImageReader::IntoDecoder() && {
  reader_.set_limits(
      std::min(max_alloc_.value_or(kMaxHeaderAlloc), kMaxHeaderAlloc));
  rs_std::Result<rust::image::ImageDecoder,
                 rust::vec_u8::VecU8>
      decoder_or = std::move(reader_).into_decoder();
  if (!decoder_or.has_value()) {
    rust::vec_u8::VecU8 err_msg_vec = std::move(decoder_or).err();
    absl::string_view err_msg = StringViewFromVecU8(err_msg_vec);
    if (absl::StartsWith(err_msg, "Rust panic caught")) {
      return absl::InternalError(err_msg);
    }
    return absl::InvalidArgumentError(err_msg);
  }
  ImageDecoder decoder(std::move(decoder_or).value());
  if (max_alloc_.has_value()) {
    decoder.SetLimits(*max_alloc_);
  }
  return decoder;
}

}  // namespace security::pixel_bridge
