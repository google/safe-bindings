#include "exif_bridge.h"

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "crubit_helpers/string_conversions.h"
#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "crubit/rust.h"

namespace security::exif_bridge {
namespace {

using ::absl_testing::IsOk;
using ::absl_testing::StatusIs;
using ::security::crubit_helpers::StringViewFromVecU8;
using ::testing::ElementsAre;
using ::testing::ElementsAreArray;
using ::testing::Eq;
using ::testing::Optional;
using ::testing::StrEq;

static_assert(In::kPrimary.index() == 0);
static_assert(In::kThumbnail.index() == 1);

TEST(ExifBridgeTest, ValueEqual) {
  Value v1 = Value::Short({1, 2, 3});
  Value v2 = Value::Short({1, 2, 3});
  Value v3 = Value::Short({1, 2, 4});
  EXPECT_EQ(v1, v2);
  EXPECT_NE(v1, v3);
}

TEST(ExifBridgeTest, ValueShort) {
  Value v_shorts = Value::Short({10, 20, 30, 40});

  EXPECT_THAT(v_shorts.iter_uint(), Optional(ElementsAre(10, 20, 30, 40)));

  absl::StatusOr<rust::reexport::UIntValue> uint_view =
      v_shorts.as_uint();
  ASSERT_THAT(uint_view, IsOk());
  EXPECT_THAT(uint_view->get(2), Optional(30));
  EXPECT_FALSE(uint_view->get(5).has_value());

  EXPECT_THAT(v_shorts.get_uint(0), Optional(10));
  EXPECT_THAT(v_shorts.get_uint(1), Optional(20));
  EXPECT_THAT(v_shorts.get_uint(5), Eq(std::nullopt));
}

TEST(ExifBridgeTest, ValueGetInt) {
  Value v_shorts = Value::Short({10, 20, 30, 40});
  EXPECT_THAT(v_shorts.get_int(0), Optional(10));
  EXPECT_THAT(v_shorts.get_int(5), Eq(std::nullopt));

  Value v_sshorts = Value::SShort({-10, -20});
  EXPECT_THAT(v_sshorts.get_int(0), Optional(-10));
  EXPECT_THAT(v_sshorts.get_int(1), Optional(-20));
  EXPECT_THAT(v_sshorts.get_int(2), Eq(std::nullopt));
}

TEST(ExifBridgeTest, ValueGetFloat) {
  Value v_shorts = Value::Short({10, 20, 30, 40});
  EXPECT_THAT(v_shorts.get_float(0), Optional(10.0f));
  EXPECT_THAT(v_shorts.get_float(5), Eq(std::nullopt));

  Value v_floats = Value::Float({1.5f, -2.5f});
  EXPECT_THAT(v_floats.get_float(0), Optional(1.5f));
  EXPECT_THAT(v_floats.get_float(1), Optional(-2.5f));
  EXPECT_THAT(v_floats.get_float(2), Eq(std::nullopt));

  Value v_rationals =
      Value::Rational({rust::value::Rational::new_(5, 2)});
  EXPECT_THAT(v_rationals.get_float(0), Optional(2.5f));
  EXPECT_THAT(v_rationals.get_float(1), Eq(std::nullopt));
}

TEST(ExifBridgeTest, ValueGetDouble) {
  Value v_shorts = Value::Short({10, 20});
  EXPECT_THAT(v_shorts.get_double(0), Optional(10.0));
  EXPECT_THAT(v_shorts.get_double(2), Eq(std::nullopt));

  Value v_rationals =
      Value::Rational({rust::value::Rational::new_(5, 2)});
  EXPECT_THAT(v_rationals.get_double(0), Optional(2.5));
  EXPECT_THAT(v_rationals.get_double(1), Eq(std::nullopt));
}

TEST(ExifBridgeTest, ValueNonUInt) {
  Value v_asciis = Value::Ascii({"hello", "world"});

  EXPECT_FALSE(v_asciis.iter_uint().has_value());
  EXPECT_FALSE(v_asciis.as_uint().ok());
  EXPECT_FALSE(v_asciis.get_uint(0).has_value());
}

TEST(ExifBridgeTest, ValueDisplayAs) {
  EXPECT_EQ(StringViewFromVecU8(Value::Undefined({'0', '2', '3', '1'}, 0)
                                    .display_as(Tag::kExifVersion)
                                    .to_string()),
            "2.31");
  EXPECT_EQ(StringViewFromVecU8(
                Value::Short({2}).display_as(Tag::kResolutionUnit).to_string()),
            "inch");
}

TEST(ExifBridgeTest, TagTest) {
  EXPECT_EQ(Tag::kXResolution.number(), 0x11a);
  EXPECT_THAT(Tag::kXResolution.description(),
              Optional(Eq("Image resolution in width direction")));
}

TEST(ExifBridgeTest, TagFromU16) {
  Tag t = Tag::from_u16(rust::Context::Exif::Make(), 0x9003);
  EXPECT_EQ(t, Tag::kDateTimeOriginal);
  EXPECT_EQ(t.number(), 0x9003);
}

TEST(ExifBridgeTest, TagEqual) {
  EXPECT_EQ(Tag::kXResolution, Tag::kXResolution);
  EXPECT_NE(Tag::kXResolution, Tag::kYResolution);

  EXPECT_LT(Tag::kXResolution, Tag::kYResolution);
  EXPECT_GT(Tag::kYResolution, Tag::kXResolution);

  EXPECT_LE(Tag::kXResolution, Tag::kYResolution);
  EXPECT_LE(Tag::kXResolution, Tag::kXResolution);

  EXPECT_GE(Tag::kYResolution, Tag::kXResolution);
  EXPECT_GE(Tag::kXResolution, Tag::kXResolution);
}

TEST(ExifBridgeTest, ParseExifOk) {
  static constexpr auto data = std::to_array<uint8_t>(
      {'M',  'M',  0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00,
       0x01, 0x01, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
       0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00});
  absl::StatusOr<TiffExifData> parsed = parse_exif(data);
  ASSERT_THAT(parsed, IsOk());
  ASSERT_EQ(parsed->fields.size(), 1);
  EXPECT_EQ(parsed->fields[0].value(), Value::Unknown(0xffff, 1, 0x12));
  EXPECT_FALSE(parsed->is_little_endian);
}

TEST(ExifBridgeTest, ParseMakerNotesMock) {
  static constexpr auto data = std::to_array<uint8_t>(
      {// TIFF Header (Offset 0)
       'M', 'M', 0, 42, 0, 0, 0, 8,
       // IFD 0 (Offset 8)
       0, 2,  // Count: 2
              // Entry 1: Make (0x010f), Type: ASCII (2), Count: 6, Offset: 38
       0x01, 0x0f, 0, 2, 0, 0, 0, 6, 0, 0, 0, 38,
       // Entry 2: MakerNote (0x927c), Type: UNDEFINED (7), Count: 18, Offset:
       // 44
       0x92, 0x7c, 0, 7, 0, 0, 0, 18, 0, 0, 0, 44,
       // Next IFD Offset (Offset 34)
       0, 0, 0, 0,
       // Make string "Canon\0" (Offset 38)
       'C', 'a', 'n', 'o', 'n', '\0',
       // MakerNote payload (Offset 44) - Canon MakerNote (no header, direct
       // IFD)
       0, 1,  // Count: 1
              // Sub-tag: 3, Type: SHORT (3), Count: 1, Value: 42 (0x002a)
       0x00, 0x03, 0, 3, 0, 0, 0, 1, 0, 42, 0, 0,
       // Next IFD Offset (Offset 58)
       0, 0, 0, 0});

  Reader reader;
  absl::StatusOr<Exif> exif = reader.read_raw(data);
  ASSERT_THAT(exif, IsOk());

  // Verify MakerNotes type is 2 (Canon)
  EXPECT_EQ(exif->get_mnote_type(), 2);

  // Verify we can retrieve the MakerNote sub-tag 3
  std::optional<Field> field = exif->get_mnote_field(3);
  ASSERT_TRUE(field.has_value());
  EXPECT_EQ(field->tag().number(), 3);
  EXPECT_THAT(field->value().get_int(0), Optional(42));
}

TEST(ExifBridgeTest, ParseExifError) {
  static constexpr auto data = std::to_array<uint8_t>(
      {'M',  'M',  0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00,
       0x01, 0x87, 0x69, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01,
       0x00, 0x00, 0x00, 0x1a, 0x00, 0x00, 0x00, 0x00, 0x00,
       0x01, 0x90, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x04,
       '0',  '2',  '3',  '1',  0x00, 0x00, 0x00, 0x08});
  EXPECT_THAT(parse_exif(data), StatusIs(absl::StatusCode::kInvalidArgument,
                                         StrEq("Unexpected next IFD")));
}

TEST(ExifBridgeTest, WriterPushFieldWrite) {
  Writer writer;
  // This scope is here as a sanity check. Even if field is out of scope, we
  // copied it into the writer, so later steps should NOT fail.
  // If it fails it means that we are allowing potential use-after-free.
  {
    Field field =
        Field(Tag::kImageDescription, In::kPrimary, Value::Ascii({"Sample"}));
    writer.push_field(std::move(field));
  }

  absl::StatusOr<ExifBytes> buf = writer.write(false);
  ASSERT_THAT(buf, IsOk());
  static constexpr auto expected = std::to_array<uint8_t>(
      {0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x01, 0x01,
       0x0e, 0x00, 0x02, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x1a,
       0x00, 0x00, 0x00, 0x00, 'S',  'a',  'm',  'p',  'l',  'e',  0x00});
  EXPECT_THAT(buf->view(), ElementsAreArray(expected));
}

TEST(ExifBridgeTest, WriterSetStripsWrite) {
  Writer writer;
  Field field =
      Field(Tag::kImageDescription, In::kPrimary, Value::Ascii({"tif"}));
  writer.push_field(std::move(field));

  // This scope is here as a sanity check. Even if tile is out of scope, we
  // copied it into the writer, so later steps should NOT fail with ASAN.
  // If it fails it means that we are allowing potential use-after-free.
  {
    uint8_t strip_data[] = "STRIP";
    std::vector<std::vector<uint8_t>> strips = {
        std::vector<uint8_t>(strip_data, strip_data + sizeof(strip_data))};
    writer.set_strips(std::move(strips), In::kThumbnail);
  }

  absl::StatusOr<ExifBytes> buf = writer.write(false);
  ASSERT_THAT(buf, IsOk());
  static constexpr auto expected = std::to_array<uint8_t>(
      {0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x01, 0x01,
       0x0e, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 't',  'i',  'f',  0x00,
       0x00, 0x00, 0x00, 0x1a, 0x00, 0x02, 0x01, 0x11, 0x00, 0x04, 0x00,
       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x38, 0x01, 0x17, 0x00, 0x04,
       0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
       0x00, 'S',  'T',  'R',  'I',  'P',  0x00});
  EXPECT_THAT(buf->view(), ElementsAreArray(expected));
}

TEST(ExifBridgeTest, WriterSetTilesWrite) {
  Writer writer;
  // This scope is here as a sanity check. Even if tile is out of scope, we
  // copied it into the writer, so later steps should NOT fail with ASAN.
  // If it fails it means that we are allowing potential use-after-free.
  {
    uint8_t tile_data[] = "TILE";
    std::vector<std::vector<uint8_t>> tiles = {
        std::vector<uint8_t>(tile_data, tile_data + sizeof(tile_data))};
    writer.set_tiles(std::move(tiles), In::kPrimary);
  }
  absl::StatusOr<ExifBytes> buf = writer.write(false);
  ASSERT_THAT(buf, IsOk());
  static constexpr auto expected = std::to_array<uint8_t>(
      {0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x02, 0x01,
       0x44, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x26,
       0x01, 0x45, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
       0x05, 0x00, 0x00, 0x00, 0x00, 'T',  'I',  'L',  'E',  0x00});
  EXPECT_THAT(buf->view(), ElementsAreArray(expected));
}

TEST(ExifBridgeTest, WriterSetJpegWrite) {
  Writer writer;
  // This scope is here as a sanity check. Original Rust library requires jpeg
  // to live longer than writer. Since C++ does not check this, we designed the
  // API such that it should still produce valid result if user accidentally
  // let jpeg be destroyed too early.
  {
    Field field =
        Field(Tag::kImageDescription, In::kPrimary, Value::Ascii({"jpg"}));
    writer.push_field(std::move(field));
    // If static constexpr is present, this variable will not be stored on stack
    // and ASAN will not flag this. Since we want this test to fail if there is
    // a use-after-free, we do not use a static constexpr here.
    uint8_t jpg[] = "JPG";
    uint8_t jpeg[] = "JPEG";
    // First set_jpeg call should be overwritten by second set_jpeg call.
    writer.set_jpeg(jpg, In::kThumbnail);
    writer.set_jpeg(jpeg, In::kThumbnail);
  }

  absl::StatusOr<ExifBytes> buf = writer.write(false);
  ASSERT_THAT(buf, IsOk());
  static constexpr auto expected = std::to_array<uint8_t>(
      {0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x01, 0x01,
       0x0e, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 'j',  'p',  'g',  0x00,
       0x00, 0x00, 0x00, 0x1a, 0x00, 0x02, 0x02, 0x01, 0x00, 0x04, 0x00,
       0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x38, 0x02, 0x02, 0x00, 0x04,
       0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
       0x00, 'J',  'P',  'E',  'G',  0x00});
  EXPECT_THAT(buf->view(), ElementsAreArray(expected));
}

TEST(ExifBridgeTest, WriterHandlesManyFields) {
  Writer writer;

  writer.push_field(
      Field(Tag::kImageDescription, In::kPrimary, Value::Ascii({"Sample"})));

  std::array<Tag, 8> tags = {Tag::kMake,
                             Tag::kModel,
                             Tag::kSoftware,
                             Tag::kArtist,
                             Tag::kCopyright,
                             Tag::kDateTime,
                             Tag::kDateTimeOriginal,
                             Tag::kDateTimeDigitized};

  // Add more fields to trigger resize.
  for (const Tag& tag : tags) {
    writer.push_field(Field(tag, In::kPrimary, Value::Ascii({"Value"})));
  }

  absl::StatusOr<ExifBytes> buf = writer.write(false);
  ASSERT_THAT(buf, IsOk());
  EXPECT_GT(buf->view().size(), 0);

  // Make sure all fields are present.
  Reader reader;
  absl::StatusOr<Exif> exif = reader.read_raw(buf->view());
  ASSERT_THAT(exif, IsOk());
  for (const Tag& tag : tags) {
    ASSERT_TRUE(exif->get_field(tag, In::kPrimary).has_value());
  }
}

void AppendU16(std::vector<uint8_t>& v, uint16_t x) {
  v.push_back(x >> 8);
  v.push_back(x & 0xff);
}

void AppendU32(std::vector<uint8_t>& v, uint32_t x) {
  AppendU16(v, x >> 16);
  AppendU16(v, x & 0xffff);
}

// A big-endian TIFF with `num_ifds` chained IFDs. IFD i holds a single
// ImageWidth field with the value 100 + i.
std::vector<uint8_t> MultiIfdTiff(int num_ifds) {
  constexpr uint32_t kIfdSize = 2 + 12 + 4;
  std::vector<uint8_t> tiff = {'M', 'M', 0x00, 0x2a};
  AppendU32(tiff, 8);
  for (int i = 0; i < num_ifds; ++i) {
    AppendU16(tiff, 1);       // Entry count.
    AppendU16(tiff, 0x0100);  // ImageWidth.
    AppendU16(tiff, 3);       // SHORT.
    AppendU32(tiff, 1);       // Count.
    AppendU16(tiff, 100 + i);
    AppendU16(tiff, 0);
    const bool last = i + 1 == num_ifds;
    AppendU32(tiff, last ? 0 : 8 + (i + 1) * kIfdSize);
  }
  return tiff;
}

// Size of the TIFF data that CanonMakerNoteTiff(entries, ...) returns.
uint32_t CanonMakerNoteTiffSize(int entries) {
  return 44 + 2 + 12 * entries + 4;
}

// A big-endian TIFF whose IFD 0 holds Make "Canon" and a Canon maker note
// with `entries` entries, numbered from 1. Each is UNDEFINED and points at the
// first `value_size` bytes of the TIFF data.
std::vector<uint8_t> CanonMakerNoteTiff(int entries, uint32_t value_size) {
  const uint32_t maker_note_size = CanonMakerNoteTiffSize(entries) - 44;
  std::vector<uint8_t> tiff = {'M', 'M', 0x00, 0x2a};
  AppendU32(tiff, 8);
  AppendU16(tiff, 2);  // Entry count.
  AppendU16(tiff, 0x010f);  // Make, ASCII, 6 bytes at offset 38.
  AppendU16(tiff, 2);
  AppendU32(tiff, 6);
  AppendU32(tiff, 38);
  AppendU16(tiff, 0x927c);  // MakerNote, UNDEFINED, at offset 44.
  AppendU16(tiff, 7);
  AppendU32(tiff, maker_note_size);
  AppendU32(tiff, 44);
  AppendU32(tiff, 0);  // No next IFD.
  for (char c : std::string("Canon", 6)) tiff.push_back(c);
  // Canon maker notes are a bare IFD with offsets into the TIFF data.
  AppendU16(tiff, entries);
  for (int i = 0; i < entries; ++i) {
    AppendU16(tiff, i + 1);
    AppendU16(tiff, 7);
    AppendU32(tiff, value_size);
    AppendU32(tiff, 0);
  }
  AppendU32(tiff, 0);
  return tiff;
}

Field AsciiField(const Tag& tag, const std::string& s) {
  return Field(tag, In::kPrimary, Value::Ascii({s}));
}

std::vector<uint8_t> Bytes(absl::string_view s) {
  return std::vector<uint8_t>(s.begin(), s.end());
}

// Returns the Field at `ifd_index` with `tag`.
std::optional<Field> FindField(const Exif& exif, const Tag& tag,
                               uint16_t ifd_index) {
  for (const Field& f : exif.fields()) {
    if (f.tag() == tag && f.ifd_num().index() == ifd_index) {
      return exif.get_field(tag, f.ifd_num());
    }
  }
  return std::nullopt;
}

TEST(ExifBridgeTest, FieldsReportIfdsBeyondTheThumbnail) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_THAT(exif, IsOk());

