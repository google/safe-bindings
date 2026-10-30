#ifndef SECURITY_ZIP_FILE_H_
#define SECURITY_ZIP_FILE_H_

#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <variant>

#include "converters.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"

namespace security::zip {

class BufferedZipWriter;
class FsZipWriter;
class BufferedZipStreamWriter;
class FsZipStreamWriter;

enum class CompressionMethod : int32_t {
  kStored,
  kDeflated,
  kBzip2,
  kZstd,
  kLzma,
  kXz,
};

// What an archive's central directory records about one entry.
//
// Obtaining this reads nothing: the central directory is parsed when the
// archive is opened. Contrast a ZipFile, which additionally reads the entry's
// local file header to find where its data begins, so that listing an archive
// entry by entry costs one read apiece. Prefer this whenever the entry's
// contents are not wanted; over a network the difference is one round trip
// against one per entry.
//
// This is a snapshot and does not borrow the archive, so it stays valid after
// the archive is destroyed and several may be held at once.
//
// Correspondingly there is no GetDataStart(): where an entry's data begins is
// the one thing the central directory does not record. Open the entry for it.
// Nor is there GetExtraData(): the zip crate exposes an entry's extra fields
// only once the entry is opened, so open it (ZipFile::GetExtraData()) for them.
//
// A moved-from ZipEntryMetadata describes no entry: its name and comment are
// empty, every numeric accessor returns 0, IsFile()/IsDir()/IsEncrypted()
// return false and GetCompressionMethod() returns an error.
class ZipEntryMetadata final {
 public:
  explicit ZipEntryMetadata(rust::ZipEntryMetadata metadata)
      : metadata_(std::move(metadata)) {}

  // Returns whether the entry is a file, that is, neither a directory nor a
  // symbolic link.
  [[nodiscard]] bool IsFile() const;
  // Returns whether the entry is a directory.
  [[nodiscard]] bool IsDir() const;
  // Returns whether the entry's data is encrypted. This library does not
  // decrypt, so such an entry can be listed but not read.
  [[nodiscard]] bool IsEncrypted() const;
  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  [[nodiscard]] std::string GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  [[nodiscard]] std::string GetComment() const;
  // WARNING: Untrusted Header Value: Returns the uncompressed size declared in
  // the ZIP header without verifying it against the actual decompressed
  // payload. Crafty or corrupted archives (including zip bombs) may spoof this
  // value; do not trust it for unbounded memory pre-allocation.
  [[nodiscard]] uint64_t GetUncompressedSize() const;
  [[nodiscard]] uint64_t GetCompressedSize() const;
  // WARNING: Untrusted Header Value: Returns the raw CRC-32 checksum declared
  // in the ZIP header. The checksum is only verified when the entry payload is
  // actually read and decompressed.
  [[nodiscard]] uint32_t GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]), or 0 if the entry records none.
  [[nodiscard]] uint16_t GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]), or 0 if the entry records none.
  [[nodiscard]] uint16_t GetLastModifiedTime() const;
  // Returns the full POSIX `st_mode` bitmask (including `S_IFMT` file-type bits
  // such as `0100644` for regular files), or 0 if Unix mode metadata is absent
  // (e.g. non-Unix archives).
  [[nodiscard]] uint32_t GetUnixMode() const;
  // Returns the offset of this entry's local file header within the archive.
  // Unlike ZipFile::GetHeaderStart() this is always meaningful, since a
  // ZipEntryMetadata only ever comes from an archive's central directory.
  [[nodiscard]] uint64_t GetHeaderStart() const;

 private:
  rust::ZipEntryMetadata metadata_;
};

