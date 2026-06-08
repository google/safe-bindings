#ifndef SECURITY_ZIP_WRITE_H_
#define SECURITY_ZIP_WRITE_H_

#include <cstdint>
#include <utility>
#include <variant>

#include "converters.h"
#include "file.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

class BufferedZipWriter;
class FsZipWriter;

class ZipWriterFileOptions {
 public:
  ZipWriterFileOptions()
      : options_(rust::ZipWriterFileOptions::new_()) {}

  absl::Status SetCompressionMethod(CompressionMethod method);

  void SetCompressionLevel(int64_t level) {
    options_ = options_.compression_level(level);
  }

  void SetUnixPermissions(uint32_t permissions) {
    options_ = options_.unix_permissions(permissions);
  }

  void SetIsLargeFile(bool is_large_file) {
    options_ = options_.large_file(is_large_file);
  }

 private:
  rust::ZipWriterFileOptions options_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
};

class BufferedZipWriter final {
 public:
  static absl::StatusOr<BufferedZipWriter> NewFromData(absl::string_view data,
                                                       bool append);
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);

 private:
  explicit BufferedZipWriter(rust::BufferedZipWriter writer)
      : writer_(std::move(writer)) {}
  rust::BufferedZipWriter writer_;
};

class FsZipWriter final {
 public:
  static absl::StatusOr<FsZipWriter> NewFromPath(absl::string_view path,
                                                 bool append);
  absl::Status Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);

 private:
  explicit FsZipWriter(rust::FsZipWriter writer)
      : writer_(std::move(writer)) {}
  rust::FsZipWriter writer_;
};

class ZipWriter final {
 public:
  static absl::StatusOr<ZipWriter> FromFile(absl::string_view path,
                                            bool append);
  static absl::StatusOr<ZipWriter> FromBuffer(absl::string_view data,
                                              bool append);
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);

 private:
  using BackendType = std::variant<BufferedZipWriter, FsZipWriter>;

  explicit ZipWriter(BackendType writer) : writer_(std::move(writer)) {}
  BackendType writer_;
};

}  // namespace security::zip

#endif  // SECURITY_ZIP_WRITE_H_