  std::vector<uint16_t> ifds;
  for (const Field& f : exif->fields()) ifds.push_back(f.ifd_num().index());
  EXPECT_THAT(ifds, ElementsAre(0, 1, 2));

  // get_field() with the In of a field in IFD 2 used to look in IFD 0.
  const In ifd2 = exif->fields()[2].ifd_num();
  std::optional<Field> width = exif->get_field(Tag::kImageWidth, ifd2);
  ASSERT_TRUE(width.has_value());
  EXPECT_THAT(width->value().get_uint(0), Optional(102));
}

TEST(ExifBridgeTest, MovedExifKeepsItsFields) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_THAT(exif, IsOk());
  Exif moved(std::move(*exif));

  EXPECT_EQ(moved.fields().size(), 3);
  EXPECT_THAT(moved.get_field(Tag::kImageWidth, In::kThumbnail)
                  ->value()
                  .get_uint(0),
              Optional(101));
  // A moved-from Exif has no fields.
  EXPECT_TRUE(exif->fields().empty());  // NOLINT(bugprone-use-after-move)
}

TEST(ExifBridgeTest, ParseMakerNoteValueAtOffset) {
  const std::vector<uint8_t> tiff = CanonMakerNoteTiff(1, 8);
  absl::StatusOr<Exif> exif = Reader().read_raw(tiff);
  ASSERT_THAT(exif, IsOk());

  EXPECT_EQ(exif->get_mnote_type(), 2);  // Canon.
  ASSERT_EQ(exif->mnote_fields().size(), 1);
  std::optional<Field> field = exif->get_mnote_field(1);
  ASSERT_TRUE(field.has_value());
  EXPECT_THAT(field->value().get_bytes(),
              Optional(std::vector<uint8_t>(tiff.begin(), tiff.begin() + 8)));
}

