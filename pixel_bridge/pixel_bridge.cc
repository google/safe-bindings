#include "pixel_bridge.h"

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
#include "support/rs_std/result.h"

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

absl::StatusOr<std::optional<std::string>> GetMetadata(
    rs_std::Result<rs_std::Option<rust::vec_u8::VecU8>,
                   rust::vec_u8::VecU8>
        result_stat) {
  if (!result_stat.has_value()) {
    // NOTE: b/351976355 - Find a better error code for this Rust error.
    return absl::InternalError(
        StringViewFromVecU8(std::move(result_stat).err()));
  }
  std::optional<rust::vec_u8::VecU8> result_opt =
      std::move(result_stat).value();
  if (!result_opt.has_value()) {
    return std::nullopt;
  }
  return std::string(StringViewFromVecU8(std::move(result_opt).value()));
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

uint64_t ImageDecoder::GetWidth() { return decoder_.width(); }

uint64_t ImageDecoder::GetHeight() { return decoder_.height(); }

absl::StatusOr<ImageDecoder::Samples> ImageDecoder::ReadSamples() && {
  absl::Status status;
  switch (decoder_.pixel_type()) {
    case PixelType::kUint8: {
      std::vector<uint8_t> result(decoder_.total_bytes());
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kUint16: {
      // This is a uint16_t vector, respect that when calculating the size.
      std::vector<uint16_t> result(decoder_.total_bytes() / sizeof(uint16_t));
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kFloat32: {
      // This is a float vector, respect that when calculating the size.
      std::vector<float> result(decoder_.total_bytes() / sizeof(float));
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
  }
  return absl::InternalError("Unknown pixel type");
}

absl::Status ImageDecoder::ReadSamplesIntoRaw(absl::Span<uint8_t> buffer) && {
  Format format = GetFormat();
  absl::Status status = ToStatus(std::move(decoder_).read_u8_slice(buffer));
  return status;
}

absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint8_t> buffer) && {
  CHECK_EQ(decoder_.pixel_type(), PixelType::kUint8);
  Format format = GetFormat();
  absl::Status status = ToStatus(std::move(decoder_).read_u8_slice(buffer));
  return status;
}
absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint16_t> buffer) && {
  CHECK_EQ(decoder_.pixel_type(), PixelType::kUint16);
  Format format = GetFormat();
  absl::Status status = ToStatus(std::move(decoder_).read_u16_slice(buffer));
  return status;
}
absl::Status ImageDecoder::ReadSamplesInto(absl::Span<float> buffer) && {
  CHECK_EQ(decoder_.pixel_type(), PixelType::kFloat32);
  Format format = GetFormat();
  absl::Status status = ToStatus(std::move(decoder_).read_f32_slice(buffer));
  return status;
}

absl::StatusOr<ImageDecoder::Samples> ImageDecoder::ReadSamplesConverted(
    const ConversionOptions& options) && {
  rs_std::Result<uint64_t, rust::vec_u8::VecU8> total_samples_res =
      decoder_.converted_packed_samples(options.color_type, options.pixel_type,
                                        options.downscale_mode,
                                        options.row_bytes);
  if (!total_samples_res.has_value()) {
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(total_samples_res).err()));
  }
  const size_t total_samples =
      static_cast<size_t>(std::move(total_samples_res).value());

  switch (options.pixel_type) {
    case PixelType::kUint8: {
      std::vector<uint8_t> result(total_samples);
      absl::Status status = std::move(*this).ReadSamplesConvertedIntoRaw(
          absl::MakeSpan(result), options);
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kUint16: {
      std::vector<uint16_t> result(total_samples);
      absl::Status status = std::move(*this).ReadSamplesConvertedIntoRaw(
          absl::MakeSpan(reinterpret_cast<uint8_t*>(result.data()),
                         result.size() * sizeof(uint16_t)),
          options);
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case PixelType::kFloat32: {
      std::vector<float> result(total_samples);
      absl::Status status = std::move(*this).ReadSamplesConvertedIntoRaw(
          absl::MakeSpan(reinterpret_cast<uint8_t*>(result.data()),
                         result.size() * sizeof(float)),
          options);
      if (!status.ok()) {
        return status;
      }
      return result;
    }
  }
  return absl::InvalidArgumentError("Invalid pixel_type in ConversionOptions");
}

absl::Status ImageDecoder::ReadSamplesConvertedIntoRaw(
    absl::Span<uint8_t> buffer, const ConversionOptions& options) && {
  // Crubit `absl::Span` to Rust slice conversion panics if the slice byte
  // length exceeds `isize::MAX` (`ptrdiff_t::max()`).
  if (buffer.size() >
      static_cast<size_t>(std::numeric_limits<ptrdiff_t>::max())) {
    return absl::InvalidArgumentError(
        "Destination buffer exceeds maximum supported slice size");
  }

  absl::Status status = ToStatus(std::move(decoder_).read_converted_u8_slice(
      buffer, options.color_type, options.pixel_type, options.downscale_mode,
      options.row_bytes, options.rows_bottom_up));
  return status;
}

void ImageDecoder::SetBackgroundColor(std::array<uint8_t, 4> color_8bit,
                                      std::array<uint16_t, 4> color_16bit) {
  uint32_t color_8_bit = (static_cast<uint32_t>(color_8bit[3]) << 24) |
                         (static_cast<uint32_t>(color_8bit[2]) << 16) |
                         (static_cast<uint32_t>(color_8bit[1]) << 8) |
                         (static_cast<uint32_t>(color_8bit[0]));
  uint64_t color_16_bit = (static_cast<uint64_t>(color_16bit[3]) << 48) |
                          (static_cast<uint64_t>(color_16bit[2]) << 32) |
                          (static_cast<uint64_t>(color_16bit[1]) << 16) |
                          (static_cast<uint64_t>(color_16bit[0]));

  decoder_.set_background_sustitution(color_8_bit, color_16_bit);
}

void ImageDecoder::SetLimits(uint64_t max_alloc) {
  static_cast<void>(decoder_.set_limits(max_alloc));
}

bool ImageDecoder::IsAnimated() const { return decoder_.is_animated(); }

absl::StatusOr<Frames> ImageDecoder::GetAllFrames() && {
  Format format = GetFormat();
  rs_std::Result<rust::image::Frames, rust::vec_u8::VecU8>
      result = std::move(decoder_).all_frames();
  if (!result.has_value()) {
    rust::vec_u8::VecU8 err_msg_vec = std::move(result).err();
    absl::string_view err_msg = StringViewFromVecU8(err_msg_vec);
    return absl::InternalError(err_msg);
  }

  return Frames(std::move(result).value());
}

PixelType ImageDecoder::GetPixelType() { return decoder_.pixel_type(); }

ColorType ImageDecoder::GetColorType() { return decoder_.color_type(); }

Strides ImageDecoder::GetStrides() {
  rust::image::Strides strides = decoder_.strides();
  return Strides{
      .width = strides.width,
      .height = strides.height,
      .channels = strides.channels,
  };
}

Format ImageDecoder::GetFormat() const {
  return FromRustFormat(decoder_.format());
}

bool ImageDecoder::IsCmyk() const { return decoder_.is_cmyk(); }

bool ImageDecoder::HasPalette() const { return decoder_.has_palette(); }

std::optional<uint8_t> ImageDecoder::GetInputBitDepth() const {
  return decoder_.bit_depth();
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetIccProfile() {
  return GetMetadata(decoder_.icc_profile());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetExifMetadata() {
  return GetMetadata(decoder_.exif_metadata());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetXmpMetadata() {
  return GetMetadata(decoder_.xmp_metadata());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetIptcMetadata() {
  return GetMetadata(decoder_.iptc_metadata());
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

absl::StatusOr<ImageDecoder> ImageReader::IntoDecoder() && {
  rs_std::Result<rust::image::ImageDecoder,
                 rust::vec_u8::VecU8>
      decoder = std::move(reader_).into_decoder();
  if (!decoder.has_value()) {
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(decoder).err()));
  }
  return ImageDecoder(std::move(decoder).value());
}

}  // namespace security::pixel_bridge
