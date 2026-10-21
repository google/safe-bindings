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

namespace security::zip {

class BufferedZipWriter;
class FsZipWriter;

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
  bool IsNone() const;
  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  absl::Status CheckNone() const;
  rust::BufferedZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
};

class FsZipFile final {
 public:
  explicit FsZipFile(rust::FsZipFile zip) : zip_(std::move(zip)) {}
  absl::StatusOr<bool> IsFile() const;
  absl::StatusOr<bool> IsDir() const;
  bool IsNone() const;

  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

  // Decompresses at most `max_size` bytes from the entry, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed size exceeds
  // `max_size`.
  absl::StatusOr<RustVecU8Wrapper> GetFileData(size_t max_size);

 private:
  absl::Status CheckNone() const;
  rust::FsZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
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
  bool IsNone() const;

  // WARNING: Path Traversal ("Zip Slip") Hazard: The returned file name is
  // extracted directly from the archive header without sanitization. If using
  // this path for filesystem operations, callers MUST sanitize and validate
  // that the resolved path does not traverse outside the target destination
  // directory (e.g., rejecting paths with ".." components or leading directory
  // separators).
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;

  // WARNING: Decompression Bomb Hazard: This overload decompresses the entire
  // entry into an unbounded heap buffer. For untrusted zip archives, prefer
  // `GetFileData(size_t max_size)` below to cap memory allocation and defend
  // against zip bomb memory exhaustion (OOM) attacks.
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

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
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_FILE_H_
