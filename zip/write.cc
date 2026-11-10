#include "write.h"

#include <cstdint>
#include <utility>
#include <variant>

#include "converters.h"
#include "file.h"
#include "crubit/rust.h"
#include "absl/functional/overload.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {
namespace internal {

struct ZipFileWriterPeer {
  template <typename RustWriter>
  static absl::Status WriteZipFileContent(RustWriter& writer, ZipFile& file) {
    return std::visit(
        absl::Overload{[&](BufferedZipFile& file) {
                         return FromRustResultUnit(
                             writer.write_buffered_zip_file_content(file.zip_));
                       },
                       [&](FsZipFile& file) {
                         return FromRustResultUnit(
                             writer.write_fs_zip_file_content(file.zip_));
                       }},
        file.zip_);
  }
};

}  // namespace internal

namespace {

absl::StatusOr<rust::CompressionMethod> ToZipWrapperCompressionMethod(
    CompressionMethod method) {
  rust::CompressionMethod rust_method =
      rust::CompressionMethod::from_i32(static_cast<int32_t>(method));
  if (rust_method.tag == rust::CompressionMethod::Tag::Unsupported) {
    return absl::InvalidArgumentError("Invalid CompressionMethod value");
  }
  return rust_method;
}

// The four Rust writer types (Buffered/Fs x seekable/stream) expose the same
// method set, so the bodies below are shared.
template <typename RustWriter>
absl::Status StartFileImpl(RustWriter& writer, absl::string_view file_name,
                           const rust::ZipWriterFileOptions& options) {
  return FromRustResultUnit(writer.start_file(ToByteSpan(file_name), options));
}

template <typename RustWriter>
absl::Status AddDirectoryImpl(
    RustWriter& writer, absl::string_view file_name,
    const rust::ZipWriterFileOptions& options) {
  return FromRustResultUnit(
      writer.add_directory(ToByteSpan(file_name), options));
}

template <typename RustWriter>
absl::Status WriteDataImpl(RustWriter& writer, absl::string_view data) {
  return FromRustResultUnit(
      writer.write_data(rust::VecU8::copy_from_slice(ToByteSpan(data))));
}

template <typename RustWriter>
absl::Status WriteFileContentImpl(RustWriter& writer, absl::string_view path) {
  return FromRustResultUnit(writer.write_file_content(ToByteSpan(path)));
}

template <typename RustWriter>
absl::Status SetCommentImpl(RustWriter& writer, absl::string_view comment) {
  return FromRustResultUnit(writer.set_comment(ToByteSpan(comment)));
}

template <typename RustWriter>
absl::Status FlushImpl(RustWriter& writer) {
  return FromRustResultUnit(writer.flush());
}

// Shared by the ZipWriter and ZipStreamWriter facades. The buffer backend's
// Finish() returns the archive bytes; the file backend's returns only a
// status, since the archive is on disk, so it is mapped to an empty wrapper.
template <typename BufferedWriter, typename FsWriter>
absl::StatusOr<RustVecU8Wrapper> FinishImpl(
    std::variant<BufferedWriter, FsWriter>& writer) {
  return std::visit(
      absl::Overload{
          [](BufferedWriter& writer) -> absl::StatusOr<RustVecU8Wrapper> {
            return writer.Finish();
          },
          [](FsWriter& writer) -> absl::StatusOr<RustVecU8Wrapper> {
            ABSL_RETURN_IF_ERROR(writer.Finish());
            return RustVecU8Wrapper();
          }},
      writer);
}

}  // namespace

absl::Status ZipWriterFileOptions::SetCompressionMethod(
    CompressionMethod method) {
  ABSL_ASSIGN_OR_RETURN(rust::CompressionMethod rust_method,
                        ToZipWrapperCompressionMethod(method));
  options_ = options_.compression_method(rust_method);
  return absl::OkStatus();
}

absl::StatusOr<BufferedZipWriter> BufferedZipWriter::NewFromData(
    absl::string_view data, bool append) {
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipWriter writer,
      FromRustBufferedZipWriter(rust::BufferedZipWriter::new_from_data(
          rust::VecU8::copy_from_slice(ToByteSpan(data)), append)));
  return BufferedZipWriter(std::move(writer));
}

absl::StatusOr<RustVecU8Wrapper> BufferedZipWriter::Finish() {
  return FromRustResultVecU8(writer_.finish());
}

absl::Status BufferedZipWriter::StartFile(absl::string_view file_name,
                                          const ZipWriterFileOptions& options) {
  return StartFileImpl(writer_, file_name, options.options_);
}

absl::Status BufferedZipWriter::AddDirectory(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return AddDirectoryImpl(writer_, file_name, options.options_);
}

absl::Status BufferedZipWriter::WriteData(absl::string_view data) {
  return WriteDataImpl(writer_, data);
}

absl::Status BufferedZipWriter::WriteZipFileContent(ZipFile& file) {
  return internal::ZipFileWriterPeer::WriteZipFileContent(writer_, file);
}

