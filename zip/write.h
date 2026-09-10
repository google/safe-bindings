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
class BufferedZipStreamWriter;
class FsZipStreamWriter;
class ZipStreamWriter;

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
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
  friend class ZipStreamWriter;
};

class BufferedZipWriter final {
 public:
  static absl::StatusOr<BufferedZipWriter> NewFromData(absl::string_view data,
                                                       bool append);
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit BufferedZipWriter(rust::BufferedZipWriter writer)
      : writer_(std::move(writer)) {}
  rust::BufferedZipWriter writer_;
};

class FsZipWriter final {
 public:
  static absl::StatusOr<FsZipWriter> NewFromPath(absl::string_view path,
                                                 bool append);
  [[nodiscard]] absl::Status Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

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
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipWriter, FsZipWriter>;

  explicit ZipWriter(BackendType writer) : writer_(std::move(writer)) {}
  BackendType writer_;
};

// Streaming zip writers write zip archives sequentially without seeking.
class BufferedZipStreamWriter final {
 public:
  static absl::StatusOr<BufferedZipStreamWriter> New();
  static absl::StatusOr<BufferedZipStreamWriter> NewFromData(
      absl::string_view data);
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit BufferedZipStreamWriter(rust::BufferedZipStreamWriter writer)
      : writer_(std::move(writer)) {}
  rust::BufferedZipStreamWriter writer_;
};

class FsZipStreamWriter final {
 public:
  static absl::StatusOr<FsZipStreamWriter> NewFromPath(absl::string_view path);
  [[nodiscard]] absl::Status Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit FsZipStreamWriter(rust::FsZipStreamWriter writer)
      : writer_(std::move(writer)) {}
  rust::FsZipStreamWriter writer_;
};

class ZipStreamWriter final {
 public:
  static absl::StatusOr<ZipStreamWriter> FromFile(absl::string_view path);
  static absl::StatusOr<ZipStreamWriter> FromBuffer();
  static absl::StatusOr<ZipStreamWriter> FromBuffer(absl::string_view data);
  [[nodiscard]] absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipStreamWriter, FsZipStreamWriter>;

  explicit ZipStreamWriter(BackendType writer) : writer_(std::move(writer)) {}
  BackendType writer_;
};

absl::StatusOr<BufferedZipStreamWriter> NewBufferedZipStreamWriter();
absl::StatusOr<FsZipStreamWriter> NewFsZipStreamWriter(absl::string_view path);

}  // namespace security::zip

#endif  // SECURITY_ZIP_WRITE_H_
