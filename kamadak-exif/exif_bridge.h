#ifndef SECURITY_EXIF_BRIDGE_EXIF_BRIDGE_H_
#define SECURITY_EXIF_BRIDGE_EXIF_BRIDGE_H_

#include <array>
#include <cstddef>
#include <cstdint>
#include <deque>
#include <optional>
#include <string>
#include <vector>

#include "crubit/support/rs_std/slice_ref.h"
#include "file/base/file.h"
#include "rust/exif_bridge_rs.h"
#include "absl/base/attributes.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"

namespace security::exif_bridge {

/**
 * An enum that indicates how a tag number is interpreted.
 *
 * This alias is provided for user convenience and is guaranteed to always be
 * the same as exif_bridge_rs::Context.
 */
using Context = exif_bridge_rs::Context;
/**
 * A signed rational number, which is a pair of 32-bit signed integers.
 *
 * This alias is provided for user convenience and is guaranteed to always be
 * the same as exif_bridge_rs::value::SRational.
 */
using SRational = exif_bridge_rs::value::SRational;
/**
 * An unsigned rational number, which is a pair of 32-bit unsigned integers.
 *
 * This alias is provided for user convenience and is guaranteed to always be
 * the same as exif_bridge_rs::value::Rational.
 */
using Rational = exif_bridge_rs::value::Rational;

class Tag;
class Field;

/**
 * A type and values of a TIFF/Exif field.
 */
class Value final {
 public:
  Value() = delete;
  Value(Value&&) = default;
  Value& operator=(Value&&) = default;
  Value(const Value&) = delete;
  Value& operator=(const Value&) = delete;

  ~Value() = default;

  bool operator==(const Value& other) const;
  bool operator!=(const Value& other) const { return !(*this == other); }

  /**
   * Vector of 8-bit unsigned integers.
   */
  static Value Byte(absl::Span<const uint8_t> value);
  /**
   * Vector of slices of 8-bit bytes containing 7-bit ASCII characters.
   * The trailing null characters are not included.  Note that
   * the 8th bits may present if a non-conforming data is given.
   */
  static Value Ascii(absl::Span<const std::string> value);
  /**
   * Vector of 16-bit unsigned integers.
   */
  static Value Short(absl::Span<const uint16_t> value);
  /**
   * Vector of 32-bit unsigned integers.
   */
  static Value Long(absl::Span<const uint32_t> value);
  /**
   * Vector of unsigned rationals.
   * An unsigned rational number is a pair of 32-bit unsigned integers.
   */
  static Value Rational(absl::Span<const Rational> value);
  /**
   * Vector of 8-bit signed integers.
   */
  static Value SByte(absl::Span<const int8_t> value);
  /** Slice of 8-bit bytes.
   *
   * The second member keeps the offset of the value in the Exif data.
   * The interpretation of the value does not generally depend on
   * the location, but if it does, the offset information helps.
   * When encoding Exif, it is ignored.
   */
  static Value Undefined(absl::Span<const uint8_t> value, uint32_t offset);
  /**
   * Vector of 16-bit signed integers.
   */
  static Value SShort(absl::Span<const int16_t> value);
  /**
   * Vector of 32-bit signed integers.
   */
  static Value SLong(absl::Span<const int32_t> value);
  /**
   * Vector of signed rationals.
   * A signed rational number is a pair of 32-bit signed integers.
   */
  static Value SRational(absl::Span<const SRational> value);
  /**
   * Vector of 32-bit (single precision) floating-point numbers.
   */
  static Value Float(absl::Span<const float> value);
  /**
   * Vector of 64-bit (double precision) floating-point numbers.
   */
  static Value Double(absl::Span<const double> value);
  /**
   * The type is unknown to this implementation.
   * The associated values are the type, the count, and the
   * offset of the "Value Offset" element.
   */
  static Value Unknown(uint16_t type, uint32_t count, uint32_t offset);

  /**
   * Returns an object that implements `Display` for
   * printing a value in a tag-specific format.
   * The tag of the value is specified as the argument.
   *
   * If you want to display with the unit, use `Field::display_value`.
   */
  exif_bridge_rs::reexport::Display display_as(Tag tag) const;

  /**
   * Returns `UIntValue` if the value type is unsigned integer (BYTE,
   * SHORT, or LONG).  Otherwise Error StatusCode is returned.
   *
   * The integer(s) can be obtained by the `get(std::size_t index)` method
   * on `UIntValue`, which returns `std::optional<uint32_t>`.
   * `std::nullopt` is returned if the index is out of bounds.
   */
  absl::StatusOr<exif_bridge_rs::reexport::UIntValue> as_uint() const;

