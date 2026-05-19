#ifndef SECURITY_ZIP_READ_H_
#define SECURITY_ZIP_READ_H_

#include <cstdint>
#include <utility>
#include <variant>

#include "file.h"
#include "rust/zip_wrapper.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

class BufferedZipArchive final {
 public:
  static absl::StatusOr<BufferedZipArchive> NewFromData(absl::string_view data);
  absl::StatusOr<uintptr_t> GetLength();
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index);
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index);

 private:
  zip_wrapper::BufferedZipArchive archive_;
  explicit BufferedZipArchive(zip_wrapper::BufferedZipArchive archive)
      : archive_(std::move(archive)) {}
};

class FsZipArchive final {
 public:
  static absl::StatusOr<FsZipArchive> NewFromPath(absl::string_view path);
  absl::StatusOr<uintptr_t> GetLength();
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index);
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index);

 private:
  zip_wrapper::FsZipArchive archive_;
  explicit FsZipArchive(zip_wrapper::FsZipArchive archive)
      : archive_(std::move(archive)) {}
};

class ZipArchive final {
 public:
  static absl::StatusOr<ZipArchive> FromFile(absl::string_view path);
  static absl::StatusOr<ZipArchive> FromBuffer(std::string_view data);
  absl::StatusOr<uintptr_t> GetLength();
  // Returns a ZipFile by index. The result file returns decompressed data when
  // read.
  absl::StatusOr<ZipFile> GetFileByIndex(uintptr_t index);
  // Returns a ZipFile by index. The result file returns compressed data as-is
  // without decompressing when read.
  // Warning: Writers in zip-rs do not support writing compressed data as-is.
  // This data will be compressed again when written through a `*ZipWriter`.
  absl::StatusOr<ZipFile> GetFileByIndexRaw(uintptr_t index);

 private:
  using BackendType = std::variant<BufferedZipArchive, FsZipArchive>;
  BackendType archive_;

  explicit ZipArchive(BackendType archive) : archive_(std::move(archive)) {}
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_READ_H_