absl::Status BufferedZipWriter::WriteFileContent(absl::string_view path) {
  return WriteFileContentImpl(writer_, path);
}

absl::Status BufferedZipWriter::SetComment(absl::string_view comment) {
  return SetCommentImpl(writer_, comment);
}

absl::Status BufferedZipWriter::Flush() { return FlushImpl(writer_); }

bool BufferedZipWriter::IsSeekPossible() const {
  return writer_.is_seek_possible();
}

bool BufferedZipWriter::IsNone() const { return writer_.is_none(); }

absl::StatusOr<FsZipWriter> FsZipWriter::NewFromPath(absl::string_view path,
                                                     bool append) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipWriter writer,
      FromRustFsZipWriter(
          rust::FsZipWriter::new_from_path(ToByteSpan(path), append)));
  return FsZipWriter(std::move(writer));
}

absl::Status FsZipWriter::Finish() {
  return FromRustResultUnit(writer_.finish());
}

absl::Status FsZipWriter::StartFile(absl::string_view file_name,
                                    const ZipWriterFileOptions& options) {
  return StartFileImpl(writer_, file_name, options.options_);
}

absl::Status FsZipWriter::AddDirectory(absl::string_view file_name,
                                       const ZipWriterFileOptions& options) {
  return AddDirectoryImpl(writer_, file_name, options.options_);
}

absl::Status FsZipWriter::WriteData(absl::string_view data) {
  return WriteDataImpl(writer_, data);
}

absl::Status FsZipWriter::WriteZipFileContent(ZipFile& file) {
  return internal::ZipFileWriterPeer::WriteZipFileContent(writer_, file);
}

absl::Status FsZipWriter::WriteFileContent(absl::string_view path) {
  return WriteFileContentImpl(writer_, path);
}

absl::Status FsZipWriter::SetComment(absl::string_view comment) {
  return SetCommentImpl(writer_, comment);
}

absl::Status FsZipWriter::Flush() { return FlushImpl(writer_); }

bool FsZipWriter::IsSeekPossible() const { return writer_.is_seek_possible(); }

bool FsZipWriter::IsNone() const { return writer_.is_none(); }

absl::StatusOr<ZipWriter> ZipWriter::FromFile(absl::string_view path,
                                              bool append) {
  ABSL_ASSIGN_OR_RETURN(FsZipWriter writer,
                        FsZipWriter::NewFromPath(path, append));
  return ZipWriter(std::move(writer));
}

absl::StatusOr<ZipWriter> ZipWriter::FromBuffer(absl::string_view data,
                                                bool append) {
  ABSL_ASSIGN_OR_RETURN(BufferedZipWriter writer,
                        BufferedZipWriter::NewFromData(data, append));
  return ZipWriter(std::move(writer));
}

absl::StatusOr<RustVecU8Wrapper> ZipWriter::Finish() {
  return FinishImpl(writer_);
}

absl::Status ZipWriter::StartFile(absl::string_view file_name,
                                  const ZipWriterFileOptions& options) {
  return std::visit(
      [&](auto& writer) { return writer.StartFile(file_name, options); },
      writer_);
}

absl::Status ZipWriter::AddDirectory(absl::string_view file_name,
                                     const ZipWriterFileOptions& options) {
  return std::visit(
      [&](auto& writer) { return writer.AddDirectory(file_name, options); },
      writer_);
}

absl::Status ZipWriter::WriteData(absl::string_view data) {
  return std::visit([&](auto& writer) { return writer.WriteData(data); },
                    writer_);
}

absl::Status ZipWriter::WriteZipFileContent(ZipFile& file) {
  return std::visit(
      [&](auto& writer) { return writer.WriteZipFileContent(file); }, writer_);
}

absl::Status ZipWriter::WriteFileContent(absl::string_view path) {
  return std::visit([&](auto& writer) { return writer.WriteFileContent(path); },
                    writer_);
}

absl::Status ZipWriter::SetComment(absl::string_view comment) {
  return std::visit([&](auto& writer) { return writer.SetComment(comment); },
                    writer_);
}

absl::Status ZipWriter::Flush() {
  return std::visit([&](auto& writer) { return writer.Flush(); }, writer_);
}

bool ZipWriter::IsSeekPossible() const {
  return std::visit([&](const auto& writer) { return writer.IsSeekPossible(); },
                    writer_);
}

bool ZipWriter::IsNone() const {
  return std::visit([&](const auto& writer) { return writer.IsNone(); },
                    writer_);
}

absl::StatusOr<BufferedZipStreamWriter> BufferedZipStreamWriter::New() {
  return BufferedZipStreamWriter(
      rust::BufferedZipStreamWriter::new_stream());
}

absl::StatusOr<RustVecU8Wrapper> BufferedZipStreamWriter::Finish() {
  return FromRustResultVecU8(writer_.finish());
}

absl::Status BufferedZipStreamWriter::StartFile(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return StartFileImpl(writer_, file_name, options.options_);
}

