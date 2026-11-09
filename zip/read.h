#ifndef SECURITY_ZIP_READ_H_
#define SECURITY_ZIP_READ_H_

#include <cstdint>
#include <memory>
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

// Seekable archives. The contract below applies to all three classes.
//
// GetFileByIndex(index) / GetFileByIndexRaw(index): Return the entry at
// `index`; the former decompresses when read, the latter returns the stored
// bytes as-is. Only one entry may be open per archive handle at a time: the
// returned ZipFile borrows the archive (a Rust `&mut` that Crubit cannot
// enforce in C++), and while it is alive any further `GetFileByIndex*` call on
// the same handle returns `kFailedPrecondition`. Destroy (or `reset()`) the
// previous ZipFile first, or scope each entry in its own block. To read
// several entries at once, open an independent handle on the same archive
// instead of sharing one. Note that `entry = archive.GetFileByIndex(i)` on an
// existing `absl::StatusOr<ZipFile>` evaluates the call before the old entry
// is destroyed, so it fails while `entry` still holds one.
class BufferedZipArchive final {
 public:
  // Parses `data` as a zip archive. Returns `kInvalidArgument` if `data` is not
  // a valid zip archive, or `kFailedPrecondition` if the archive uses an
  // unsupported feature (such as multi-disk spanning).
  static absl::StatusOr<BufferedZipArchive> NewFromData(absl::string_view data);
  // Returns the number of entries in the archive, or `kFailedPrecondition` if
  // the archive is moved-from.
  absl::StatusOr<uintptr_t> GetLength() const;
  // Returns the archive comment as the raw bytes stored in the central
  // directory, without UTF-8 validation. Returns `kFailedPrecondition` if the
  // archive is moved-from.
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read. Returns `kOutOfRange` if `index >= GetLength()`, `kInvalidArgument`
  // if the entry's local header is corrupt, `kFailedPrecondition` if the
  // archive is moved-from, another entry of this archive is still open, or the
  // entry is encrypted, and `kUnimplemented` if the entry uses an unsupported
  // compression method.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read. Returns `kOutOfRange` if
  // `index >= GetLength()`, `kInvalidArgument` if the entry's local header is
  // corrupt, and `kFailedPrecondition` if the archive is moved-from or another
  // entry of this archive is still open.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // True if moved-from; the accessors above then return `kFailedPrecondition`.
  [[nodiscard]] bool IsNone() const;

  BufferedZipArchive(BufferedZipArchive&&) noexcept = default;
  // Drops the previous Rust handle and leaves `other` empty (`IsNone()`); see
  // internal::MoveAssignRustHandle in file.h.
  BufferedZipArchive& operator=(BufferedZipArchive&& other) noexcept {
    if (this != &other) {
      internal::MoveAssignRustHandle(archive_, std::move(other.archive_));
      active_entry_ = std::move(other.active_entry_);
    }
    return *this;
  }

 private:
  explicit BufferedZipArchive(rust::BufferedZipArchive archive)
      : archive_(std::move(archive)),
        active_entry_(std::make_shared<bool>(false)) {}

  rust::BufferedZipArchive archive_;
  // Set while a ZipFile returned by GetFileByIndex* is alive; see
  // internal::ActiveEntryGuard. Null on a moved-from archive.
  std::shared_ptr<bool> active_entry_;
};

class FsZipArchive final {
 public:
  // Opens `path` and parses its central directory. Propagates OS file-open
  // errors (such as `kNotFound` and `kPermissionDenied`), returns
  // `kInvalidArgument` if `path` is empty, contains NUL bytes, is not valid
  // UTF-8, or is not a valid zip archive, and returns `kFailedPrecondition` if
  // the archive uses an unsupported feature (such as multi-disk spanning).
  static absl::StatusOr<FsZipArchive> NewFromPath(absl::string_view path);
  // Returns the number of entries in the archive, or `kFailedPrecondition` if
  // the archive is moved-from.
  absl::StatusOr<uintptr_t> GetLength() const;
  // Returns the archive comment as the raw bytes stored in the central
  // directory, without UTF-8 validation. Returns `kFailedPrecondition` if the
  // archive is moved-from.
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read. Returns `kOutOfRange` if `index >= GetLength()`, `kInvalidArgument`
  // if the entry's local header is corrupt, `kFailedPrecondition` if the
  // archive is moved-from, another entry of this archive is still open, or the
  // entry is encrypted, `kUnimplemented` if the entry uses an unsupported
  // compression method, or propagates underlying I/O errors.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read. Returns `kOutOfRange` if
  // `index >= GetLength()`, `kInvalidArgument` if the entry's local header is
  // corrupt, `kFailedPrecondition` if the archive is moved-from or another
  // entry of this archive is still open, or propagates underlying I/O errors.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // True if moved-from; the accessors above then return `kFailedPrecondition`.
  [[nodiscard]] bool IsNone() const;

