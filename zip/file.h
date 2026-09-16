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
class BufferedZipStreamWriter;
class FsZipStreamWriter;
class ZipStreamWriter;

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
  [[nodiscard]] absl::StatusOr<bool> IsFile() const;
  [[nodiscard]] absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;
  [[nodiscard]] absl::StatusOr<std::string> GetFileName() const;
  [[nodiscard]] absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  [[nodiscard]] absl::StatusOr<std::string> GetComment() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetUncompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetCompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetCrc32() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetUnixMode() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetFileData();
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

 private:
  absl::Status CheckNone() const;
  rust::BufferedZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
  friend class ZipStreamWriter;
};

class FsZipFile final {
 public:
  explicit FsZipFile(rust::FsZipFile zip) : zip_(std::move(zip)) {}
  [[nodiscard]] absl::StatusOr<bool> IsFile() const;
  [[nodiscard]] absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;
  [[nodiscard]] absl::StatusOr<std::string> GetFileName() const;
  [[nodiscard]] absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  [[nodiscard]] absl::StatusOr<std::string> GetComment() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetUncompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetCompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetCrc32() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetUnixMode() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetFileData();
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

 private:
  absl::Status CheckNone() const;
  rust::FsZipFile zip_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
  friend class ZipStreamWriter;
};

class ZipFile final {
 public:
  static ZipFile FromFile(rust::FsZipFile zip) {
    return ZipFile(FsZipFile(std::move(zip)));
  }

  static ZipFile FromBuffer(rust::BufferedZipFile zip) {
    return ZipFile(BufferedZipFile(std::move(zip)));
  }

  [[nodiscard]] absl::StatusOr<bool> IsFile() const;
  [[nodiscard]] absl::StatusOr<bool> IsDir() const;
  [[nodiscard]] bool IsNone() const;
  [[nodiscard]] absl::StatusOr<std::string> GetFileName() const;
  [[nodiscard]] absl::StatusOr<CompressionMethod> GetCompressionMethod() const;
  [[nodiscard]] absl::StatusOr<std::string> GetComment() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetUncompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint64_t> GetCompressedSize() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetCrc32() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedDate() const;
  [[nodiscard]] absl::StatusOr<uint16_t> GetLastModifiedTime() const;
  [[nodiscard]] absl::StatusOr<uint32_t> GetUnixMode() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetExtraData() const;
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> GetFileData();
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> ReadBytes(size_t max_bytes);

 private:
  using BackendType = std::variant<BufferedZipFile, FsZipFile>;
  BackendType zip_;
  explicit ZipFile(BackendType b) : zip_(std::move(b)) {}
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
  friend class ZipStreamWriter;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_FILE_H_