// The reader parses the maker note up front, so entries that all point at the
// same bytes used to cost (entries x size) memory before the caller asked for
// anything.
TEST(ExifBridgeTest, RejectMakerNoteThatRepeatsTheSameBytes) {
  constexpr int kEntries = 5;
  absl::StatusOr<Exif> exif = Reader().read_raw(
      CanonMakerNoteTiff(kEntries, CanonMakerNoteTiffSize(kEntries)));
  ASSERT_THAT(exif, IsOk());

  EXPECT_EQ(exif->get_mnote_type(), 2);  // Canon.
  EXPECT_TRUE(exif->mnote_fields().empty());
  // The rest of the data is still readable.
  EXPECT_TRUE(exif->get_field(Tag::kMake, In::kPrimary).has_value());
}

// Writer used to index a two-element array with In::index(), which was 65535
// for any IFD after the thumbnail.
TEST(ExifBridgeTest, WriterWritesDataForIfdsBeyondTheThumbnail) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_THAT(exif, IsOk());
  const In ifd2 = exif->fields()[2].ifd_num();

  Writer writer;
  for (const Field& f : exif->fields()) {
    writer.push_field(Field(f.tag(), f.ifd_num(),
                            Value::Short({static_cast<uint16_t>(
                                *f.value().get_uint(0))})));
  }
  writer.set_jpeg(std::vector<uint8_t>(16, 0xab), ifd2);
  writer.set_strips({{1, 2, 3}, {4, 5}}, ifd2);

  absl::StatusOr<ExifBytes> out = writer.write(/*little_endian=*/false);
  ASSERT_THAT(out, IsOk());
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_THAT(reread, IsOk());

  std::optional<Field> width = FindField(*reread, Tag::kImageWidth, 2);
  ASSERT_TRUE(width.has_value());
  EXPECT_THAT(width->value().get_uint(0), Optional(102));
  std::optional<Field> jpeg_length =
      FindField(*reread, Tag::kJPEGInterchangeFormatLength, 2);
  ASSERT_TRUE(jpeg_length.has_value());
  EXPECT_THAT(jpeg_length->value().get_uint(0), Optional(16));
  std::optional<Field> strip_counts =
      FindField(*reread, Tag::kStripByteCounts, 2);
  ASSERT_TRUE(strip_counts.has_value());
  EXPECT_THAT(strip_counts->value().iter_uint(), Optional(ElementsAre(3, 2)));
}

