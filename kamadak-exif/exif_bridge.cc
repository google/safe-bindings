#include "exif_bridge.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <iterator>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "support/rs_std/slice_ref.h"
#include "file/base/file.h"
#include "file/base/helpers.h"
#include "file/base/options.h"
#include "rust/exif_bridge_rs.h"
#include "tech/file/proto/types.proto.h"
#include "absl/algorithm/container.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "third_party/gloop/util/status/status_macros.h"

namespace security::exif_bridge {
namespace {

absl::StatusOr<std::vector<uint8_t>> ReadFileContents(File& file) {
  tech::file::StatProto stat;
  RETURN_IF_ERROR(file.Stat(&stat, file::Defaults()));

  const size_t length = stat.length();
  std::vector<uint8_t> contents(length);
  RETURN_IF_ERROR(file::ReadToBuffer(&file,
                                     reinterpret_cast<char*>(contents.data()),
                                     length, nullptr, file::Defaults()));
  return contents;
}

// Mapping Rust error to C++ absl::Status.
absl::Status FromRustError(exif_bridge_rs::error::Error error) {
  switch (error.status()) {
    case exif_bridge_rs::error::ErrorStatus::INVALID_FORMAT:
      return absl::InvalidArgumentError(error.message());
    case exif_bridge_rs::error::ErrorStatus::IO:
      return absl::InternalError(error.message());
    case exif_bridge_rs::error::ErrorStatus::NOT_FOUND:
      return absl::NotFoundError(error.message());
    case exif_bridge_rs::error::ErrorStatus::BLANK_VALUE:
      return absl::NotFoundError(error.message());
    case exif_bridge_rs::error::ErrorStatus::TOO_BIG:
      return absl::OutOfRangeError(error.message());
    case exif_bridge_rs::error::ErrorStatus::NOT_SUPPORTED:
      return absl::UnimplementedError(error.message());
    case exif_bridge_rs::error::ErrorStatus::UNEXPECTED_VALUE:
      return absl::InvalidArgumentError(error.message());
    case exif_bridge_rs::error::ErrorStatus::PARTIAL_RESULT:
      return absl::DataLossError(error.message());
    default:
      return absl::UnknownError(error.message());
  }
}
}  // namespace

// Value
Value::Value(exif_bridge_rs::value::Value value) : value_(std::move(value)) {}

bool Value::operator==(const Value& other) const {
  return value_.equals(other.value_);
}

Value Value::Byte(absl::Span<const uint8_t> value) {
  return Value(exif_bridge_rs::value::Value::Byte(
      exif_bridge_rs::types::VecU8::copy_from_raw(value.data(), value.size())));
}

Value Value::Ascii(absl::Span<const std::string> value) {
  std::vector<exif_bridge_rs::types::VecU8> rust_vec;
  rust_vec.reserve(value.size());
  absl::c_transform(value, std::back_inserter(rust_vec),
                    [](const std::string& s) {
                      return exif_bridge_rs::types::VecU8::copy_from_raw(
                          reinterpret_cast<const uint8_t*>(s.data()), s.size());
                    });
  return Value(exif_bridge_rs::value::Value::Ascii(
      exif_bridge_rs::types::VecVecU8::copy_from_raw(rust_vec.data(),
                                                     rust_vec.size())));
}

Value Value::Short(absl::Span<const uint16_t> value) {
  return Value(exif_bridge_rs::value::Value::Short(
      exif_bridge_rs::types::VecU16::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::Long(absl::Span<const uint32_t> value) {
  return Value(exif_bridge_rs::value::Value::Long(
      exif_bridge_rs::types::VecU32::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::Rational(absl::Span<const exif_bridge_rs::value::Rational> value) {
  return Value(exif_bridge_rs::value::Value::Rational(
      exif_bridge_rs::value::VecRational::copy_from_raw(value.data(),
                                                        value.size())));
}

Value Value::SByte(absl::Span<const int8_t> value) {
  return Value(exif_bridge_rs::value::Value::SByte(
      exif_bridge_rs::types::VecI8::copy_from_raw(value.data(), value.size())));
}

Value Value::Undefined(absl::Span<const uint8_t> value, uint32_t offset) {
  return Value(exif_bridge_rs::value::Value::Undefined(
      exif_bridge_rs::types::VecU8::copy_from_raw(value.data(), value.size()),
      offset));
}

Value Value::SShort(absl::Span<const int16_t> value) {
  return Value(exif_bridge_rs::value::Value::SShort(
      exif_bridge_rs::types::VecI16::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::SLong(absl::Span<const int32_t> value) {
  return Value(exif_bridge_rs::value::Value::SLong(
      exif_bridge_rs::types::VecI32::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::SRational(
    absl::Span<const exif_bridge_rs::value::SRational> value) {
  return Value(exif_bridge_rs::value::Value::SRational(
      exif_bridge_rs::value::VecSRational::copy_from_raw(value.data(),
                                                         value.size())));
}

Value Value::Float(absl::Span<const float> value) {
  return Value(exif_bridge_rs::value::Value::Float(
      exif_bridge_rs::types::VecF32::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::Double(absl::Span<const double> value) {
  return Value(exif_bridge_rs::value::Value::Double(
      exif_bridge_rs::types::VecF64::copy_from_raw(value.data(),
                                                   value.size())));
}

Value Value::Unknown(uint16_t type, uint32_t count, uint32_t offset) {
  return Value(exif_bridge_rs::value::Value::Unknown(type, count, offset));
}

exif_bridge_rs::reexport::Display Value::display_as(Tag tag) const {
  return value_.display_as(tag.tag_.into_inner());
}

absl::StatusOr<exif_bridge_rs::reexport::UIntValue> Value::as_uint() const {
  exif_bridge_rs::reexport::ResultUIntValue result = value_.as_uint();
  if (result.is_ok()) {
    return std::move(result).unwrap();
  }
  return FromRustError(std::move(result).unwrap_err());
}

std::optional<std::uint32_t> Value::get_uint(std::uintptr_t index) const {
  return value_.get_uint(index);
}

std::optional<std::vector<uint32_t>> Value::iter_uint() const {
  std::optional<exif_bridge_rs::types::VecU32> result = value_.iter_uint();
  if (result.has_value()) {
    return std::vector<uint32_t>(
        result.value().as_ptr(),
        result.value().as_ptr() + result.value().len());
  }
  return std::nullopt;
}

// Tag
Tag::Tag(exif_bridge_rs::tag::Tag tag) : tag_(std::move(tag)) {}

bool Tag::operator==(const Tag& other) const { return tag_.equals(other.tag_); }

bool Tag::operator<(const Tag& other) const {
  return tag_.less_than(other.tag_);
}

const Tag Tag::kAcceleration = Tag(exif_bridge_rs::tag::Tag::Acceleration());
const Tag Tag::kApertureValue = Tag(exif_bridge_rs::tag::Tag::ApertureValue());
const Tag Tag::kArtist = Tag(exif_bridge_rs::tag::Tag::Artist());
const Tag Tag::kBitsPerSample = Tag(exif_bridge_rs::tag::Tag::BitsPerSample());
const Tag Tag::kBodySerialNumber =
    Tag(exif_bridge_rs::tag::Tag::BodySerialNumber());
const Tag Tag::kBrightnessValue =
    Tag(exif_bridge_rs::tag::Tag::BrightnessValue());
const Tag Tag::kCFAPattern = Tag(exif_bridge_rs::tag::Tag::CFAPattern());
const Tag Tag::kCameraElevationAngle =
    Tag(exif_bridge_rs::tag::Tag::CameraElevationAngle());
const Tag Tag::kCameraOwnerName =
    Tag(exif_bridge_rs::tag::Tag::CameraOwnerName());
const Tag Tag::kColorSpace = Tag(exif_bridge_rs::tag::Tag::ColorSpace());
const Tag Tag::kComponentsConfiguration =
    Tag(exif_bridge_rs::tag::Tag::ComponentsConfiguration());
const Tag Tag::kCompositeImage =
    Tag(exif_bridge_rs::tag::Tag::CompositeImage());
const Tag Tag::kCompressedBitsPerPixel =
    Tag(exif_bridge_rs::tag::Tag::CompressedBitsPerPixel());
const Tag Tag::kCompression = Tag(exif_bridge_rs::tag::Tag::Compression());
const Tag Tag::kContrast = Tag(exif_bridge_rs::tag::Tag::Contrast());
const Tag Tag::kCopyright = Tag(exif_bridge_rs::tag::Tag::Copyright());
const Tag Tag::kCustomRendered =
    Tag(exif_bridge_rs::tag::Tag::CustomRendered());
const Tag Tag::kDateTime = Tag(exif_bridge_rs::tag::Tag::DateTime());
const Tag Tag::kDateTimeDigitized =
    Tag(exif_bridge_rs::tag::Tag::DateTimeDigitized());
const Tag Tag::kDateTimeOriginal =
    Tag(exif_bridge_rs::tag::Tag::DateTimeOriginal());
const Tag Tag::kDeviceSettingDescription =
    Tag(exif_bridge_rs::tag::Tag::DeviceSettingDescription());
const Tag Tag::kDigitalZoomRatio =
    Tag(exif_bridge_rs::tag::Tag::DigitalZoomRatio());
const Tag Tag::kExifVersion = Tag(exif_bridge_rs::tag::Tag::ExifVersion());
const Tag Tag::kExposureBiasValue =
    Tag(exif_bridge_rs::tag::Tag::ExposureBiasValue());
const Tag Tag::kExposureIndex = Tag(exif_bridge_rs::tag::Tag::ExposureIndex());
const Tag Tag::kExposureMode = Tag(exif_bridge_rs::tag::Tag::ExposureMode());
const Tag Tag::kExposureProgram =
    Tag(exif_bridge_rs::tag::Tag::ExposureProgram());
const Tag Tag::kExposureTime = Tag(exif_bridge_rs::tag::Tag::ExposureTime());
const Tag Tag::kFNumber = Tag(exif_bridge_rs::tag::Tag::FNumber());
const Tag Tag::kFileSource = Tag(exif_bridge_rs::tag::Tag::FileSource());
const Tag Tag::kFlash = Tag(exif_bridge_rs::tag::Tag::Flash());
const Tag Tag::kFlashEnergy = Tag(exif_bridge_rs::tag::Tag::FlashEnergy());
const Tag Tag::kFlashpixVersion =
    Tag(exif_bridge_rs::tag::Tag::FlashpixVersion());
const Tag Tag::kFocalLength = Tag(exif_bridge_rs::tag::Tag::FocalLength());
const Tag Tag::kFocalLengthIn35mmFilm =
    Tag(exif_bridge_rs::tag::Tag::FocalLengthIn35mmFilm());
const Tag Tag::kFocalPlaneResolutionUnit =
    Tag(exif_bridge_rs::tag::Tag::FocalPlaneResolutionUnit());
const Tag Tag::kFocalPlaneXResolution =
    Tag(exif_bridge_rs::tag::Tag::FocalPlaneXResolution());
const Tag Tag::kFocalPlaneYResolution =
    Tag(exif_bridge_rs::tag::Tag::FocalPlaneYResolution());
const Tag Tag::kGPSAltitude = Tag(exif_bridge_rs::tag::Tag::GPSAltitude());
const Tag Tag::kGPSAltitudeRef =
    Tag(exif_bridge_rs::tag::Tag::GPSAltitudeRef());
const Tag Tag::kGPSAreaInformation =
    Tag(exif_bridge_rs::tag::Tag::GPSAreaInformation());
const Tag Tag::kGPSDOP = Tag(exif_bridge_rs::tag::Tag::GPSDOP());
const Tag Tag::kGPSDateStamp = Tag(exif_bridge_rs::tag::Tag::GPSDateStamp());
const Tag Tag::kGPSDestBearing =
    Tag(exif_bridge_rs::tag::Tag::GPSDestBearing());
const Tag Tag::kGPSDestBearingRef =
    Tag(exif_bridge_rs::tag::Tag::GPSDestBearingRef());
const Tag Tag::kGPSDestDistance =
    Tag(exif_bridge_rs::tag::Tag::GPSDestDistance());
const Tag Tag::kGPSDestDistanceRef =
    Tag(exif_bridge_rs::tag::Tag::GPSDestDistanceRef());
const Tag Tag::kGPSDestLatitude =
    Tag(exif_bridge_rs::tag::Tag::GPSDestLatitude());
const Tag Tag::kGPSDestLatitudeRef =
    Tag(exif_bridge_rs::tag::Tag::GPSDestLatitudeRef());
const Tag Tag::kGPSDestLongitude =
    Tag(exif_bridge_rs::tag::Tag::GPSDestLongitude());
const Tag Tag::kGPSDestLongitudeRef =
    Tag(exif_bridge_rs::tag::Tag::GPSDestLongitudeRef());
const Tag Tag::kGPSDifferential =
    Tag(exif_bridge_rs::tag::Tag::GPSDifferential());
const Tag Tag::kGPSHPositioningError =
    Tag(exif_bridge_rs::tag::Tag::GPSHPositioningError());
const Tag Tag::kGPSImgDirection =
    Tag(exif_bridge_rs::tag::Tag::GPSImgDirection());
const Tag Tag::kGPSImgDirectionRef =
    Tag(exif_bridge_rs::tag::Tag::GPSImgDirectionRef());
const Tag Tag::kGPSLatitude = Tag(exif_bridge_rs::tag::Tag::GPSLatitude());
const Tag Tag::kGPSLatitudeRef =
    Tag(exif_bridge_rs::tag::Tag::GPSLatitudeRef());
const Tag Tag::kGPSLongitude = Tag(exif_bridge_rs::tag::Tag::GPSLongitude());
const Tag Tag::kGPSLongitudeRef =
    Tag(exif_bridge_rs::tag::Tag::GPSLongitudeRef());
const Tag Tag::kGPSMapDatum = Tag(exif_bridge_rs::tag::Tag::GPSMapDatum());
const Tag Tag::kGPSMeasureMode =
    Tag(exif_bridge_rs::tag::Tag::GPSMeasureMode());
const Tag Tag::kGPSProcessingMethod =
    Tag(exif_bridge_rs::tag::Tag::GPSProcessingMethod());
const Tag Tag::kGPSSatellites = Tag(exif_bridge_rs::tag::Tag::GPSSatellites());
const Tag Tag::kGPSSpeed = Tag(exif_bridge_rs::tag::Tag::GPSSpeed());
const Tag Tag::kGPSSpeedRef = Tag(exif_bridge_rs::tag::Tag::GPSSpeedRef());
const Tag Tag::kGPSStatus = Tag(exif_bridge_rs::tag::Tag::GPSStatus());
const Tag Tag::kGPSTimeStamp = Tag(exif_bridge_rs::tag::Tag::GPSTimeStamp());
const Tag Tag::kGPSTrack = Tag(exif_bridge_rs::tag::Tag::GPSTrack());
const Tag Tag::kGPSTrackRef = Tag(exif_bridge_rs::tag::Tag::GPSTrackRef());
const Tag Tag::kGPSVersionID = Tag(exif_bridge_rs::tag::Tag::GPSVersionID());
const Tag Tag::kGainControl = Tag(exif_bridge_rs::tag::Tag::GainControl());
const Tag Tag::kGamma = Tag(exif_bridge_rs::tag::Tag::Gamma());
const Tag Tag::kHumidity = Tag(exif_bridge_rs::tag::Tag::Humidity());
const Tag Tag::kISOSpeed = Tag(exif_bridge_rs::tag::Tag::ISOSpeed());
const Tag Tag::kISOSpeedLatitudeyyy =
    Tag(exif_bridge_rs::tag::Tag::ISOSpeedLatitudeyyy());
const Tag Tag::kISOSpeedLatitudezzz =
    Tag(exif_bridge_rs::tag::Tag::ISOSpeedLatitudezzz());
const Tag Tag::kImageDescription =
    Tag(exif_bridge_rs::tag::Tag::ImageDescription());
const Tag Tag::kImageLength = Tag(exif_bridge_rs::tag::Tag::ImageLength());
const Tag Tag::kImageUniqueID = Tag(exif_bridge_rs::tag::Tag::ImageUniqueID());
const Tag Tag::kImageWidth = Tag(exif_bridge_rs::tag::Tag::ImageWidth());
const Tag Tag::kInteroperabilityIndex =
    Tag(exif_bridge_rs::tag::Tag::InteroperabilityIndex());
const Tag Tag::kInteroperabilityVersion =
    Tag(exif_bridge_rs::tag::Tag::InteroperabilityVersion());
const Tag Tag::kJPEGInterchangeFormat =
    Tag(exif_bridge_rs::tag::Tag::JPEGInterchangeFormat());
const Tag Tag::kJPEGInterchangeFormatLength =
    Tag(exif_bridge_rs::tag::Tag::JPEGInterchangeFormatLength());
const Tag Tag::kLensMake = Tag(exif_bridge_rs::tag::Tag::LensMake());
const Tag Tag::kLensModel = Tag(exif_bridge_rs::tag::Tag::LensModel());
const Tag Tag::kLensSerialNumber =
    Tag(exif_bridge_rs::tag::Tag::LensSerialNumber());
const Tag Tag::kLensSpecification =
    Tag(exif_bridge_rs::tag::Tag::LensSpecification());
const Tag Tag::kLightSource = Tag(exif_bridge_rs::tag::Tag::LightSource());
const Tag Tag::kMake = Tag(exif_bridge_rs::tag::Tag::Make());
const Tag Tag::kMakerNote = Tag(exif_bridge_rs::tag::Tag::MakerNote());
const Tag Tag::kMaxApertureValue =
    Tag(exif_bridge_rs::tag::Tag::MaxApertureValue());
const Tag Tag::kMeteringMode = Tag(exif_bridge_rs::tag::Tag::MeteringMode());
const Tag Tag::kModel = Tag(exif_bridge_rs::tag::Tag::Model());
const Tag Tag::kOECF = Tag(exif_bridge_rs::tag::Tag::OECF());
const Tag Tag::kOffsetTime = Tag(exif_bridge_rs::tag::Tag::OffsetTime());
const Tag Tag::kOffsetTimeDigitized =
    Tag(exif_bridge_rs::tag::Tag::OffsetTimeDigitized());
const Tag Tag::kOffsetTimeOriginal =
    Tag(exif_bridge_rs::tag::Tag::OffsetTimeOriginal());
const Tag Tag::kOrientation = Tag(exif_bridge_rs::tag::Tag::Orientation());
const Tag Tag::kPhotographicSensitivity =
    Tag(exif_bridge_rs::tag::Tag::PhotographicSensitivity());
const Tag Tag::kPhotometricInterpretation =
    Tag(exif_bridge_rs::tag::Tag::PhotometricInterpretation());
const Tag Tag::kPixelXDimension =
    Tag(exif_bridge_rs::tag::Tag::PixelXDimension());
const Tag Tag::kPixelYDimension =
    Tag(exif_bridge_rs::tag::Tag::PixelYDimension());
const Tag Tag::kPlanarConfiguration =
    Tag(exif_bridge_rs::tag::Tag::PlanarConfiguration());
const Tag Tag::kPressure = Tag(exif_bridge_rs::tag::Tag::Pressure());
const Tag Tag::kPrimaryChromaticities =
    Tag(exif_bridge_rs::tag::Tag::PrimaryChromaticities());
const Tag Tag::kRecommendedExposureIndex =
    Tag(exif_bridge_rs::tag::Tag::RecommendedExposureIndex());
const Tag Tag::kReferenceBlackWhite =
    Tag(exif_bridge_rs::tag::Tag::ReferenceBlackWhite());
const Tag Tag::kRelatedImageFileFormat =
    Tag(exif_bridge_rs::tag::Tag::RelatedImageFileFormat());
const Tag Tag::kRelatedImageLength =
    Tag(exif_bridge_rs::tag::Tag::RelatedImageLength());
const Tag Tag::kRelatedImageWidth =
    Tag(exif_bridge_rs::tag::Tag::RelatedImageWidth());
const Tag Tag::kRelatedSoundFile =
    Tag(exif_bridge_rs::tag::Tag::RelatedSoundFile());
const Tag Tag::kResolutionUnit =
    Tag(exif_bridge_rs::tag::Tag::ResolutionUnit());
const Tag Tag::kRowsPerStrip = Tag(exif_bridge_rs::tag::Tag::RowsPerStrip());
const Tag Tag::kSamplesPerPixel =
    Tag(exif_bridge_rs::tag::Tag::SamplesPerPixel());
const Tag Tag::kSaturation = Tag(exif_bridge_rs::tag::Tag::Saturation());
const Tag Tag::kSceneCaptureType =
    Tag(exif_bridge_rs::tag::Tag::SceneCaptureType());
const Tag Tag::kSceneType = Tag(exif_bridge_rs::tag::Tag::SceneType());
const Tag Tag::kSensingMethod = Tag(exif_bridge_rs::tag::Tag::SensingMethod());
const Tag Tag::kSensitivityType =
    Tag(exif_bridge_rs::tag::Tag::SensitivityType());
const Tag Tag::kSharpness = Tag(exif_bridge_rs::tag::Tag::Sharpness());
const Tag Tag::kShutterSpeedValue =
    Tag(exif_bridge_rs::tag::Tag::ShutterSpeedValue());
const Tag Tag::kSoftware = Tag(exif_bridge_rs::tag::Tag::Software());
const Tag Tag::kSourceExposureTimesOfCompositeImage =
    Tag(exif_bridge_rs::tag::Tag::SourceExposureTimesOfCompositeImage());
const Tag Tag::kSourceImageNumberOfCompositeImage =
    Tag(exif_bridge_rs::tag::Tag::SourceImageNumberOfCompositeImage());
const Tag Tag::kSpatialFrequencyResponse =
    Tag(exif_bridge_rs::tag::Tag::SpatialFrequencyResponse());
const Tag Tag::kSpectralSensitivity =
    Tag(exif_bridge_rs::tag::Tag::SpectralSensitivity());
const Tag Tag::kStandardOutputSensitivity =
    Tag(exif_bridge_rs::tag::Tag::StandardOutputSensitivity());
const Tag Tag::kStripByteCounts =
    Tag(exif_bridge_rs::tag::Tag::StripByteCounts());
const Tag Tag::kStripOffsets = Tag(exif_bridge_rs::tag::Tag::StripOffsets());
const Tag Tag::kSubSecTime = Tag(exif_bridge_rs::tag::Tag::SubSecTime());
const Tag Tag::kSubSecTimeDigitized =
    Tag(exif_bridge_rs::tag::Tag::SubSecTimeDigitized());
const Tag Tag::kSubSecTimeOriginal =
    Tag(exif_bridge_rs::tag::Tag::SubSecTimeOriginal());
const Tag Tag::kSubjectArea = Tag(exif_bridge_rs::tag::Tag::SubjectArea());
const Tag Tag::kSubjectDistance =
    Tag(exif_bridge_rs::tag::Tag::SubjectDistance());
const Tag Tag::kSubjectDistanceRange =
    Tag(exif_bridge_rs::tag::Tag::SubjectDistanceRange());
const Tag Tag::kSubjectLocation =
    Tag(exif_bridge_rs::tag::Tag::SubjectLocation());
const Tag Tag::kTemperature = Tag(exif_bridge_rs::tag::Tag::Temperature());
const Tag Tag::kTileByteCounts =
    Tag(exif_bridge_rs::tag::Tag::TileByteCounts());
const Tag Tag::kTileOffsets = Tag(exif_bridge_rs::tag::Tag::TileOffsets());
const Tag Tag::kTransferFunction =
    Tag(exif_bridge_rs::tag::Tag::TransferFunction());
const Tag Tag::kUserComment = Tag(exif_bridge_rs::tag::Tag::UserComment());
const Tag Tag::kWaterDepth = Tag(exif_bridge_rs::tag::Tag::WaterDepth());
const Tag Tag::kWhiteBalance = Tag(exif_bridge_rs::tag::Tag::WhiteBalance());
const Tag Tag::kWhitePoint = Tag(exif_bridge_rs::tag::Tag::WhitePoint());
const Tag Tag::kXResolution = Tag(exif_bridge_rs::tag::Tag::XResolution());
const Tag Tag::kYCbCrCoefficients =
    Tag(exif_bridge_rs::tag::Tag::YCbCrCoefficients());
const Tag Tag::kYCbCrPositioning =
    Tag(exif_bridge_rs::tag::Tag::YCbCrPositioning());
const Tag Tag::kYCbCrSubSampling =
    Tag(exif_bridge_rs::tag::Tag::YCbCrSubSampling());
const Tag Tag::kYResolution = Tag(exif_bridge_rs::tag::Tag::YResolution());

Context Tag::context() const { return tag_.context(); }

std::uint16_t Tag::number() const { return tag_.number(); }

std::optional<std::string> Tag::description() const {
  exif_bridge_rs::types::OptionString desc = tag_.description();
  if (desc.is_some()) {
    return std::optional<std::string>(std::move(desc).unwrap());
  }
  return std::nullopt;
}

std::optional<Value> Tag::default_value() const {
  std::optional<exif_bridge_rs::value::Value> val = tag_.default_value();
  if (val.has_value()) {
    return Value(std::move(*val));
  }
  return std::nullopt;
}

// Field
Field::Field(Tag tag, In ifd_num, Value value)
    : field_(exif_bridge_rs::tiff::Field::new_(tag.tag_, ifd_num.in_,
                                               value.value_)),
      tag_(field_.get_tag()),
      ifd_num_(field_.get_ifd()),
      value_(field_.get_value()) {}

Field::Field(exif_bridge_rs::tiff::Field field)
    : field_(std::move(field)),
      tag_(field_.get_tag()),
      ifd_num_(field_.get_ifd()),
      value_(field_.get_value()) {}

absl::StatusOr<TiffExifData> parse_exif(absl::Span<const uint8_t> data) {
  exif_bridge_rs::tiff::ResultVecFieldBool result =
      exif_bridge_rs::tiff::parse_exif(data);
  if (!result.is_ok()) {
    return FromRustError(std::move(result).unwrap_err());
  }
  auto [fields_vec, is_little_endian] = std::move(result).unwrap();
  std::vector<Field> fields;
  fields.reserve(fields_vec.len());
  std::transform(
      fields_vec.as_mut_ptr(), fields_vec.as_mut_ptr() + fields_vec.len(),
      std::back_inserter(fields),
      [](exif_bridge_rs::tiff::Field& f) { return Field(std::move(f)); });
  return TiffExifData(std::move(fields), is_little_endian);
}

// Exif
Exif::Exif(exif_bridge_rs::reader::Exif exif) : exif_(std::move(exif)) {
  // Initialize cached buffer.
  exif_bridge_rs::types::VecU8 buf_view = exif_.buf();
  buf_ = std::vector<uint8_t>(buf_view.as_mut_ptr(),
                              buf_view.as_mut_ptr() + buf_view.len());
  // Initialize cached fields.
  exif_bridge_rs::tiff::VecField fields_vec = exif_.fields();
  std::vector<Field> fields;
  fields.reserve(fields_vec.len());
  std::transform(
      fields_vec.as_mut_ptr(), fields_vec.as_mut_ptr() + fields_vec.len(),
      std::back_inserter(fields),
      [](exif_bridge_rs::tiff::Field& f) { return Field(std::move(f)); });
  fields_ = std::move(fields);
}

absl::Span<const uint8_t> Exif::buf() const {
  return absl::Span<const uint8_t>(buf_);
}

absl::Span<const Field> Exif::fields() const {
  return absl::Span<const Field>(fields_);
}

bool Exif::little_endian() const { return exif_.little_endian(); }

std::optional<Field> Exif::get_field(Tag tag, In in) const {
  exif_bridge_rs::tiff::OptionField result = exif_.get_field(tag.tag_, in.in_);
  if (result.is_some()) {
    return Field(std::move(result).unwrap());
  }
  return std::nullopt;
}

// Reader
Reader::Reader() : reader_(exif_bridge_rs::reader::Reader::new_()) {}

absl::StatusOr<Exif> Reader::read_raw(absl::Span<const uint8_t> data) {
  exif_bridge_rs::reader::ResultExif result = reader_.read_raw(
      exif_bridge_rs::types::VecU8::copy_from_raw(data.data(), data.size()));
  if (result.is_ok()) {
    return Exif(std::move(result).unwrap());
  }
  return FromRustError(std::move(result).unwrap_err());
}

absl::StatusOr<Exif> Reader::read_from_container(
    absl::Span<const uint8_t> data) {
  exif_bridge_rs::reader::ResultExif result = reader_.read_from_container(
      exif_bridge_rs::types::VecU8::copy_from_raw(data.data(), data.size()));
  if (result.is_ok()) {
    return Exif(std::move(result).unwrap());
  }
  return FromRustError(std::move(result).unwrap_err());
}

absl::StatusOr<Exif> Reader::read_from_container(File& file) {
  ASSIGN_OR_RETURN(std::vector<uint8_t> data, ReadFileContents(file));
  exif_bridge_rs::reader::ResultExif result = reader_.read_from_container(
      exif_bridge_rs::types::VecU8::copy_from_raw(data.data(), data.size()));
  if (result.is_ok()) {
    return Exif(std::move(result).unwrap());
  }
  return FromRustError(std::move(result).unwrap_err());
}

// Writer
Writer::Writer() : writer_(exif_bridge_rs::writer::Writer::new_()) {}

void Writer::push_field(Field field) {
  // We can further optimize this, since upstream's push_field ignores
  // some fields. It's therefore not necessary to store references to these
  // fields with fields_storage_. However, this means we need to sync this
  // with upstream logic whenever it changes. For now this is good enough.
  fields_storage_.push_back(std::move(field));
  writer_.push_field(&fields_storage_.back().field_);
}

void Writer::set_strips(std::vector<std::vector<uint8_t>> strips, In ifd_num) {
  uint16_t idx = ifd_num.index();
  strips_storage_[idx] = std::move(strips);
  strips_refs_storage_[idx].clear();

  for (const auto& v : strips_storage_[idx]) {
    strips_refs_storage_[idx].push_back(rs_std::SliceRef<const uint8_t>(
        absl::Span<const uint8_t>(v.data(), v.size())));
  }

  writer_.set_strips(strips_refs_storage_[idx], ifd_num.in_);
}

void Writer::set_tiles(std::vector<std::vector<uint8_t>> tiles, In ifd_num) {
  uint16_t idx = ifd_num.index();
  tiles_storage_[idx] = std::move(tiles);
  tiles_refs_storage_[idx].clear();

  for (const auto& v : tiles_storage_[idx]) {
    tiles_refs_storage_[idx].push_back(rs_std::SliceRef<const uint8_t>(
        absl::Span<const uint8_t>(v.data(), v.size())));
  }

  writer_.set_tiles(tiles_refs_storage_[idx], ifd_num.in_);
}

void Writer::set_jpeg(absl::Span<const uint8_t> jpeg, In ifd_num) {
  uint16_t idx = ifd_num.index();
  jpeg_storage_[idx] = std::vector<uint8_t>(jpeg.begin(), jpeg.end());
  writer_.set_jpeg(jpeg_storage_[idx], ifd_num.in_);
}

absl::StatusOr<ExifBytes> Writer::write(bool little_endian) {
  exif_bridge_rs::writer::ResultVecU8 result = writer_.write(little_endian);
  if (!result.is_ok()) {
    return FromRustError(std::move(result).unwrap_err());
  }
  return ExifBytes(std::move(result).unwrap());
}

}  // namespace security::exif_bridge