  /**
   * Returns the unsigned integer at the given position.
   * None is returned if the value type is not unsigned integer
   * (BYTE, SHORT, or LONG) or the position is out of bounds.
   */
  std::optional<uint32_t> get_uint(size_t index) const;

  /**
   * Returns an u32 iterator over the unsigned integers (BYTE, SHORT, or LONG).
   * `None` is returned if the value is not an unsigned integer type.
   */
  std::optional<std::vector<uint32_t>> iter_uint() const;

 private:
  friend class Tag;
  friend class Field;

  explicit Value(exif_bridge_rs::value::Value value);
  exif_bridge_rs::value::Value value_;
};

/**
 * A tag of a TIFF/Exif field.
 *
 * Some well-known tags are provided as associated constants of
 * this type.  The constant names follow the Exif specification.
 */
class Tag final {
 public:
  static const Tag kAcceleration;
  static const Tag kApertureValue;
  static const Tag kArtist;
  static const Tag kBitsPerSample;
  static const Tag kBodySerialNumber;
  static const Tag kBrightnessValue;
  static const Tag kCFAPattern;
  static const Tag kCameraElevationAngle;
  static const Tag kCameraOwnerName;
  static const Tag kColorSpace;
  static const Tag kComponentsConfiguration;
  static const Tag kCompositeImage;
  static const Tag kCompressedBitsPerPixel;
  static const Tag kCompression;
  static const Tag kContrast;
  static const Tag kCopyright;
  static const Tag kCustomRendered;
  static const Tag kDateTime;
  static const Tag kDateTimeDigitized;
  static const Tag kDateTimeOriginal;
  static const Tag kDeviceSettingDescription;
  static const Tag kDigitalZoomRatio;
  static const Tag kExifVersion;
  static const Tag kExposureBiasValue;
  static const Tag kExposureIndex;
  static const Tag kExposureMode;
  static const Tag kExposureProgram;
  static const Tag kExposureTime;
  static const Tag kFNumber;
  static const Tag kFileSource;
  static const Tag kFlash;
  static const Tag kFlashEnergy;
  static const Tag kFlashpixVersion;
  static const Tag kFocalLength;
  static const Tag kFocalLengthIn35mmFilm;
  static const Tag kFocalPlaneResolutionUnit;
  static const Tag kFocalPlaneXResolution;
  static const Tag kFocalPlaneYResolution;
  static const Tag kGPSAltitude;
  static const Tag kGPSAltitudeRef;
  static const Tag kGPSAreaInformation;
  static const Tag kGPSDOP;
  static const Tag kGPSDateStamp;
  static const Tag kGPSDestBearing;
  static const Tag kGPSDestBearingRef;
  static const Tag kGPSDestDistance;
  static const Tag kGPSDestDistanceRef;
  static const Tag kGPSDestLatitude;
  static const Tag kGPSDestLatitudeRef;
  static const Tag kGPSDestLongitude;
  static const Tag kGPSDestLongitudeRef;
  static const Tag kGPSDifferential;
  static const Tag kGPSHPositioningError;
  static const Tag kGPSImgDirection;
  static const Tag kGPSImgDirectionRef;
  static const Tag kGPSLatitude;
  static const Tag kGPSLatitudeRef;
  static const Tag kGPSLongitude;
  static const Tag kGPSLongitudeRef;
  static const Tag kGPSMapDatum;
  static const Tag kGPSMeasureMode;
  static const Tag kGPSProcessingMethod;
  static const Tag kGPSSatellites;
  static const Tag kGPSSpeed;
  static const Tag kGPSSpeedRef;
  static const Tag kGPSStatus;
  static const Tag kGPSTimeStamp;
  static const Tag kGPSTrack;
  static const Tag kGPSTrackRef;
  static const Tag kGPSVersionID;
  static const Tag kGainControl;
  static const Tag kGamma;
  static const Tag kHumidity;
  static const Tag kISOSpeed;
  static const Tag kISOSpeedLatitudeyyy;
  static const Tag kISOSpeedLatitudezzz;
  static const Tag kImageDescription;
  static const Tag kImageLength;
  static const Tag kImageUniqueID;
  static const Tag kImageWidth;
  static const Tag kInteroperabilityIndex;
  static const Tag kInteroperabilityVersion;
  static const Tag kJPEGInterchangeFormat;
  static const Tag kJPEGInterchangeFormatLength;
  static const Tag kLensMake;
  static const Tag kLensModel;
  static const Tag kLensSerialNumber;
  static const Tag kLensSpecification;
  static const Tag kLightSource;
  static const Tag kMake;
  static const Tag kMakerNote;
  static const Tag kMaxApertureValue;
  static const Tag kMeteringMode;
  static const Tag kModel;
  static const Tag kOECF;
  static const Tag kOffsetTime;
  static const Tag kOffsetTimeDigitized;
  static const Tag kOffsetTimeOriginal;
  static const Tag kOrientation;
  static const Tag kPhotographicSensitivity;
  static const Tag kPhotometricInterpretation;
  static const Tag kPixelXDimension;
  static const Tag kPixelYDimension;
  static const Tag kPlanarConfiguration;
  static const Tag kPressure;
  static const Tag kPrimaryChromaticities;
  static const Tag kRecommendedExposureIndex;
  static const Tag kReferenceBlackWhite;
  static const Tag kRelatedImageFileFormat;
  static const Tag kRelatedImageLength;
  static const Tag kRelatedImageWidth;
  static const Tag kRelatedSoundFile;
  static const Tag kResolutionUnit;
  static const Tag kRowsPerStrip;
  static const Tag kSamplesPerPixel;
  static const Tag kSaturation;
  static const Tag kSceneCaptureType;
  static const Tag kSceneType;
  static const Tag kSensingMethod;
  static const Tag kSensitivityType;
  static const Tag kSharpness;
  static const Tag kShutterSpeedValue;
  static const Tag kSoftware;
  static const Tag kSourceExposureTimesOfCompositeImage;
  static const Tag kSourceImageNumberOfCompositeImage;
  static const Tag kSpatialFrequencyResponse;
  static const Tag kSpectralSensitivity;
  static const Tag kStandardOutputSensitivity;
  static const Tag kStripByteCounts;
  static const Tag kStripOffsets;
  static const Tag kSubSecTime;
  static const Tag kSubSecTimeDigitized;
  static const Tag kSubSecTimeOriginal;
  static const Tag kSubjectArea;
  static const Tag kSubjectDistance;
  static const Tag kSubjectDistanceRange;
  static const Tag kSubjectLocation;
  static const Tag kTemperature;
  static const Tag kTileByteCounts;
  static const Tag kTileOffsets;
  static const Tag kTransferFunction;
  static const Tag kUserComment;
  static const Tag kWaterDepth;
  static const Tag kWhiteBalance;
  static const Tag kWhitePoint;
  static const Tag kXResolution;
  static const Tag kYCbCrCoefficients;
  static const Tag kYCbCrPositioning;
  static const Tag kYCbCrSubSampling;
  static const Tag kYResolution;