class BufferedZipFile final {
 public:
  explicit BufferedZipFile(rust::BufferedZipFile zip)
      : zip_(std::move(zip)) {}
  absl::StatusOr<bool> IsFile() const;
  absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;
  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  // WARNING: Untrusted Header Value: Returns the uncompressed size declared in
  // the ZIP header without verifying it against the actual decompressed
  // payload. Crafty or corrupted archives (including zip bombs) may spoof this
  // value; do not trust it for unbounded memory pre-allocation.
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  // WARNING: Untrusted Header Value: Returns the raw CRC-32 checksum declared
  // in the ZIP header. The checksum is only verified when the entry payload is
  // actually read and decompressed.
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  // Returns the full POSIX `st_mode` bitmask (including `S_IFMT` file-type bits
  // such as `0100644` for regular files), or 0 if Unix mode metadata is absent
  // (e.g. non-Unix archives).
  absl::StatusOr<uint32_t> GetUnixMode() const;
  // Returns the entry's extra fields re-serialized by zip-rs (note that zip-rs
  // omits ExtendedTimestamp (0x5455) and Ntfs (0x000a) extra fields when
  // re-serializing `extra_data()`).
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  // Returns the offset of this entry's local file header within the archive.
  // Only meaningful for entries opened from a *ZipArchive: entries produced by
  // a *ZipStreamReader do not know their offset and always report 0, which is
  // indistinguishable from the genuine offset of an archive's first entry.
  absl::StatusOr<uint64_t> GetHeaderStart() const;
  // Returns the offset of this entry's data within the archive, or 0 if it is
  // unknown. Together with GetCompressedSize() this delimits the entry's raw
  // bytes, so a caller that only needs a kStored entry can read that byte range
  // directly rather than through this object. The offset is only known once the
  // local file header has been parsed, which happens when an archive opens the
  // entry; entries produced by a *ZipStreamReader always report 0.
  absl::StatusOr<uint64_t> GetDataStart() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. The result may
  // be shorter than `max_bytes`; a short but non-empty result does NOT mean
  // the entry is exhausted. Only an empty result signals end of entry.
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);
  // Reads into `buf` (decompressing unless the entry was opened raw) and
  // returns how many bytes were written, which is less than `buf.size()` only
  // at end of entry.
  //
  // Unlike ReadBytes(), nothing is allocated and the bytes are not copied
  // again on the way out, so this is what a caller reading an entry of any
  // size should use.
  //
  // zip-rs verifies the CRC only on the read that reaches the end of the
  // entry, so reading exactly the uncompressed size never checks it; read
  // until a short count (or 0) to have the entry validated.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<size_t> ReadInto(absl::Span<char> buf);
  // Discards up to `max_bytes` and returns how many were discarded, which is
  // less than `max_bytes` only at end of entry.
  //
  // A zip entry is a forward-only stream, so this is how a caller seeks
  // within one. Nothing is allocated, whatever the distance.
  //
  // As with ReadInto(), the CRC is checked only by the read or skip that
  // reaches the end of the entry, so skipping exactly the uncompressed size
  // never checks it.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<uint64_t> Skip(uint64_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  absl::Status CheckNone() const;
  rust::BufferedZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
};

class FsZipFile final {
 public:
  explicit FsZipFile(rust::FsZipFile zip) : zip_(std::move(zip)) {}
  absl::StatusOr<bool> IsFile() const;
  absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;

  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  // WARNING: Untrusted Header Value: Returns the uncompressed size declared in
  // the ZIP header without verifying it against the actual decompressed
  // payload. Crafty or corrupted archives (including zip bombs) may spoof this
  // value; do not trust it for unbounded memory pre-allocation.
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  // WARNING: Untrusted Header Value: Returns the raw CRC-32 checksum declared
  // in the ZIP header. The checksum is only verified when the entry payload is
  // actually read and decompressed.
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  // Returns the full POSIX `st_mode` bitmask (including `S_IFMT` file-type bits
  // such as `0100644` for regular files), or 0 if Unix mode metadata is absent
  // (e.g. non-Unix archives).
  absl::StatusOr<uint32_t> GetUnixMode() const;
  // Returns the entry's extra fields re-serialized by zip-rs (note that zip-rs
  // omits ExtendedTimestamp (0x5455) and Ntfs (0x000a) extra fields when
  // re-serializing `extra_data()`).
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  // Returns the offset of this entry's local file header within the archive.
  // Only meaningful for entries opened from a *ZipArchive: entries produced by
  // a *ZipStreamReader do not know their offset and always report 0, which is
  // indistinguishable from the genuine offset of an archive's first entry.
  absl::StatusOr<uint64_t> GetHeaderStart() const;
  // Returns the offset of this entry's data within the archive, or 0 if it is
  // unknown. Together with GetCompressedSize() this delimits the entry's raw
  // bytes, so a caller that only needs a kStored entry can read that byte range
  // directly rather than through this object. The offset is only known once the
  // local file header has been parsed, which happens when an archive opens the
  // entry; entries produced by a *ZipStreamReader always report 0.
  absl::StatusOr<uint64_t> GetDataStart() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. The result may
  // be shorter than `max_bytes`; a short but non-empty result does NOT mean
  // the entry is exhausted. Only an empty result signals end of entry.
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);
  // Reads into `buf` (decompressing unless the entry was opened raw) and
  // returns how many bytes were written, which is less than `buf.size()` only
  // at end of entry.
  //
  // Unlike ReadBytes(), nothing is allocated and the bytes are not copied
  // again on the way out, so this is what a caller reading an entry of any
  // size should use.
  //
  // zip-rs verifies the CRC only on the read that reaches the end of the
  // entry, so reading exactly the uncompressed size never checks it; read
  // until a short count (or 0) to have the entry validated.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<size_t> ReadInto(absl::Span<char> buf);
  // Discards up to `max_bytes` and returns how many were discarded, which is
  // less than `max_bytes` only at end of entry.
  //
  // A zip entry is a forward-only stream, so this is how a caller seeks
  // within one. Nothing is allocated, whatever the distance.
  //
  // As with ReadInto(), the CRC is checked only by the read or skip that
  // reaches the end of the entry, so skipping exactly the uncompressed size
  // never checks it.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<uint64_t> Skip(uint64_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  absl::Status CheckNone() const;
  rust::FsZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
};