TEST(ExifBridgeTest, WriterSettingDataAgainReplacesIt) {
  Writer writer;
  writer.push_field(AsciiField(Tag::kArtist, "a"));
  writer.set_strips({{1, 2, 3}}, In::kPrimary);
  writer.set_strips({{4, 5}, {6}}, In::kPrimary);

  absl::StatusOr<ExifBytes> out = writer.write(/*little_endian=*/true);
  ASSERT_THAT(out, IsOk());
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_THAT(reread, IsOk());
  std::optional<Field> strip_counts =
      reread->get_field(Tag::kStripByteCounts, In::kPrimary);
  ASSERT_TRUE(strip_counts.has_value());
  EXPECT_THAT(strip_counts->value().iter_uint(), Optional(ElementsAre(2, 1)));
}

// A defaulted move assignment freed the target's fields while the Rust writer
// that the source was left with still referenced them.
TEST(ExifBridgeTest, WriterMoveAssignmentLeavesBothWritersUsable) {
  Writer a;
  a.push_field(AsciiField(Tag::kArtist, std::string(200, 'A')));
  Writer b;
  b.push_field(AsciiField(Tag::kCopyright, "b"));

  a = std::move(b);
  // A moved-from Writer is empty and can be reused.
  // NOLINTNEXTLINE(bugprone-use-after-move)
  b.push_field(AsciiField(Tag::kSoftware, "s"));

  absl::StatusOr<ExifBytes> out_a = a.write(/*little_endian=*/false);
  ASSERT_THAT(out_a, IsOk());
  absl::StatusOr<Exif> reread_a = Reader().read_raw(out_a->view());
  ASSERT_THAT(reread_a, IsOk());
  ASSERT_EQ(reread_a->fields().size(), 1);
  EXPECT_EQ(reread_a->fields()[0].tag(), Tag::kCopyright);
  EXPECT_THAT(reread_a->fields()[0].value().get_bytes(),
              Optional(Bytes("b")));

  absl::StatusOr<ExifBytes> out_b = b.write(/*little_endian=*/false);
  ASSERT_THAT(out_b, IsOk());
  absl::StatusOr<Exif> reread_b = Reader().read_raw(out_b->view());
  ASSERT_THAT(reread_b, IsOk());
  ASSERT_EQ(reread_b->fields().size(), 1);
  EXPECT_EQ(reread_b->fields()[0].tag(), Tag::kSoftware);
  EXPECT_THAT(reread_b->fields()[0].value().get_bytes(),
              Optional(Bytes("s")));
}