  Tag() = delete;
  Tag(Tag&&) = default;
  Tag& operator=(Tag&&) = default;
  Tag(const Tag&) = default;
  Tag& operator=(const Tag&) = default;

  ~Tag() = default;

  bool operator==(const Tag& other) const;
  bool operator!=(const Tag& other) const { return !(*this == other); }
  bool operator<(const Tag& other) const;
  bool operator>(const Tag& other) const { return other < *this; }
  bool operator<=(const Tag& other) const { return !(other < *this); }
  bool operator>=(const Tag& other) const { return !(*this < other); }

  /**
   * Returns the context of the tag.
   */
  Context context() const;

  /**
   * Returns the tag number.
   */
  uint16_t number() const;

  /**
   * Returns the description of the tag.
   */
  std::optional<std::string> description() const;

  /**
   * Returns the default value of the tag. `None` is returned if
   * it is not defined in the standard or it depends on another tag.
   */
  std::optional<Value> default_value() const;

 private:
  friend class Value;
  friend class Field;
  friend class Exif;

  explicit Tag(exif_bridge_rs::tag::Tag tag);
  exif_bridge_rs::tag::Tag tag_;
};

/**
 * An IFD number.
 *
 * The IFDs are indexed from 0.  The 0th IFD is for the primary image
 * and the 1st one is for the thumbnail.  Two associated constants,
 * `In::kPrimary` and `In::kThumbnail`, are defined for them respectively.
 */
class In final {
 public:
  static const In kPrimary;
  static const In kThumbnail;

