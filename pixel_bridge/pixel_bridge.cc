#include "pixel_bridge.h"

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "monitoring/streamz/public/counter.h"
#include "rust.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/match.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::pixel_bridge {

namespace {

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

absl::string_view StringViewFromVecU8(
    const rust::vec_u8::VecU8& vec) {
  absl::Span<const uint8_t> data = absl::MakeSpan(vec.as_ptr(), vec.len());
  return StringViewFromSpanU8(data);
}

absl::StatusOr<std::optional<std::string>> GetMetadata(
    rust::image::ResultOptionVecU8 result_stat) {
  if (result_stat.is_err()) {
    // NOTE: b/351976355 - Find a better error code for this Rust error.
    return absl::InternalError(
        StringViewFromVecU8(std::move(result_stat).unwrap_err()));
  }
  rust::image::OptionVecU8 result_opt =
      std::move(result_stat).unwrap();
  if (result_opt.is_none()) {
    return std::nullopt;
  }
  return std::string(StringViewFromVecU8(std::move(result_opt).unwrap()));
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
  }
}

auto* const rust_panic_counter = monitoring::streamz::Counter<std::string>::New(
    "/security/ise_memory_safety/image_processing/pixel_bridge/"
    "rust_panic_count",
    "format",
    monitoring::streamz::Metadata(
        "Count of Rust panics caught in pixel_bridge"));

void RecordPanicIfApplicable(absl::string_view message, Format format) {
  if (absl::StartsWith(message, "Rust panic caught")) {
    rust_panic_counter->Increment(FormatToString(format));
  }
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
  rust::image::OptionFrame result = frames_.curr_frame_and_advance();
  if (result.is_none()) {
    return std::nullopt;
  }
  return Frame(std::move(result).unwrap());
}

ImageDecoder::ImageDecoder(rust::image::ImageDecoder decoder)
    : decoder_(std::move(decoder)) {}

uint64_t ImageDecoder::GetWidth() { return decoder_.width(); }

uint64_t ImageDecoder::GetHeight() { return decoder_.height(); }