TEST(ExifBridgeTest, WriterMoveConstructionKeepsData) {
  Writer a;
  a.push_field(AsciiField(Tag::kArtist, "a"));
  a.set_jpeg(std::vector<uint8_t>(8, 0xcd), In::kPrimary);
  Writer b(std::move(a));

  absl::StatusOr<ExifBytes> out = b.write(/*little_endian=*/false);
  ASSERT_THAT(out, IsOk());
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_THAT(reread, IsOk());
  std::optional<Field> jpeg_length =
      reread->get_field(Tag::kJPEGInterchangeFormatLength, In::kPrimary);
  ASSERT_TRUE(jpeg_length.has_value());
  EXPECT_THAT(jpeg_length->value().get_uint(0), Optional(8));
}

// Pushing a moved-from Field used to panic in Rust, which aborts the process.
TEST(ExifBridgeTest, WriterIgnoresMovedFromField) {
  Field f = AsciiField(Tag::kArtist, "x");
  Field g = std::move(f);
  Writer writer;
  writer.push_field(std::move(f));  // NOLINT(bugprone-use-after-move)
  writer.push_field(std::move(g));

  absl::StatusOr<ExifBytes> out = writer.write(/*little_endian=*/false);
  ASSERT_THAT(out, IsOk());
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_THAT(reread, IsOk());
  ASSERT_EQ(reread->fields().size(), 1);
  EXPECT_EQ(reread->fields()[0].tag(), Tag::kArtist);
}

