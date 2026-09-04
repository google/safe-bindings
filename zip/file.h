#ifndef SECURITY_ZIP_FILE_H_
#define SECURITY_ZIP_FILE_H_

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
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]).
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]).
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  absl::StatusOr<uint32_t> GetUnixMode() const;
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

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
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]).
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]).
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  absl::StatusOr<uint32_t> GetUnixMode() const;
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

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
  absl::StatusOr<std::string> GetFileName() const;
  absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  absl::StatusOr<std::string> GetComment() const;
  absl::StatusOr<uint64_t> GetUncompressedSize() const;
  absl::StatusOr<uint64_t> GetCompressedSize() const;
  absl::StatusOr<uint32_t> GetCrc32() const;
  // Returns the last-modified date encoded as an MS-DOS FAT date bitfield
  // (bits 15-9: year offset from 1980, bits 8-5: month [1-12], bits 4-0: day
  // [1-31]).
  absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  // Returns the last-modified time encoded as an MS-DOS FAT time bitfield
  // (bits 15-11: hour [0-23], bits 10-5: minute [0-59], bits 4-0: second / 2
  // [0-29]).
  absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  absl::StatusOr<uint32_t> GetUnixMode() const;
  absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  absl::StatusOr<RustVecU8Wrapper> GetFileData();

 private:
  using BackendType = std::variant<BufferedZipFile, FsZipFile>;
  BackendType zip_;
  explicit ZipFile(BackendType b) : zip_(std::move(b)) {}
  friend class BufferedZipWriter;
  friend class FsZipWriter;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_FILE_H_
