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
#include "crubit/rust.h"
#include "crubit_helpers/string_conversions.h"
#include "absl/algorithm/container.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"

namespace security::exif_bridge {
namespace {

using ::security::crubit_helpers::StringViewFromVecU8;

// Mapping Rust error to C++ absl::Status.
absl::Status FromRustError(rust::error::Error error) {
  switch (error.status()) {
    case rust::error::ErrorStatus::INVALID_FORMAT:
      return absl::InvalidArgumentError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::IO:
      return absl::InternalError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::NOT_FOUND:
      return absl::NotFoundError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::BLANK_VALUE:
      return absl::NotFoundError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::TOO_BIG:
      return absl::OutOfRangeError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::NOT_SUPPORTED:
      return absl::UnimplementedError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::UNEXPECTED_VALUE:
      return absl::InvalidArgumentError(StringViewFromVecU8(error.message()));
    case rust::error::ErrorStatus::PARTIAL_RESULT:
      return absl::DataLossError(StringViewFromVecU8(error.message()));
    default:
      return absl::UnknownError(StringViewFromVecU8(error.message()));
  }
}
}  // namespace

// Value
Value::Value(rust::value::Value value) : value_(std::move(value)) {}

bool Value::operator==(const Value& other) const {
  return value_.equals(other.value_);
}

Value Value::Byte(absl::Span<const uint8_t> value) {
  return Value(rust::value::Value::Byte(
      rust::types::VecU8::copy_from_slice(value)));
}

Value Value::Ascii(absl::Span<const std::string> value) {
  std::vector<rust::types::VecU8> rust_vec;
  rust_vec.reserve(value.size());
  absl::c_transform(
      value, std::back_inserter(rust_vec), [](const std::string& s) {
        return rust::types::VecU8::copy_from_slice(
            absl::MakeConstSpan(reinterpret_cast<const uint8_t*>(s.data()),
                                s.size()));
      });
  return Value(rust::value::Value::Ascii(
      rust::types::VecVecU8::copy_from_slice(rust_vec)));
}

Value Value::Short(absl::Span<const uint16_t> value) {
  return Value(rust::value::Value::Short(
      rust::types::VecU16::copy_from_slice(value)));
}

Value Value::Long(absl::Span<const uint32_t> value) {
  return Value(rust::value::Value::Long(
      rust::types::VecU32::copy_from_slice(value)));
}

Value Value::Rational(absl::Span<const rust::value::Rational> value) {
  return Value(rust::value::Value::Rational(
      rust::value::VecRational::copy_from_slice(value)));
}

Value Value::SByte(absl::Span<const int8_t> value) {
  return Value(rust::value::Value::SByte(
      rust::types::VecI8::copy_from_slice(value)));
}

Value Value::Undefined(absl::Span<const uint8_t> value, uint32_t offset) {
  return Value(rust::value::Value::Undefined(
      rust::types::VecU8::copy_from_slice(value), offset));
}

Value Value::SShort(absl::Span<const int16_t> value) {
  return Value(rust::value::Value::SShort(
      rust::types::VecI16::copy_from_slice(value)));
}

Value Value::SLong(absl::Span<const int32_t> value) {
  return Value(rust::value::Value::SLong(
      rust::types::VecI32::copy_from_slice(value)));
}

Value Value::SRational(
    absl::Span<const rust::value::SRational> value) {
  return Value(rust::value::Value::SRational(
      rust::value::VecSRational::copy_from_slice(value)));
}

Value Value::Float(absl::Span<const float> value) {
  return Value(rust::value::Value::Float(
      rust::types::VecF32::copy_from_slice(value)));
}

Value Value::Double(absl::Span<const double> value) {
  return Value(rust::value::Value::Double(
      rust::types::VecF64::copy_from_slice(value)));
}

Value Value::Unknown(uint16_t type, uint32_t count, uint32_t offset) {
  return Value(rust::value::Value::Unknown(type, count, offset));
}

rust::reexport::Display Value::display_as(Tag tag) const {
  return value_.display_as(tag.tag_.into_inner());
}

absl::StatusOr<rust::reexport::UIntValue> Value::as_uint() const {
  rs_std::Result<rust::reexport::UIntValue,
                 rust::error::Error>
      result = value_.as_uint();
  if (result.has_value()) {
    return std::move(result).value();
  }
  return FromRustError(std::move(result).err());
}

std::optional<std::uint32_t> Value::get_uint(std::uintptr_t index) const {
  return value_.get_uint(index);
}

std::optional<int64_t> Value::get_int(size_t index) const {
  return value_.get_int(index);
}

std::optional<float> Value::get_float(size_t index) const {
  return value_.get_float(index);
}

std::optional<double> Value::get_double(size_t index) const {
  return value_.get_double(index);
}

std::optional<std::vector<uint8_t>> Value::get_bytes() const {
  std::optional<rust::types::VecU8> result = value_.get_bytes();
  if (result.has_value()) {
    const size_t len = result.value().len();
    std::vector<uint8_t> bytes;
    bytes.reserve(len);
    bytes.insert(bytes.end(), result.value().as_ptr(),
                 result.value().as_ptr() + len);
    return bytes;
  }
  return std::nullopt;
}

std::optional<std::vector<uint32_t>> Value::iter_uint() const {
  std::optional<rust::types::VecU32> result = value_.iter_uint();
  if (result.has_value()) {
    const size_t len = result.value().len();
    std::vector<uint32_t> vec;
    vec.reserve(len);
    vec.insert(vec.end(), result.value().as_ptr(),
               result.value().as_ptr() + len);
    return vec;
  }
  return std::nullopt;
}

// Tag
Tag::Tag(rust::tag::Tag tag) : tag_(std::move(tag)) {}

bool Tag::operator==(const Tag& other) const { return tag_.equals(other.tag_); }

bool Tag::operator<(const Tag& other) const {
  return tag_.less_than(other.tag_);
}

const Tag Tag::kAcceleration = Tag(rust::tag::Tag::Acceleration());
const Tag Tag::kApertureValue = Tag(rust::tag::Tag::ApertureValue());
const Tag Tag::kArtist = Tag(rust::tag::Tag::Artist());
const Tag Tag::kBitsPerSample = Tag(rust::tag::Tag::BitsPerSample());
const Tag Tag::kBodySerialNumber =
    Tag(rust::tag::Tag::BodySerialNumber());
const Tag Tag::kBrightnessValue =
    Tag(rust::tag::Tag::BrightnessValue());
const Tag Tag::kCFAPattern = Tag(rust::tag::Tag::CFAPattern());
const Tag Tag::kCameraElevationAngle =
    Tag(rust::tag::Tag::CameraElevationAngle());
const Tag Tag::kCameraOwnerName =
    Tag(rust::tag::Tag::CameraOwnerName());
const Tag Tag::kColorSpace = Tag(rust::tag::Tag::ColorSpace());
const Tag Tag::kComponentsConfiguration =
    Tag(rust::tag::Tag::ComponentsConfiguration());
const Tag Tag::kCompositeImage =
    Tag(rust::tag::Tag::CompositeImage());
const Tag Tag::kCompressedBitsPerPixel =
    Tag(rust::tag::Tag::CompressedBitsPerPixel());
const Tag Tag::kCompression = Tag(rust::tag::Tag::Compression());
const Tag Tag::kContrast = Tag(rust::tag::Tag::Contrast());
const Tag Tag::kCopyright = Tag(rust::tag::Tag::Copyright());
const Tag Tag::kCustomRendered =
    Tag(rust::tag::Tag::CustomRendered());
const Tag Tag::kDateTime = Tag(rust::tag::Tag::DateTime());
const Tag Tag::kDateTimeDigitized =
    Tag(rust::tag::Tag::DateTimeDigitized());
const Tag Tag::kDateTimeOriginal =
    Tag(rust::tag::Tag::DateTimeOriginal());
const Tag Tag::kDeviceSettingDescription =
    Tag(rust::tag::Tag::DeviceSettingDescription());
const Tag Tag::kDigitalZoomRatio =
    Tag(rust::tag::Tag::DigitalZoomRatio());
const Tag Tag::kExifVersion = Tag(rust::tag::Tag::ExifVersion());
const Tag Tag::kExposureBiasValue =
    Tag(rust::tag::Tag::ExposureBiasValue());
const Tag Tag::kExposureIndex = Tag(rust::tag::Tag::ExposureIndex());
const Tag Tag::kExposureMode = Tag(rust::tag::Tag::ExposureMode());
const Tag Tag::kExposureProgram =
    Tag(rust::tag::Tag::ExposureProgram());
const Tag Tag::kExposureTime = Tag(rust::tag::Tag::ExposureTime());
const Tag Tag::kFNumber = Tag(rust::tag::Tag::FNumber());
const Tag Tag::kFileSource = Tag(rust::tag::Tag::FileSource());
const Tag Tag::kFlash = Tag(rust::tag::Tag::Flash());
const Tag Tag::kFlashEnergy = Tag(rust::tag::Tag::FlashEnergy());
const Tag Tag::kFlashpixVersion =
    Tag(rust::tag::Tag::FlashpixVersion());
const Tag Tag::kFocalLength = Tag(rust::tag::Tag::FocalLength());
const Tag Tag::kFocalLengthIn35mmFilm =
    Tag(rust::tag::Tag::FocalLengthIn35mmFilm());
const Tag Tag::kFocalPlaneResolutionUnit =
    Tag(rust::tag::Tag::FocalPlaneResolutionUnit());
const Tag Tag::kFocalPlaneXResolution =
    Tag(rust::tag::Tag::FocalPlaneXResolution());
const Tag Tag::kFocalPlaneYResolution =
    Tag(rust::tag::Tag::FocalPlaneYResolution());
const Tag Tag::kGPSAltitude = Tag(rust::tag::Tag::GPSAltitude());
const Tag Tag::kGPSAltitudeRef =
    Tag(rust::tag::Tag::GPSAltitudeRef());
const Tag Tag::kGPSAreaInformation =
    Tag(rust::tag::Tag::GPSAreaInformation());
const Tag Tag::kGPSDOP = Tag(rust::tag::Tag::GPSDOP());
const Tag Tag::kGPSDateStamp = Tag(rust::tag::Tag::GPSDateStamp());
const Tag Tag::kGPSDestBearing =
    Tag(rust::tag::Tag::GPSDestBearing());
const Tag Tag::kGPSDestBearingRef =
    Tag(rust::tag::Tag::GPSDestBearingRef());
const Tag Tag::kGPSDestDistance =
    Tag(rust::tag::Tag::GPSDestDistance());
const Tag Tag::kGPSDestDistanceRef =
    Tag(rust::tag::Tag::GPSDestDistanceRef());
const Tag Tag::kGPSDestLatitude =
    Tag(rust::tag::Tag::GPSDestLatitude());
const Tag Tag::kGPSDestLatitudeRef =
    Tag(rust::tag::Tag::GPSDestLatitudeRef());
const Tag Tag::kGPSDestLongitude =
    Tag(rust::tag::Tag::GPSDestLongitude());
const Tag Tag::kGPSDestLongitudeRef =
    Tag(rust::tag::Tag::GPSDestLongitudeRef());
const Tag Tag::kGPSDifferential =
    Tag(rust::tag::Tag::GPSDifferential());
const Tag Tag::kGPSHPositioningError =
    Tag(rust::tag::Tag::GPSHPositioningError());
const Tag Tag::kGPSImgDirection =
    Tag(rust::tag::Tag::GPSImgDirection());
const Tag Tag::kGPSImgDirectionRef =
    Tag(rust::tag::Tag::GPSImgDirectionRef());
const Tag Tag::kGPSLatitude = Tag(rust::tag::Tag::GPSLatitude());
const Tag Tag::kGPSLatitudeRef =
    Tag(rust::tag::Tag::GPSLatitudeRef());
const Tag Tag::kGPSLongitude = Tag(rust::tag::Tag::GPSLongitude());
const Tag Tag::kGPSLongitudeRef =
    Tag(rust::tag::Tag::GPSLongitudeRef());
const Tag Tag::kGPSMapDatum = Tag(rust::tag::Tag::GPSMapDatum());
const Tag Tag::kGPSMeasureMode =
    Tag(rust::tag::Tag::GPSMeasureMode());
const Tag Tag::kGPSProcessingMethod =
    Tag(rust::tag::Tag::GPSProcessingMethod());
const Tag Tag::kGPSSatellites = Tag(rust::tag::Tag::GPSSatellites());
const Tag Tag::kGPSSpeed = Tag(rust::tag::Tag::GPSSpeed());
const Tag Tag::kGPSSpeedRef = Tag(rust::tag::Tag::GPSSpeedRef());
const Tag Tag::kGPSStatus = Tag(rust::tag::Tag::GPSStatus());
const Tag Tag::kGPSTimeStamp = Tag(rust::tag::Tag::GPSTimeStamp());
const Tag Tag::kGPSTrack = Tag(rust::tag::Tag::GPSTrack());
const Tag Tag::kGPSTrackRef = Tag(rust::tag::Tag::GPSTrackRef());
const Tag Tag::kGPSVersionID = Tag(rust::tag::Tag::GPSVersionID());
const Tag Tag::kGainControl = Tag(rust::tag::Tag::GainControl());
const Tag Tag::kGamma = Tag(rust::tag::Tag::Gamma());
const Tag Tag::kHumidity = Tag(rust::tag::Tag::Humidity());
const Tag Tag::kISOSpeed = Tag(rust::tag::Tag::ISOSpeed());
const Tag Tag::kISOSpeedLatitudeyyy =
    Tag(rust::tag::Tag::ISOSpeedLatitudeyyy());
const Tag Tag::kISOSpeedLatitudezzz =
    Tag(rust::tag::Tag::ISOSpeedLatitudezzz());
const Tag Tag::kImageDescription =
    Tag(rust::tag::Tag::ImageDescription());
const Tag Tag::kImageLength = Tag(rust::tag::Tag::ImageLength());
const Tag Tag::kImageUniqueID = Tag(rust::tag::Tag::ImageUniqueID());
const Tag Tag::kImageWidth = Tag(rust::tag::Tag::ImageWidth());
const Tag Tag::kInteroperabilityIndex =
    Tag(rust::tag::Tag::InteroperabilityIndex());
const Tag Tag::kInteroperabilityVersion =
    Tag(rust::tag::Tag::InteroperabilityVersion());
const Tag Tag::kJPEGInterchangeFormat =
    Tag(rust::tag::Tag::JPEGInterchangeFormat());
const Tag Tag::kJPEGInterchangeFormatLength =
    Tag(rust::tag::Tag::JPEGInterchangeFormatLength());
const Tag Tag::kLensMake = Tag(rust::tag::Tag::LensMake());
const Tag Tag::kLensModel = Tag(rust::tag::Tag::LensModel());
const Tag Tag::kLensSerialNumber =
    Tag(rust::tag::Tag::LensSerialNumber());
const Tag Tag::kLensSpecification =
    Tag(rust::tag::Tag::LensSpecification());
const Tag Tag::kLightSource = Tag(rust::tag::Tag::LightSource());
const Tag Tag::kMake = Tag(rust::tag::Tag::Make());
const Tag Tag::kMakerNote = Tag(rust::tag::Tag::MakerNote());
const Tag Tag::kMaxApertureValue =
    Tag(rust::tag::Tag::MaxApertureValue());
const Tag Tag::kMeteringMode = Tag(rust::tag::Tag::MeteringMode());
const Tag Tag::kModel = Tag(rust::tag::Tag::Model());
const Tag Tag::kOECF = Tag(rust::tag::Tag::OECF());
const Tag Tag::kOffsetTime = Tag(rust::tag::Tag::OffsetTime());
const Tag Tag::kOffsetTimeDigitized =
    Tag(rust::tag::Tag::OffsetTimeDigitized());
const Tag Tag::kOffsetTimeOriginal =
    Tag(rust::tag::Tag::OffsetTimeOriginal());
const Tag Tag::kOrientation = Tag(rust::tag::Tag::Orientation());
const Tag Tag::kPhotographicSensitivity =
    Tag(rust::tag::Tag::PhotographicSensitivity());
const Tag Tag::kPhotometricInterpretation =
    Tag(rust::tag::Tag::PhotometricInterpretation());
const Tag Tag::kPixelXDimension =
    Tag(rust::tag::Tag::PixelXDimension());
const Tag Tag::kPixelYDimension =
    Tag(rust::tag::Tag::PixelYDimension());
const Tag Tag::kPlanarConfiguration =
    Tag(rust::tag::Tag::PlanarConfiguration());
const Tag Tag::kPressure = Tag(rust::tag::Tag::Pressure());
const Tag Tag::kPrimaryChromaticities =
    Tag(rust::tag::Tag::PrimaryChromaticities());
const Tag Tag::kRecommendedExposureIndex =
    Tag(rust::tag::Tag::RecommendedExposureIndex());
const Tag Tag::kReferenceBlackWhite =
    Tag(rust::tag::Tag::ReferenceBlackWhite());
const Tag Tag::kRelatedImageFileFormat =
    Tag(rust::tag::Tag::RelatedImageFileFormat());
const Tag Tag::kRelatedImageLength =
    Tag(rust::tag::Tag::RelatedImageLength());
const Tag Tag::kRelatedImageWidth =
    Tag(rust::tag::Tag::RelatedImageWidth());
const Tag Tag::kRelatedSoundFile =
    Tag(rust::tag::Tag::RelatedSoundFile());
const Tag Tag::kResolutionUnit =
    Tag(rust::tag::Tag::ResolutionUnit());
const Tag Tag::kRowsPerStrip = Tag(rust::tag::Tag::RowsPerStrip());
const Tag Tag::kSamplesPerPixel =
    Tag(rust::tag::Tag::SamplesPerPixel());
const Tag Tag::kSaturation = Tag(rust::tag::Tag::Saturation());
const Tag Tag::kSceneCaptureType =
    Tag(rust::tag::Tag::SceneCaptureType());
const Tag Tag::kSceneType = Tag(rust::tag::Tag::SceneType());
const Tag Tag::kSensingMethod = Tag(rust::tag::Tag::SensingMethod());
const Tag Tag::kSensitivityType =
    Tag(rust::tag::Tag::SensitivityType());
const Tag Tag::kSharpness = Tag(rust::tag::Tag::Sharpness());
const Tag Tag::kShutterSpeedValue =
    Tag(rust::tag::Tag::ShutterSpeedValue());
const Tag Tag::kSoftware = Tag(rust::tag::Tag::Software());
const Tag Tag::kSourceExposureTimesOfCompositeImage =
    Tag(rust::tag::Tag::SourceExposureTimesOfCompositeImage());
const Tag Tag::kSourceImageNumberOfCompositeImage =
    Tag(rust::tag::Tag::SourceImageNumberOfCompositeImage());
const Tag Tag::kSpatialFrequencyResponse =
    Tag(rust::tag::Tag::SpatialFrequencyResponse());
const Tag Tag::kSpectralSensitivity =
    Tag(rust::tag::Tag::SpectralSensitivity());
const Tag Tag::kStandardOutputSensitivity =
    Tag(rust::tag::Tag::StandardOutputSensitivity());
const Tag Tag::kStripByteCounts =
    Tag(rust::tag::Tag::StripByteCounts());
const Tag Tag::kStripOffsets = Tag(rust::tag::Tag::StripOffsets());
const Tag Tag::kSubSecTime = Tag(rust::tag::Tag::SubSecTime());
const Tag Tag::kSubSecTimeDigitized =
    Tag(rust::tag::Tag::SubSecTimeDigitized());
const Tag Tag::kSubSecTimeOriginal =
    Tag(rust::tag::Tag::SubSecTimeOriginal());
const Tag Tag::kSubjectArea = Tag(rust::tag::Tag::SubjectArea());
const Tag Tag::kSubjectDistance =
    Tag(rust::tag::Tag::SubjectDistance());
const Tag Tag::kSubjectDistanceRange =
    Tag(rust::tag::Tag::SubjectDistanceRange());
const Tag Tag::kSubjectLocation =
    Tag(rust::tag::Tag::SubjectLocation());
const Tag Tag::kTemperature = Tag(rust::tag::Tag::Temperature());
const Tag Tag::kTileByteCounts =
    Tag(rust::tag::Tag::TileByteCounts());
const Tag Tag::kTileOffsets = Tag(rust::tag::Tag::TileOffsets());
const Tag Tag::kTransferFunction =
    Tag(rust::tag::Tag::TransferFunction());
const Tag Tag::kUserComment = Tag(rust::tag::Tag::UserComment());
const Tag Tag::kWaterDepth = Tag(rust::tag::Tag::WaterDepth());
const Tag Tag::kWhiteBalance = Tag(rust::tag::Tag::WhiteBalance());
const Tag Tag::kWhitePoint = Tag(rust::tag::Tag::WhitePoint());
const Tag Tag::kXResolution = Tag(rust::tag::Tag::XResolution());
const Tag Tag::kYCbCrCoefficients =
    Tag(rust::tag::Tag::YCbCrCoefficients());
const Tag Tag::kYCbCrPositioning =
    Tag(rust::tag::Tag::YCbCrPositioning());
const Tag Tag::kYCbCrSubSampling =
    Tag(rust::tag::Tag::YCbCrSubSampling());
const Tag Tag::kYResolution = Tag(rust::tag::Tag::YResolution());

Context Tag::context() const { return tag_.context(); }

std::uint16_t Tag::number() const { return tag_.number(); }

std::optional<std::string> Tag::description() const {
  std::optional<rust::types::VecU8> desc = tag_.description();
  if (desc.has_value()) {
    return std::string(reinterpret_cast<const char*>(desc->as_ptr()),
                       desc->len());
  }
  return std::nullopt;
}

std::optional<Value> Tag::default_value() const {
  std::optional<rust::value::Value> val = tag_.default_value();
  if (val.has_value()) {
    return Value(std::move(*val));
  }
  return std::nullopt;
}

Tag Tag::from_u16(Context context, uint16_t number) {
  return Tag(rust::tag::Tag::from_u16(context, number));
}

// Field
Field::Field(Tag tag, In ifd_num, Value value)
    : field_(rust::tiff::Field::new_(tag.tag_, ifd_num.in_,
                                               value.value_)),
      tag_(field_.get_tag()),
      ifd_num_(field_.get_ifd()),
      value_(field_.get_value()) {}

Field::Field(rust::tiff::Field field)
    : field_(std::move(field)),
      tag_(field_.get_tag()),
      ifd_num_(field_.get_ifd()),
      value_(field_.get_value()) {}

absl::StatusOr<TiffExifData> parse_exif(absl::Span<const uint8_t> data) {
  rs_std::Result<rust::types::TiffExifDataRs,
                 rust::error::Error>
      result = rust::tiff::parse_exif(data);
  if (!result.has_value()) {
    return FromRustError(std::move(result).err());
  }
  rust::types::TiffExifDataRs val = std::move(result).value();
  auto fields_vec = std::move(val.fields);
  bool is_little_endian = val.is_little_endian;
  std::vector<Field> fields;
  fields.reserve(fields_vec.len());
  std::transform(
      fields_vec.as_mut_ptr(), fields_vec.as_mut_ptr() + fields_vec.len(),
      std::back_inserter(fields),
      [](rust::tiff::Field& f) { return Field(std::move(f)); });
  return TiffExifData(std::move(fields), is_little_endian);
}

// Exif
Exif::Exif(rust::reader::Exif exif) : exif_(std::move(exif)) {
  // Initialize cached buffer.
  rust::types::VecU8 buf_view = exif_.buf();
  buf_ = std::vector<uint8_t>(buf_view.as_mut_ptr(),
                              buf_view.as_mut_ptr() + buf_view.len());
  // Initialize cached fields.
  rust::types::VecField fields_vec = exif_.fields();
  std::vector<Field> fields;
  fields.reserve(fields_vec.len());
  std::transform(
      fields_vec.as_mut_ptr(), fields_vec.as_mut_ptr() + fields_vec.len(),
      std::back_inserter(fields),
      [](rust::tiff::Field& f) { return Field(std::move(f)); });
  fields_ = std::move(fields);

  // Initialize cached mnote fields.
  rust::types::VecField mnote_vec = exif_.mnote_fields();
  std::vector<Field> mnote_fields;
  mnote_fields.reserve(mnote_vec.len());
  std::transform(
      mnote_vec.as_mut_ptr(), mnote_vec.as_mut_ptr() + mnote_vec.len(),
      std::back_inserter(mnote_fields),
      [](rust::tiff::Field& f) { return Field(std::move(f)); });
  mnote_fields_ = std::move(mnote_fields);
}

absl::Span<const uint8_t> Exif::buf() const {
  return absl::Span<const uint8_t>(buf_);
}

absl::Span<const Field> Exif::fields() const {
  return absl::Span<const Field>(fields_);
}

absl::Span<const Field> Exif::mnote_fields() const {
  return absl::Span<const Field>(mnote_fields_);
}

bool Exif::little_endian() const { return exif_.little_endian(); }

std::optional<Field> Exif::get_field(Tag tag, In in) const {
  std::optional<rust::tiff::Field> result =
      exif_.get_field(tag.tag_, in.in_);
  if (result.has_value()) {
    return Field(std::move(result).value());
  }
  return std::nullopt;
}

uint32_t Exif::get_mnote_type() const { return exif_.get_mnote_type(); }

std::optional<Field> Exif::get_mnote_field(uint32_t tag_num) const {
  std::optional<rust::tiff::Field> result =
      exif_.get_mnote_field(tag_num);
  if (result.has_value()) {
    return Field(std::move(result).value());
  }
  return std::nullopt;
}

// Reader
Reader::Reader() : reader_(rust::reader::Reader::new_()) {}

absl::StatusOr<Exif> Reader::read_raw(absl::Span<const uint8_t> data) {
  rs_std::Result<rust::reader::Exif, rust::error::Error>
      result =
          reader_.read_raw(rust::types::VecU8::copy_from_slice(data));
  if (result.has_value()) {
    return Exif(std::move(result).value());
  }
  return FromRustError(std::move(result).err());
}

absl::StatusOr<Exif> Reader::read_from_container(
    absl::Span<const uint8_t> data) {
  rs_std::Result<rust::reader::Exif, rust::error::Error>
      result = reader_.read_from_container(
          rust::types::VecU8::copy_from_slice(data));
  if (result.has_value()) {
    return Exif(std::move(result).value());
  }
  return FromRustError(std::move(result).err());
}

// Writer
Writer::Writer() : writer_(rust::writer::Writer::new_()) {}

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
  rs_std::Result<rust::types::VecU8, rust::error::Error>
      result = writer_.write(little_endian);
  if (!result.has_value()) {
    return FromRustError(std::move(result).err());
  }
  return ExifBytes(std::move(result).value());
}

}  // namespace security::exif_bridge