absl::Status BufferedZipStreamWriter::AddDirectory(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return AddDirectoryImpl(writer_, file_name, options.options_);
}

absl::Status BufferedZipStreamWriter::WriteData(absl::string_view data) {
  return WriteDataImpl(writer_, data);
}

absl::Status BufferedZipStreamWriter::WriteZipFileContent(ZipFile& file) {
  return internal::ZipFileWriterPeer::WriteZipFileContent(writer_, file);
}

absl::Status BufferedZipStreamWriter::WriteFileContent(absl::string_view path) {
  return WriteFileContentImpl(writer_, path);
}

absl::Status BufferedZipStreamWriter::SetComment(absl::string_view comment) {
  return SetCommentImpl(writer_, comment);
}

absl::Status BufferedZipStreamWriter::Flush() { return FlushImpl(writer_); }

bool BufferedZipStreamWriter::IsSeekPossible() const {
  return writer_.is_seek_possible();
}

bool BufferedZipStreamWriter::IsNone() const { return writer_.is_none(); }

absl::StatusOr<FsZipStreamWriter> FsZipStreamWriter::NewFromPath(
    absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipStreamWriter writer,
      FromRustFsZipStreamWriter(
          rust::FsZipStreamWriter::new_from_path(ToByteSpan(path))));
  return FsZipStreamWriter(std::move(writer));
}

absl::Status FsZipStreamWriter::Finish() {
  return FromRustResultUnit(writer_.finish());
}

absl::Status FsZipStreamWriter::StartFile(absl::string_view file_name,
                                          const ZipWriterFileOptions& options) {
  return StartFileImpl(writer_, file_name, options.options_);
}

absl::Status FsZipStreamWriter::AddDirectory(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return AddDirectoryImpl(writer_, file_name, options.options_);
}

absl::Status FsZipStreamWriter::WriteData(absl::string_view data) {
  return WriteDataImpl(writer_, data);
}

absl::Status FsZipStreamWriter::WriteZipFileContent(ZipFile& file) {
  return internal::ZipFileWriterPeer::WriteZipFileContent(writer_, file);
}

absl::Status FsZipStreamWriter::WriteFileContent(absl::string_view path) {
  return WriteFileContentImpl(writer_, path);
}

absl::Status FsZipStreamWriter::SetComment(absl::string_view comment) {
  return SetCommentImpl(writer_, comment);
}

absl::Status FsZipStreamWriter::Flush() { return FlushImpl(writer_); }

bool FsZipStreamWriter::IsSeekPossible() const {
  return writer_.is_seek_possible();
}

bool FsZipStreamWriter::IsNone() const { return writer_.is_none(); }

absl::StatusOr<ZipStreamWriter> ZipStreamWriter::FromFile(
    absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(FsZipStreamWriter writer,
                        FsZipStreamWriter::NewFromPath(path));
  return ZipStreamWriter(std::move(writer));
}

absl::StatusOr<ZipStreamWriter> ZipStreamWriter::FromBuffer() {
  ABSL_ASSIGN_OR_RETURN(BufferedZipStreamWriter writer,
                        BufferedZipStreamWriter::New());
  return ZipStreamWriter(std::move(writer));
}

absl::StatusOr<RustVecU8Wrapper> ZipStreamWriter::Finish() {
  return FinishImpl(writer_);
}

absl::Status ZipStreamWriter::StartFile(absl::string_view file_name,
                                        const ZipWriterFileOptions& options) {
  return std::visit(
      [&](auto& writer) { return writer.StartFile(file_name, options); },
      writer_);
}

absl::Status ZipStreamWriter::AddDirectory(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return std::visit(
      [&](auto& writer) { return writer.AddDirectory(file_name, options); },
      writer_);
}

absl::Status ZipStreamWriter::WriteData(absl::string_view data) {
  return std::visit([&](auto& writer) { return writer.WriteData(data); },
                    writer_);
}

absl::Status ZipStreamWriter::WriteZipFileContent(ZipFile& file) {
  return std::visit(
      [&](auto& writer) { return writer.WriteZipFileContent(file); }, writer_);
}

absl::Status ZipStreamWriter::WriteFileContent(absl::string_view path) {
  return std::visit([&](auto& writer) { return writer.WriteFileContent(path); },
                    writer_);
}

absl::Status ZipStreamWriter::SetComment(absl::string_view comment) {
  return std::visit([&](auto& writer) { return writer.SetComment(comment); },
                    writer_);
}

absl::Status ZipStreamWriter::Flush() {
  return std::visit([&](auto& writer) { return writer.Flush(); }, writer_);
}

bool ZipStreamWriter::IsSeekPossible() const {
  return std::visit([&](const auto& writer) { return writer.IsSeekPossible(); },
                    writer_);
}

bool ZipStreamWriter::IsNone() const {
  return std::visit([&](const auto& writer) { return writer.IsNone(); },
                    writer_);
}

}  // namespace security::zip