absl::StatusOr<ImageDecoder::Samples> ImageDecoder::ReadSamples() && {
  absl::Status status;
  switch (decoder_.pixel_type().tag) {
    case rust::image::PixelType::Tag::U8: {
      std::vector<uint8_t> result(decoder_.total_bytes());
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case rust::image::PixelType::Tag::U16: {
      // This is a uint16_t vector, respect that when calculating the size.
      std::vector<uint16_t> result(decoder_.total_bytes() / sizeof(uint16_t));
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
    case rust::image::PixelType::Tag::F32: {
      // This is a float vector, respect that when calculating the size.
      std::vector<float> result(decoder_.total_bytes() / sizeof(float));
      status = std::move(*this).ReadSamplesInto(absl::MakeSpan(result));
      if (!status.ok()) {
        return status;
      }
      return result;
    }
  }
}

absl::Status ImageDecoder::ReadSamplesIntoRaw(absl::Span<uint8_t> buffer) && {
  Format format = GetFormat();
  absl::Status status = std::move(decoder_).read_u8_slice(buffer).status();
  RecordPanicIfApplicable(status.message(), format);
  return status;
}

absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint8_t> buffer) && {
  CHECK_EQ(decoder_.pixel_type().tag,
           rust::image::PixelType::Tag::U8);
  Format format = GetFormat();
  absl::Status status = std::move(decoder_).read_u8_slice(buffer).status();
  RecordPanicIfApplicable(status.message(), format);
  return status;
}
absl::Status ImageDecoder::ReadSamplesInto(absl::Span<uint16_t> buffer) && {
  CHECK_EQ(decoder_.pixel_type().tag,
           rust::image::PixelType::Tag::U16);
  Format format = GetFormat();
  absl::Status status = std::move(decoder_).read_u16_slice(buffer).status();
  RecordPanicIfApplicable(status.message(), format);
  return status;
}
absl::Status ImageDecoder::ReadSamplesInto(absl::Span<float> buffer) && {
  CHECK_EQ(decoder_.pixel_type().tag,
           rust::image::PixelType::Tag::F32);
  Format format = GetFormat();
  absl::Status status = std::move(decoder_).read_f32_slice(buffer).status();
  RecordPanicIfApplicable(status.message(), format);
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
  decoder_.set_limits(max_alloc);
}

bool ImageDecoder::IsAnimated() const { return decoder_.is_animated(); }

absl::StatusOr<Frames> ImageDecoder::GetAllFrames() && {
  Format format = GetFormat();
  rust::image::ResultFrames result =
      std::move(decoder_).all_frames();
  if (!result.is_ok()) {
    rust::vec_u8::VecU8 err_msg_vec = std::move(result).unwrap_err();
    absl::string_view err_msg = StringViewFromVecU8(err_msg_vec);
    RecordPanicIfApplicable(err_msg, format);
    return absl::InternalError(err_msg);
  }

  return Frames(std::move(result).unwrap());
}

PixelType ImageDecoder::GetPixelType() {
  switch (decoder_.pixel_type().tag) {
    case rust::image::PixelType::Tag::U8:
      return PixelType::kUint8;
    case rust::image::PixelType::Tag::U16:
      return PixelType::kUint16;
    case rust::image::PixelType::Tag::F32:
      return PixelType::kFloat32;
  }
}

ColorType ImageDecoder::GetColorType() {
  switch (decoder_.color_type().tag) {
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

ChromaSubsampling ImageDecoder::GetChromaSubsampling() {
  switch (decoder_.chroma_subsampling().tag) {
    case rust::image::ChromaSubsampling::Tag::Unknown:
      return ChromaSubsampling::kUnknown;
    case rust::image::ChromaSubsampling::Tag::CS444:
      return ChromaSubsampling::kCs444;
    case rust::image::ChromaSubsampling::Tag::CS422:
      return ChromaSubsampling::kCs422;
    case rust::image::ChromaSubsampling::Tag::CS420:
      return ChromaSubsampling::kCs420;
    case rust::image::ChromaSubsampling::Tag::CS411:
      return ChromaSubsampling::kCs411;
    case rust::image::ChromaSubsampling::Tag::CS410:
      return ChromaSubsampling::kCs410;
  }
}

bool ImageDecoder::IsCmyk() const { return decoder_.is_cmyk(); }

bool ImageDecoder::HasPalette() const { return decoder_.has_palette(); }

std::optional<uint8_t> ImageDecoder::GetInputBitDepth() const {
  rust::OptionU8 result = decoder_.bit_depth();
  if (result.is_none()) {
    return std::nullopt;
  }
  return std::move(result).unwrap();
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

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetExtendedXmpGuid() {
  return GetMetadata(decoder_.extended_xmp_guid());
}

absl::StatusOr<std::optional<std::string>>
ImageDecoder::GetExtendedXmpMetadata() {
  return GetMetadata(decoder_.extended_xmp_metadata());
}

absl::StatusOr<std::optional<std::string>> ImageDecoder::GetIptcMetadata() {
  return GetMetadata(decoder_.iptc_metadata());
}

absl::StatusOr<ImageReader> ImageReader::NewFromFile(
    absl::string_view filepath) {
  rust::reader::ResultImageReader reader =
      rust::reader::ImageReader::new_from_file(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(filepath.data()),
              filepath.size()));
  if (reader.is_err()) {
    // NOTE: b/351976355 - Find a better error code for this Rust error.
    return absl::InternalError(
        StringViewFromVecU8(std::move(reader).unwrap_err()));
  }
  return ImageReader(std::move(reader).unwrap());
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

void ImageReader::SetJpegStrictMode(bool strict_mode) {
  reader_.set_jpeg_strict_mode(strict_mode);
}

void ImageReader::SetPngIgnoreChecksums(bool ignore_checksums) {
  reader_.set_png_ignore_checksums(ignore_checksums);
}

absl::StatusOr<ImageDecoder> ImageReader::IntoDecoder() && {
  rust::image::ResultImageDecoder decoder =
      std::move(reader_).into_decoder();
  if (!decoder.is_ok()) {
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(decoder).unwrap_err()));
  }
  return ImageDecoder(std::move(decoder).unwrap());
}

}  // namespace security::pixel_bridge