  In() = delete;
  In(const In&) = default;
  In& operator=(const In&) = default;
  In(In&&) = default;
  In& operator=(In&&) = default;

  ~In() = default;

  /**
   * Returns the IFD number.
   */
  constexpr uint16_t index() const { return static_cast<uint16_t>(in_); }

 private:
  friend class Field;
  friend class Exif;
  friend class Writer;

  exif_bridge_rs::tiff::In in_;
  constexpr explicit In(exif_bridge_rs::tiff::In in) : in_(in) {}
};

inline constexpr In In::kPrimary = In(exif_bridge_rs::tiff::In::PRIMARY);
inline constexpr In In::kThumbnail = In(exif_bridge_rs::tiff::In::THUMBNAIL);

struct TiffExifData {
  std::vector<Field> fields;
  bool is_little_endian;
};

/**
 * Parse the Exif attributes in the TIFF format.
 *
 * Returns StatusOr of TiffExifData, which contains
 * a Vec of Exif fields and a bool.
 * The boolean value is true if the data is little endian.
 */
absl::StatusOr<TiffExifData> parse_exif(absl::Span<const uint8_t> data);

/**
 * A TIFF/Exif field.
 */
class Field {
 public:
  Field() = delete;
  Field(Tag tag, In ifd_num, Value value);
  Field(Field&&) = default;
  Field& operator=(Field&&) = default;
  Field(const Field&) = delete;
  Field& operator=(const Field&) = delete;

  ~Field() = default;

  /**
   * The tag of this field.
   */
  Tag tag() const { return tag_; };
  /**
   * The index of the IFD to which this field belongs.
   */
  In ifd_num() const { return ifd_num_; };
  /**
   * The value of this field.
   */
  const Value& value() const { return value_; };

 private:
  friend class Exif;
  friend class Writer;
  explicit Field(exif_bridge_rs::tiff::Field field);
  friend absl::StatusOr<TiffExifData> parse_exif(
      absl::Span<const uint8_t> data);

  exif_bridge_rs::tiff::Field field_;
  Tag tag_;
  In ifd_num_;
  Value value_;
};

/**
 * A class that holds the parsed Exif attributes.
 */
class Exif final {
 public:
  Exif() = delete;
  Exif(Exif&&) = default;
  Exif& operator=(Exif&&) = default;
  Exif(const Exif&) = delete;
  Exif& operator=(const Exif&) = delete;

  ~Exif() = default;

  /**
   * Returns the slice that contains the TIFF data.
   */
  absl::Span<const uint8_t> buf() const ABSL_ATTRIBUTE_LIFETIME_BOUND;

  /**
   * Returns a span of Exif fields.
   */
  absl::Span<const Field> fields() const ABSL_ATTRIBUTE_LIFETIME_BOUND;

  /**
   * Returns true if the Exif data (TIFF structure) is in the
   * little-endian byte order.
   */
  bool little_endian() const;

  /**
   * Returns the Exif field specified by the tag and the IFD number, if found.
   */
  std::optional<Field> get_field(Tag tag, In in) const;

 private:
  friend class Reader;

  explicit Exif(exif_bridge_rs::reader::Exif exif);
  exif_bridge_rs::reader::Exif exif_;
  // Internal cached fields. When exif is created, this is populated from
  // exif_.fields(). Note that this only works because Exif fields is immutable
  // in this library.
  std::vector<Field> fields_;
  // Internal cached buffer. When exif is created, this is populated from
  // exif_.buf().
  std::vector<uint8_t> buf_;
};

/**
 * A class to parse the Exif attributes
 * and create an `Exif` instance that holds the results.
 */
class Reader final {
 public:
  // Constructs a new `Reader` for parsing Exif data.
  Reader();
  Reader(Reader&&) = default;
  Reader& operator=(Reader&&) = default;
  Reader(const Reader&) = delete;
  Reader& operator=(const Reader&) = delete;

  ~Reader() = default;

  // NOTE(b/491278916) - Add continue_on_error() when PartialResult is bridged.

  /**
   * Parses the Exif attributes from raw Exif data.
   */
  absl::StatusOr<Exif> read_raw(absl::Span<const uint8_t> data);

  /**
   * Reads an image file from a buffer and parses the Exif attributes in it.
   */
  absl::StatusOr<Exif> read_from_container(absl::Span<const uint8_t> data);

