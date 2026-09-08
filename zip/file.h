#ifndef SECURITY_ZIP_FILE_H_
#define SECURITY_ZIP_FILE_H_

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <utility>
#include <variant>

#include "converters.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"

namespace security::zip {

struct ZipFileWriterPeer;

namespace internal {

// Internal to security/zip; not part of the public API.
//
// Move-only RAII token recording that an entry borrowed from an archive or
// stream-reader handle is alive. Each handle owns a `std::shared_ptr<bool>`
// flag; constructing a guard from it sets the flag to true and destroying the
// guard (or move-assigning over an engaged one) resets it to false. Moved-from
// guards do nothing. The handle refuses to open another entry while its flag
// is set, which keeps the Rust `&mut` borrow held by the entry exclusive even
// though Crubit cannot express that borrow in C++.
class ActiveEntryGuard final {
 public:
  ActiveEntryGuard() = default;
  explicit ActiveEntryGuard(std::shared_ptr<bool> flag)
      : flag_(std::move(flag)) {
    if (flag_ != nullptr) *flag_ = true;
  }
  ActiveEntryGuard(ActiveEntryGuard&& other) noexcept = default;
  ActiveEntryGuard& operator=(ActiveEntryGuard&& other) noexcept {
    if (this != &other) {
      Release();
      flag_ = std::move(other.flag_);
    }
    return *this;
  }
  ActiveEntryGuard(const ActiveEntryGuard&) = delete;
  ActiveEntryGuard& operator=(const ActiveEntryGuard&) = delete;
  ~ActiveEntryGuard() { Release(); }

 private:
  void Release() {
    if (flag_ != nullptr) {
      *flag_ = false;
      flag_.reset();
    }
  }

  std::shared_ptr<bool> flag_;
};

}  // namespace internal

enum class CompressionMethod : int32_t {
  kStored,
  kDeflated,
  kBzip2,
  kZstd,
  kLzma,
  kXz,
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

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. With
  // `max_bytes > 0`, callers should loop until an empty result is returned
  // rather than assuming a short non-empty read means end of entry
  // (`ReadBytes(0)` returns an empty buffer immediately without advancing the
  // entry). Returns `absl::StatusCode::kInternal` on a moved-from entry or
  // decompression/CRC errors, or propagates underlying I/O error codes.
  //
  // WARNING: Chunks returned before the terminating empty read are not yet
  // verified against the entry's CRC-32: zip-rs checks it only on the read that
  // observes end of entry. In particular, when the uncompressed size is an
  // exact multiple of `max_bytes`, the chunk carrying the last byte is still
  // unverified; always read until an empty result (or an error) before acting
  // on the data.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  friend struct ZipFileWriterPeer;

  absl::Status CheckNone() const;
  rust::BufferedZipFile zip_;
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

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. With
  // `max_bytes > 0`, callers should loop until an empty result is returned
  // rather than assuming a short non-empty read means end of entry
  // (`ReadBytes(0)` returns an empty buffer immediately without advancing the
  // entry). Returns `absl::StatusCode::kInternal` on a moved-from entry or
  // decompression/CRC errors, or propagates underlying I/O error codes.
  //
  // WARNING: Chunks returned before the terminating empty read are not yet
  // verified against the entry's CRC-32: zip-rs checks it only on the read that
  // observes end of entry. In particular, when the uncompressed size is an
  // exact multiple of `max_bytes`, the chunk carrying the last byte is still
  // unverified; always read until an empty result (or an error) before acting
  // on the data.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  friend struct ZipFileWriterPeer;

  absl::Status CheckNone() const;
  rust::FsZipFile zip_;
};

class ZipFile final {
 public:
  // `guard` is internal to security/zip: it is the token that marks the
  // originating archive or stream reader as having an entry open and is
  // released when this ZipFile is destroyed (see internal::ActiveEntryGuard).
  static ZipFile FromFile(rust::FsZipFile zip,
                          internal::ActiveEntryGuard guard) {
    return ZipFile(FsZipFile(std::move(zip)), std::move(guard));
  }

  static ZipFile FromBuffer(rust::BufferedZipFile zip,
                            internal::ActiveEntryGuard guard) {
    return ZipFile(BufferedZipFile(std::move(zip)), std::move(guard));
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

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks, or read the entry
  // incrementally with `ReadBytes()`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Reads up to `max_bytes` decompressed bytes from the entry, advancing the
  // read position. This is the chunked, memory-bounded alternative to
  // `GetFileData()`: at most `max_bytes` are buffered per call. With
  // `max_bytes > 0`, callers should loop until an empty result is returned
  // rather than assuming a short non-empty read means end of entry
  // (`ReadBytes(0)` returns an empty buffer immediately without advancing the
  // entry). Returns `absl::StatusCode::kInternal` on a moved-from entry or
  // decompression/CRC errors, or propagates underlying I/O error codes.
  //
  // WARNING: Chunks returned before the terminating empty read are not yet
  // verified against the entry's CRC-32: zip-rs checks it only on the read that
  // observes end of entry. In particular, when the uncompressed size is an
  // exact multiple of `max_bytes`, the chunk carrying the last byte is still
  // unverified; always read until an empty result (or an error) before acting
  // on the data.
  absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  using BackendType = std::variant<BufferedZipFile, FsZipFile>;
  friend struct ZipFileWriterPeer;

  ZipFile(BackendType b, internal::ActiveEntryGuard guard)
      : guard_(std::move(guard)), zip_(std::move(b)) {}

  // `guard_` is declared before `zip_` so that the Rust entry (which drains
  // the stream on destruction) is destroyed before the originating handle is
  // marked free again.
  internal::ActiveEntryGuard guard_;
  BackendType zip_;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_FILE_H_