template <typename T>
concept CanCallValue = requires(T f) { std::forward<T>(f).value(); };
template <typename T>
concept CanCallBuf = requires(T e) { std::forward<T>(e).buf(); };
template <typename T>
concept CanCallFields = requires(T e) { std::forward<T>(e).fields(); };
template <typename T>
concept CanCallMnoteFields =
    requires(T e) { std::forward<T>(e).mnote_fields(); };
template <typename T>
concept CanCallView = requires(T b) { std::forward<T>(b).view(); };

static_assert(CanCallValue<const Field&>);
static_assert(!CanCallValue<Field&&>);
static_assert(!CanCallValue<const Field&&>);

static_assert(CanCallBuf<const Exif&>);
static_assert(!CanCallBuf<Exif&&>);
static_assert(!CanCallBuf<const Exif&&>);

static_assert(CanCallFields<const Exif&>);
static_assert(!CanCallFields<Exif&&>);
static_assert(!CanCallFields<const Exif&&>);

static_assert(CanCallMnoteFields<const Exif&>);
static_assert(!CanCallMnoteFields<Exif&&>);
static_assert(!CanCallMnoteFields<const Exif&&>);

static_assert(CanCallView<const ExifBytes&>);
static_assert(!CanCallView<ExifBytes&&>);
static_assert(!CanCallView<const ExifBytes&&>);

}  // namespace
}  // namespace security::exif_bridge