  FsZipArchive(FsZipArchive&&) noexcept = default;
  // Drops the previous Rust handle and leaves `other` empty (`IsNone()`); see
  // internal::MoveAssignRustHandle in file.h.
  FsZipArchive& operator=(FsZipArchive&& other) noexcept {
    if (this != &other) {
      internal::MoveAssignRustHandle(archive_, std::move(other.archive_));
      active_entry_ = std::move(other.active_entry_);
    }
    return *this;
  }

 private:
  explicit FsZipArchive(rust::FsZipArchive archive)
      : archive_(std::move(archive)),
        active_entry_(std::make_shared<bool>(false)) {}

  rust::FsZipArchive archive_;
  // Set while a ZipFile returned by GetFileByIndex* is alive; see
  // internal::ActiveEntryGuard. Null on a moved-from archive.
  std::shared_ptr<bool> active_entry_;
};

class ZipArchive final {
 public:
  // Opens `path` as a seekable zip archive; see `FsZipArchive::NewFromPath` for
  // error codes.
  static absl::StatusOr<ZipArchive> FromFile(absl::string_view path);
  // Parses `data` as a seekable zip archive; see
  // `BufferedZipArchive::NewFromData` for error codes.
  static absl::StatusOr<ZipArchive> FromBuffer(absl::string_view data);
  // Returns the number of entries in the archive, or `kFailedPrecondition` if
  // the archive is moved-from.
  absl::StatusOr<uintptr_t> GetLength() const;
  // Returns the archive comment as the raw bytes stored in the central
  // directory, without UTF-8 validation. Returns `kFailedPrecondition` if the
  // archive is moved-from.
  absl::StatusOr<std::string> GetComment() const;
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read. Returns `kOutOfRange` if `index >= GetLength()`, `kInvalidArgument`
  // if the entry's local header is corrupt, `kFailedPrecondition` if the
  // archive is moved-from, another entry of this archive is still open, or the
  // entry is encrypted, `kUnimplemented` if the entry uses an unsupported
  // compression method, or propagates underlying I/O errors.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read. Returns `kOutOfRange` if
  // `index >= GetLength()`, `kInvalidArgument` if the entry's local header is
  // corrupt, `kFailedPrecondition` if the archive is moved-from or another
  // entry of this archive is still open, or propagates underlying I/O errors.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index)
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  // True if moved-from; the accessors above then return `kFailedPrecondition`.
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipArchive, FsZipArchive>;

  explicit ZipArchive(BackendType archive) : archive_(std::move(archive)) {}

  BackendType archive_;
};