class ZipFile final {
 public:
  static ZipFile FromFile(rust::FsZipFile zip) {
    return ZipFile(FsZipFile(std::move(zip)));
  }

  static ZipFile FromBuffer(rust::BufferedZipFile zip) {
    return ZipFile(BufferedZipFile(std::move(zip)));
  }

  absl::StatusOr<bool> IsFile() const;
  absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;

  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  // WARNING: Untrusted Header Value: Returns the uncompressed size declared in
  // the ZIP header without verifying it against the actual decompressed
  // payload. Crafty or corrupted archives (including zip bombs) may spoof this
  // value; do not trust it for unbounded memory pre-allocation.
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  // WARNING: Untrusted Header Value: Returns the raw CRC-32 checksum declared
  // in the ZIP header. The checksum is only verified when the entry payload is
  // actually read and decompressed.
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]), or 0 if no valid modification timestamp is present in the header.
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  // Returns the full POSIX `st_mode` bitmask (including `S_IFMT` file-type bits
  // such as `0100644` for regular files), or 0 if Unix mode metadata is absent
  // (e.g. non-Unix archives).
  absl::StatusOr<uint32_t> GetUnixMode() const;
  // Returns the entry's extra fields re-serialized by zip-rs (note that zip-rs
  // omits ExtendedTimestamp (0x5455) and Ntfs (0x000a) extra fields when
  // re-serializing `extra_data()`).
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  // Returns the offset of this entry's local file header within the archive.
  // Only meaningful for entries opened from a *ZipArchive: entries produced by
  // a *ZipStreamReader do not know their offset and always report 0, which is
  // indistinguishable from the genuine offset of an archive's first entry.
  absl::StatusOr<uint64_t> GetHeaderStart() const;
  // Returns the offset of this entry's data within the archive, or 0 if it is
  // unknown. Together with GetCompressedSize() this delimits the entry's raw
  // bytes, so a caller that only needs a kStored entry can read that byte range
  // directly rather than through this object. The offset is only known once the
  // local file header has been parsed, which happens when an archive opens the
  // entry; entries produced by a *ZipStreamReader always report 0.
  absl::StatusOr<uint64_t> GetDataStart() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. The result may
  // be shorter than `max_bytes`; a short but non-empty result does NOT mean
  // the entry is exhausted. Only an empty result signals end of entry.
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);
  // Reads into `buf` (decompressing unless the entry was opened raw) and
  // returns how many bytes were written, which is less than `buf.size()` only
  // at end of entry.
  //
  // Unlike ReadBytes(), nothing is allocated and the bytes are not copied
  // again on the way out, so this is what a caller reading an entry of any
  // size should use.
  //
  // zip-rs verifies the CRC only on the read that reaches the end of the
  // entry, so reading exactly the uncompressed size never checks it; read
  // until a short count (or 0) to have the entry validated.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<size_t> ReadInto(absl::Span<char> buf);
  // Discards up to `max_bytes` and returns how many were discarded, which is
  // less than `max_bytes` only at end of entry.
  //
  // A zip entry is a forward-only stream, so this is how a caller seeks
  // within one. Nothing is allocated, whatever the distance.
  //
  // As with ReadInto(), the CRC is checked only by the read or skip that
  // reaches the end of the entry, so skipping exactly the uncompressed size
  // never checks it.
  //
  // Returns `absl::StatusCode::kFailedPrecondition` on a moved-from entry and
  // `absl::StatusCode::kInternal` on decompression/CRC/I/O errors.
  absl::StatusOr<uint64_t> Skip(uint64_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  using BackendType = std::variant<BufferedZipFile, FsZipFile>;
  BackendType zip_;
  explicit ZipFile(BackendType b) : zip_(std::move(b)) {}
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_FILE_H_