  /**
   * Reads an image file and parses the Exif attributes in it.
   * If an error occurred, an `absl::Status` is returned.
   *
   * Supported formats are:
   * - TIFF and some RAW image formats based on it
   * - JPEG
   * - HEIF and coding-specific variations including HEIC and AVIF
   * - PNG
   * - WebP
   */
  absl::StatusOr<Exif> read_from_container(File& file);

 private:
  exif_bridge_rs::reader::Reader reader_;
};

class Writer;

/**
 * A class that holds the written Exif data.
 *
 * This class owns the data returned by `Writer::write` and provides a view to
 * it via `view()`.
 */
class ExifBytes final {
 public:
  ExifBytes() = delete;
  ExifBytes(ExifBytes&&) = default;
  ExifBytes& operator=(ExifBytes&&) = default;
  ExifBytes(const ExifBytes&) = delete;
  ExifBytes& operator=(const ExifBytes&) = delete;
  ~ExifBytes() = default;

  /**
   * Returns a view to the Exif data.
   */
  absl::Span<const uint8_t> view() const {
    return absl::Span<const uint8_t>(vec_.as_ptr(), vec_.len());
  }

 private:
  friend class Writer;
  explicit ExifBytes(exif_bridge_rs::types::VecU8 vec) : vec_(std::move(vec)) {}
  exif_bridge_rs::types::VecU8 vec_;
};

/**
 * The `Writer` struct is used to encode and write Exif data.
 */
class Writer final {
 public:
  Writer();
  Writer(Writer&&) = default;
  Writer& operator=(Writer&&) = default;
  Writer(const Writer&) = delete;
  Writer& operator=(const Writer&) = delete;

  ~Writer() = default;

  /**
   * Appends a field to be written.
   *
   * The fields can be appended in any order.
   * Duplicate fields must not be appended.
   *
   * The following fields are ignored and synthesized when needed:
   * ExifIFDPointer, GPSInfoIFDPointer, InteropIFDPointer,
   * StripOffsets, StripByteCounts, TileOffsets, TileByteCounts,
   * JPEGInterchangeFormat, and JPEGInterchangeFormatLength.
   *
   */
  void push_field(Field field);
  /**
   * Sets TIFF strips for the specified IFD.
   * If this method is called multiple times, the last one is used.
   */
  void set_strips(std::vector<std::vector<uint8_t>> strips, In ifd_num);
  /**
   * Sets TIFF tiles for the specified IFD.
   * If this method is called multiple times, the last one is used.
   */
  void set_tiles(std::vector<std::vector<uint8_t>> tiles, In ifd_num);

  /**
   * Sets the JPEG data for the specified IFD.
   * If this method is called multiple times, the last one is used.
   */
  void set_jpeg(absl::Span<const uint8_t> jpeg, In ifd_num);
  /**
   * Encodes Exif data and returns it as an ExifBytes object.
   * View the data via `ExifBytes::view()`.
   */
  absl::StatusOr<ExifBytes> write(bool little_endian);

 private:
  // `std::deque` is used because pointer stability is needed for constructing
  // Rust side data structures that might reference memory in this deque.
  std::deque<Field> fields_storage_;

  /**
   * Storage for JPEG data corresponding to the IFD. There are only 2 IFDs, so
   * there are only 2 possible JPEG data. This storage ensures that the Rust
   * writer can reference them without risking use-after-free.
   */
  std::array<std::vector<uint8_t>, 2> jpeg_storage_;

  /**
   * Storage for tiles data corresponding to the IFD. There are only 2 IFDs, so
   * there are only 2 possible tiles data. This storage ensures that the Rust
   * writer can reference them without risking use-after-free.
   */
  std::array<std::vector<std::vector<uint8_t>>, 2> tiles_storage_;
  std::array<std::vector<rs_std::SliceRef<const uint8_t>>, 2>
      tiles_refs_storage_;

  /**
   * Storage for strips data corresponding to the IFD. There are only 2 IFDs, so
   * there are only 2 possible strips data. This storage ensures that the Rust
   * writer can reference them without risking use-after-free.
   */
  std::array<std::vector<std::vector<uint8_t>>, 2> strips_storage_;
  std::array<std::vector<rs_std::SliceRef<const uint8_t>>, 2>
      strips_refs_storage_;

  // writer_ must be declared last to ensure other members it might reference
  // are destroyed after it.
  exif_bridge_rs::writer::Writer writer_;
};

}  // namespace security::exif_bridge
#endif  // SECURITY_EXIF_BRIDGE_EXIF_BRIDGE_H_
