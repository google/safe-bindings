#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "exif_bridge.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::exif_bridge {
namespace {

using ::testing::ElementsAre;
using ::testing::Optional;

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

TEST(InTest, FieldsReportIfdsBeyondTheThumbnail) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_TRUE(exif.ok()) << exif.status();

  std::vector<uint16_t> ifds;
  for (const Field& f : exif->fields()) ifds.push_back(f.ifd_num().index());
  EXPECT_THAT(ifds, ElementsAre(0, 1, 2));

  // get_field() with the In of a field in IFD 2 used to look in IFD 0.
  const In ifd2 = exif->fields()[2].ifd_num();
  std::optional<Field> width = exif->get_field(Tag::kImageWidth, ifd2);
  ASSERT_TRUE(width.has_value());
  EXPECT_THAT(width->value().get_uint(0), Optional(102));
}

TEST(ExifTest, MovedExifKeepsItsFields) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_TRUE(exif.ok()) << exif.status();
  Exif moved(std::move(*exif));

  EXPECT_EQ(moved.fields().size(), 3);
  EXPECT_THAT(moved.get_field(Tag::kImageWidth, In::kThumbnail)
                  ->value()
                  .get_uint(0),
              Optional(101));
  // A moved-from Exif has no fields.
  EXPECT_TRUE(exif->fields().empty());  // NOLINT(bugprone-use-after-move)
}

TEST(MakerNoteTest, ParsesCanonMakerNote) {
  const std::vector<uint8_t> tiff = CanonMakerNoteTiff(1, 8);
  absl::StatusOr<Exif> exif = Reader().read_raw(tiff);
  ASSERT_TRUE(exif.ok()) << exif.status();

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
TEST(MakerNoteTest, RejectsMakerNoteThatRepeatsTheSameBytes) {
  constexpr int kEntries = 5;
  absl::StatusOr<Exif> exif = Reader().read_raw(
      CanonMakerNoteTiff(kEntries, CanonMakerNoteTiffSize(kEntries)));
  ASSERT_TRUE(exif.ok()) << exif.status();

  EXPECT_EQ(exif->get_mnote_type(), 2);  // Canon.
  EXPECT_TRUE(exif->mnote_fields().empty());
  // The rest of the data is still readable.
  EXPECT_TRUE(exif->get_field(Tag::kMake, In::kPrimary).has_value());
}

// Writer used to index a two-element array with In::index(), which was 65535
// for any IFD after the thumbnail.
TEST(WriterTest, WritesDataForIfdsBeyondTheThumbnail) {
  absl::StatusOr<Exif> exif = Reader().read_raw(MultiIfdTiff(3));
  ASSERT_TRUE(exif.ok()) << exif.status();
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
  ASSERT_TRUE(out.ok()) << out.status();
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_TRUE(reread.ok()) << reread.status();

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

TEST(WriterTest, SettingDataAgainReplacesIt) {
  Writer writer;
  writer.push_field(AsciiField(Tag::kArtist, "a"));
  writer.set_strips({{1, 2, 3}}, In::kPrimary);
  writer.set_strips({{4, 5}, {6}}, In::kPrimary);

  absl::StatusOr<ExifBytes> out = writer.write(/*little_endian=*/true);
  ASSERT_TRUE(out.ok()) << out.status();
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_TRUE(reread.ok()) << reread.status();
  std::optional<Field> strip_counts =
      reread->get_field(Tag::kStripByteCounts, In::kPrimary);
  ASSERT_TRUE(strip_counts.has_value());
  EXPECT_THAT(strip_counts->value().iter_uint(), Optional(ElementsAre(2, 1)));
}

// A defaulted move assignment freed the target's fields while the Rust writer
// that the source was left with still referenced them.
TEST(WriterTest, MoveAssignmentLeavesBothWritersUsable) {
  Writer a;
  a.push_field(AsciiField(Tag::kArtist, std::string(200, 'A')));
  Writer b;
  b.push_field(AsciiField(Tag::kCopyright, "b"));

  a = std::move(b);
  // A moved-from Writer is empty and can be reused.
  // NOLINTNEXTLINE(bugprone-use-after-move)
  b.push_field(AsciiField(Tag::kSoftware, "s"));

  absl::StatusOr<ExifBytes> out_a = a.write(/*little_endian=*/false);
  ASSERT_TRUE(out_a.ok()) << out_a.status();
  absl::StatusOr<Exif> reread_a = Reader().read_raw(out_a->view());
  ASSERT_TRUE(reread_a.ok()) << reread_a.status();
  ASSERT_EQ(reread_a->fields().size(), 1);
  EXPECT_EQ(reread_a->fields()[0].tag(), Tag::kCopyright);
  EXPECT_THAT(reread_a->fields()[0].value().get_bytes(),
              Optional(Bytes("b")));

  absl::StatusOr<ExifBytes> out_b = b.write(/*little_endian=*/false);
  ASSERT_TRUE(out_b.ok()) << out_b.status();
  absl::StatusOr<Exif> reread_b = Reader().read_raw(out_b->view());
  ASSERT_TRUE(reread_b.ok()) << reread_b.status();
  ASSERT_EQ(reread_b->fields().size(), 1);
  EXPECT_EQ(reread_b->fields()[0].tag(), Tag::kSoftware);
  EXPECT_THAT(reread_b->fields()[0].value().get_bytes(),
              Optional(Bytes("s")));
}

TEST(WriterTest, MoveConstructionKeepsData) {
  Writer a;
  a.push_field(AsciiField(Tag::kArtist, "a"));
  a.set_jpeg(std::vector<uint8_t>(8, 0xcd), In::kPrimary);
  Writer b(std::move(a));

  absl::StatusOr<ExifBytes> out = b.write(/*little_endian=*/false);
  ASSERT_TRUE(out.ok()) << out.status();
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_TRUE(reread.ok()) << reread.status();
  std::optional<Field> jpeg_length =
      reread->get_field(Tag::kJPEGInterchangeFormatLength, In::kPrimary);
  ASSERT_TRUE(jpeg_length.has_value());
  EXPECT_THAT(jpeg_length->value().get_uint(0), Optional(8));
}

// Pushing a moved-from Field used to panic in Rust, which aborts the process.
TEST(WriterTest, PushingAMovedFromFieldIsIgnored) {
  Field f = AsciiField(Tag::kArtist, "x");
  Field g = std::move(f);
  Writer writer;
  writer.push_field(std::move(f));  // NOLINT(bugprone-use-after-move)
  writer.push_field(std::move(g));

  absl::StatusOr<ExifBytes> out = writer.write(/*little_endian=*/false);
  ASSERT_TRUE(out.ok()) << out.status();
  absl::StatusOr<Exif> reread = Reader().read_raw(out->view());
  ASSERT_TRUE(reread.ok()) << reread.status();
  ASSERT_EQ(reread->fields().size(), 1);
  EXPECT_EQ(reread->fields()[0].tag(), Tag::kArtist);
}

}  // namespace
}  // namespace security::exif_bridge