// Streaming readers that read sequential zip entries from a stream without
// seeking. The contract below applies to all three classes.
//
// Construction: `NewFromData`/`FromBuffer` copy `data` and never fail;
// nothing is validated until the first `ReadNextFile*` call (unlike
// `*ZipArchive`, which parses the central directory up front).
// `NewFromPath`/`FromFile` propagate OS file-open errors (such as `kNotFound`
// and `kPermissionDenied`) and return `kInvalidArgument` if `path` is empty,
// contains NUL bytes, or is not valid UTF-8.
//
// ReadNextFile(): Reads the next local file header and returns the entry, or
// std::nullopt once the central directory (end of entries) is reached.
// Error codes:
//   - kFailedPrecondition: the reader is moved-from, another entry of this
//     reader is still open (see IMPORTANT below), an earlier ReadNextFile*
//     call on this reader failed (see "Errors are sticky" below), the entry is
//     encrypted, or the entry was written with general purpose bit 3 (sizes in
//     a trailing data descriptor rather than the local header). Standard
//     streaming-writer output, which also zeroes the local-header CRC-32 and
//     appends a data descriptor, must be read with seekable `ZipArchive`; see
//     `ReadNextFileWithCompressedSize()` for the narrow bit-3 case that can
//     still be streamed.
//   - kUnimplemented: the entry uses an unsupported compression method.
//   - kInvalidArgument: corrupt local file header.
//   - kInternal (or propagated OS canonical codes such as kPermissionDenied):
//     stream I/O error. This includes premature EOF (`UnexpectedEof`): an
//     empty stream, or a stream truncated inside the fixed-size local file
//     header, is reported as kInternal rather than as end of archive.
//
// Errors are sticky: once ReadNextFile* has failed, the stream is positioned
// somewhere inside an entry that was never opened (zip-rs consumes the local
// header before it rejects the entry), so every later ReadNextFile* call on
// that reader returns kFailedPrecondition instead of parsing untrusted payload
// bytes as the next header. IsFinished() stays false and IsNone() stays false
// in that state; open a new reader to start over.
//
// ReadNextFileWithCompressedSize(compressed_size): Same as ReadNextFile() but
// supplies `compressed_size` out of band (e.g. from an earlier central-
// directory scan) when the local header sets bit 3. Because zip-rs verifies
// the payload against the local header's CRC-32 and drains only
// `compressed_size` bytes when the returned `ZipFile` is destroyed (without
// consuming a trailing data descriptor), this only works when the local header
// already carries the real CRC-32 and either no data descriptor follows or no
// further entries are read from the stream. Standard streaming-writer output,
// which also zeroes the local-header CRC-32 and appends a data descriptor,
// must be read with seekable `ZipArchive`. A wrong
// `compressed_size` truncates or over-reads the entry and surfaces as a CRC or
// decoding error (kInternal) when the data is read, after which the stream
// position is undefined.
//
// IMPORTANT: The returned ZipFile borrows the underlying stream (a Rust `&mut`
// that Crubit cannot enforce in C++) and drains any unread compressed bytes
// when destroyed so the stream is positioned at the next local file header.
// Only one entry may be open per reader at a time: while a ZipFile returned by
// this reader is alive, ReadNextFile* returns kFailedPrecondition. Destroy the
// previous ZipFile first, or scope each entry in its own block. Beware that
//     std::optional<ZipFile> entry = reader.ReadNextFile();   // first entry
//     entry = reader.ReadNextFile();  // kFailedPrecondition: old entry alive
// evaluates the call before the old entry is destroyed; call `entry.reset()`
// before reading the next one. To read several entries at once, open an
// independent handle (e.g. a second reader over the same data) rather than
// sharing one.
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

  BufferedZipStreamReader(BufferedZipStreamReader&&) noexcept = default;
  // Drops the previous Rust handle and leaves `other` empty (`IsNone()`); see
  // internal::MoveAssignRustHandle in file.h.
  BufferedZipStreamReader& operator=(BufferedZipStreamReader&& other) noexcept {
    if (this != &other) {
      internal::MoveAssignRustHandle(reader_, std::move(other.reader_));
      active_entry_ = std::move(other.active_entry_);
    }
    return *this;
  }

 private:
  explicit BufferedZipStreamReader(rust::BufferedZipStreamReader reader)
      : reader_(std::move(reader)),
        active_entry_(std::make_shared<bool>(false)) {}

  rust::BufferedZipStreamReader reader_;
  // Set while a ZipFile returned by ReadNextFile* is alive; see
  // internal::ActiveEntryGuard. Null on a moved-from reader.
  std::shared_ptr<bool> active_entry_;
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

  FsZipStreamReader(FsZipStreamReader&&) noexcept = default;
  // Drops the previous Rust handle and leaves `other` empty (`IsNone()`); see
  // internal::MoveAssignRustHandle in file.h.
  FsZipStreamReader& operator=(FsZipStreamReader&& other) noexcept {
    if (this != &other) {
      internal::MoveAssignRustHandle(reader_, std::move(other.reader_));
      active_entry_ = std::move(other.active_entry_);
    }
    return *this;
  }

 private:
  explicit FsZipStreamReader(rust::FsZipStreamReader reader)
      : reader_(std::move(reader)),
        active_entry_(std::make_shared<bool>(false)) {}

  rust::FsZipStreamReader reader_;
  // Set while a ZipFile returned by ReadNextFile* is alive; see
  // internal::ActiveEntryGuard. Null on a moved-from reader.
  std::shared_ptr<bool> active_entry_;
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

  explicit ZipStreamReader(BackendType reader) : reader_(std::move(reader)) {}

  BackendType reader_;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_READ_H_
