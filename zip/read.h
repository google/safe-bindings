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
#include "absl/status/status.h"
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
  absl::Status CheckNone() const;
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
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from archive
  // and when the path can no longer be opened. The caller must ensure the path
  // still resolves to the same, unmodified file it was opened from: not
  // renamed, replaced or rewritten in between (for a relative path, the
  // working directory must not have changed either). The shared central
  // directory would otherwise describe byte ranges that no longer hold the
  // entries it names.
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
  absl::Status CheckNone() const;
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
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from file-backed
  // archive and when the path can no longer be opened.
  //
  // Not supported for archives opened with FromBuffer, which return
  // `absl::StatusCode::kUnimplemented` (even when moved from), because their
  // reader cannot currently be shared and cloning one would copy the whole
  // archive.
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
// seeking. The contract below applies to all three classes.
//
// Construction: `NewFromData`/`FromBuffer` copy `data` and never fail;
// nothing is validated until the first `ReadNextFile*` call (unlike
// `*ZipArchive`, which parses the central directory up front).
// `NewFromPath`/`FromFile` return kInvalidArgument if `path` cannot be opened.
//
// ReadNextFile(): Reads the next local file header and returns the entry, or
// std::nullopt once the central directory (end of entries) is reached.
// Error codes:
//   - kFailedPrecondition: the reader is moved-from, or the entry was written
//     with general purpose bit 3 (sizes only in the trailing data descriptor),
//     so its compressed size is not available in the local header. Use
//     ReadNextFileWithCompressedSize() for such entries.
//   - kInvalidArgument: corrupt local file header.
//   - kInternal: I/O error.
//
// ReadNextFileWithCompressedSize(compressed_size): Same as ReadNextFile() but
// uses `compressed_size` for entries whose local header carries no size (bit-3
// / data-descriptor entries), e.g. taken from the central directory of a
// previous seekable pass. A wrong hint truncates or over-reads the entry and
// surfaces as a CRC or decoding error (kInternal) when the data is read, after
// which the stream position is undefined.
//
// IMPORTANT: The returned ZipFile borrows the underlying stream (a Rust `&mut`
// that Crubit cannot enforce in C++) and drains any unread compressed bytes
// when destroyed so the stream is positioned at the next local file header.
// Only one entry may be alive per reader at a time: callers MUST destroy the
// returned ZipFile before calling ReadNextFile* again.
//
// IsFinished(): True once ReadNextFile* has returned std::nullopt; subsequent
// calls keep returning std::nullopt. False for a moved-from reader and after a
// read error.
// IsNone(): True if moved-from; ReadNextFile* then return kFailedPrecondition.
class BufferedZipStreamReader final {
 public:
  static absl::StatusOr<BufferedZipStreamReader> NewFromData(
      absl::string_view data);
  // See the contract at the top of this section.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::StatusOr<std::optional<ZipFile>> ReadNextFileWithCompressedSize(
      uint64_t compressed_size) ABSL_ATTRIBUTE_LIFETIME_BOUND;
  [[nodiscard]] bool IsFinished() const;
  [[nodiscard]] bool IsNone() const;

 private:
  absl::Status CheckNone() const;
  rust::BufferedZipStreamReader reader_;
  explicit BufferedZipStreamReader(rust::BufferedZipStreamReader reader)
      : reader_(std::move(reader)) {}
};

class FsZipStreamReader final {
 public:
  static absl::StatusOr<FsZipStreamReader> NewFromPath(absl::string_view path);
  // See the contract at the top of this section.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::StatusOr<std::optional<ZipFile>> ReadNextFileWithCompressedSize(
      uint64_t compressed_size) ABSL_ATTRIBUTE_LIFETIME_BOUND;
  [[nodiscard]] bool IsFinished() const;
  [[nodiscard]] bool IsNone() const;

 private:
  absl::Status CheckNone() const;
  rust::FsZipStreamReader reader_;
  explicit FsZipStreamReader(rust::FsZipStreamReader reader)
      : reader_(std::move(reader)) {}
};

// Facade over BufferedZipStreamReader / FsZipStreamReader; every method
// forwards to the selected backend.
class ZipStreamReader final {
 public:
  static absl::StatusOr<ZipStreamReader> FromFile(absl::string_view path);
  static absl::StatusOr<ZipStreamReader> FromBuffer(absl::string_view data);
  // See the contract at the top of this section.
  absl::StatusOr<std::optional<ZipFile>> ReadNextFile()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
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
