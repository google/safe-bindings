#ifndef SECURITY_ZIP_READ_H_
#define SECURITY_ZIP_READ_H_

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "file.h"
#include "crubit/rust.h"
#include "absl/base/attributes.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

class BufferedZipArchive final {
 public:
  static absl::StatusOr<BufferedZipArchive> NewFromData(absl::string_view data);
  absl::StatusOr<uintptr_t> GetLength() const;
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns what the central directory records about the entry at `index`,
  // reading nothing. Prefer this to GetFileByIndex when the entry's contents
  // are not wanted: that one also reads the entry's local file header, so
  // listing an archive with it costs one read per entry.
  absl::StatusOr<ZipEntryMetadata> GetEntryMetadata(uintptr_t index) const;
  [[nodiscard]] bool IsNone() const;

 private:
  rust::BufferedZipArchive archive_;
  explicit BufferedZipArchive(rust::BufferedZipArchive archive)
      : archive_(std::move(archive)) {}
};

class FsZipArchive final {
 public:
  static absl::StatusOr<FsZipArchive> NewFromPath(absl::string_view path);
  // Returns an independent handle onto the same archive.
  //
  // The new handle shares the central directory that NewFromPath already
  // parsed, so this costs one open(2) and is O(1) in the number of entries,
  // whereas NewFromPath is O(entries). Use it when several entries must be read
  // at once: GetFileByIndex keeps the archive borrowed for as long as the
  // returned ZipFile lives, so N concurrently open entries need N handles, and
  // reopening by path for each one is quadratic on large archives.
  //
  // Each handle owns its own file descriptor and therefore its own file
  // position, so handles may be read independently.
  //
  // The caller must ensure the file has not been modified since it was opened;
  // the shared central directory would otherwise describe byte ranges that no
  // longer hold the entries it names.
  absl::StatusOr<FsZipArchive> TryClone() const;
  absl::StatusOr<uintptr_t> GetLength() const;
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns what the central directory records about the entry at `index`,
  // reading nothing. Prefer this to GetFileByIndex when the entry's contents
  // are not wanted: that one also reads the entry's local file header, so
  // listing an archive with it costs one read per entry.
  absl::StatusOr<ZipEntryMetadata> GetEntryMetadata(uintptr_t index) const;
  [[nodiscard]] bool IsNone() const;

 private:
  rust::FsZipArchive archive_;
  explicit FsZipArchive(rust::FsZipArchive archive)
      : archive_(std::move(archive)) {}
};

class ZipArchive final {
 public:
  static absl::StatusOr<ZipArchive> FromFile(absl::string_view path);
  static absl::StatusOr<ZipArchive> FromBuffer(absl::string_view data);
  // Returns an independent handle onto the same archive, sharing the central
  // directory this one already parsed. See FsZipArchive::TryClone for the cost
  // model and the caller's obligations.
  //
  // Not supported for archives opened with FromBuffer, which return
  // absl::UnimplementedError, because their reader cannot currently be shared
  // and cloning one would copy the whole archive.
  absl::StatusOr<ZipArchive> TryClone() const;
  absl::StatusOr<uintptr_t> GetLength() const;
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns what the central directory records about the entry at `index`,
  // reading nothing. Prefer this to GetFileByIndex when the entry's contents
  // are not wanted: that one also reads the entry's local file header, so
  // listing an archive with it costs one read per entry.
  absl::StatusOr<ZipEntryMetadata> GetEntryMetadata(uintptr_t index) const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipArchive, FsZipArchive>;
  BackendType archive_;

  explicit ZipArchive(BackendType archive) : archive_(std::move(archive)) {}
};

// Streaming readers that read sequential zip entries from a stream without
// seeking.
class BufferedZipStreamReader final {
 public:
  static absl::StatusOr<BufferedZipStreamReader> NewFromData(
      absl::string_view data);
  // Reads the next file in the stream. Returns std::nullopt when end of archive
  // is reached.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Reads the next file in the stream with an assumed compressed size.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFileWithCompressedSize(
      uint64_t compressed_size) ABSL_ATTRIBUTE_LIFETIME_BOUND;
  [[nodiscard]] bool IsFinished() const;
  [[nodiscard]] bool IsNone() const;

 private:
  rust::BufferedZipStreamReader reader_;
  explicit BufferedZipStreamReader(rust::BufferedZipStreamReader reader)
      : reader_(std::move(reader)) {}
};

class FsZipStreamReader final {
 public:
  static absl::StatusOr<FsZipStreamReader> NewFromPath(absl::string_view path);
  // Reads the next file in the stream. Returns std::nullopt when end of archive
  // is reached.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Reads the next file in the stream with an assumed compressed size.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFileWithCompressedSize(
      uint64_t compressed_size) ABSL_ATTRIBUTE_LIFETIME_BOUND;
  [[nodiscard]] bool IsFinished() const;
  [[nodiscard]] bool IsNone() const;

 private:
  rust::FsZipStreamReader reader_;
  explicit FsZipStreamReader(rust::FsZipStreamReader reader)
      : reader_(std::move(reader)) {}
};

class ZipStreamReader final {
 public:
  static absl::StatusOr<ZipStreamReader> FromFile(absl::string_view path);
  static absl::StatusOr<ZipStreamReader> FromBuffer(absl::string_view data);
  // Reads the next file in the stream. Returns std::nullopt when end of archive
  // is reached.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Reads the next file in the stream with an assumed compressed size.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFileWithCompressedSize(
      uint64_t compressed_size) ABSL_ATTRIBUTE_LIFETIME_BOUND;
  [[nodiscard]] bool IsFinished() const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipStreamReader, FsZipStreamReader>;
  BackendType reader_;

  explicit ZipStreamReader(BackendType reader) : reader_(std::move(reader)) {}
};

// Free function helpers mirroring zip::read::read_zipfile_from_stream.
absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    BufferedZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND);
absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    FsZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND);
absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    ZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND);

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    BufferedZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND,
    uint64_t compressed_size);
absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    FsZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND,
    uint64_t compressed_size);
absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    ZipStreamReader& reader ABSL_ATTRIBUTE_LIFETIME_BOUND,
    uint64_t compressed_size);

}  // namespace security::zip

#endif  // SECURITY_ZIP_READ_H_
