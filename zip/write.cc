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
#include "absl/types/span.h"
namespace security::zip {

namespace {

absl::StatusOr<rust::CompressionMethod> ToZipWrapperCompressionMethod(
    CompressionMethod method) {
  return rust::CompressionMethod::from_i32(static_cast<int32_t>(method));
}

}  // namespace

absl::Status ZipWriterFileOptions::SetCompressionMethod(
    CompressionMethod method) {
  absl::StatusOr<rust::CompressionMethod> rust_method =
      ToZipWrapperCompressionMethod(method);
  if (!rust_method.ok()) return rust_method.status();
  options_ = options_.compression_method(*rust_method);
  return absl::OkStatus();
}

absl::StatusOr<BufferedZipWriter> BufferedZipWriter::NewFromData(
    absl::string_view data, bool append) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  ASSIGN_OR_RETURN(
      rust::BufferedZipWriter writer,
      FromRustBufferedZipWriter(
          rust::BufferedZipWriter::new_from_data(input_data, append)));
  return BufferedZipWriter(std::move(writer));
}

absl::StatusOr<RustVecU8Wrapper> BufferedZipWriter::Finish() {
  return FromRustResultVecU8(writer_.finish());
}

absl::Status BufferedZipWriter::StartFile(absl::string_view file_name,
                                          const ZipWriterFileOptions& options) {
  return FromRustResultUnit(writer_.start_file(
      absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(file_name.data()), file_name.size()),
      options.options_));
}

absl::Status BufferedZipWriter::AddDirectory(
    absl::string_view file_name, const ZipWriterFileOptions& options) {
  return FromRustResultUnit(writer_.add_directory(
      absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(file_name.data()), file_name.size()),
      options.options_));
}

absl::Status BufferedZipWriter::WriteData(absl::string_view data) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  return FromRustResultUnit(writer_.write_data(input_data));
}

absl::Status BufferedZipWriter::WriteZipFileContent(ZipFile& file) {
  return std::visit(
      absl::Overload{[&](BufferedZipFile& file) {
                       return FromRustResultUnit(
                           writer_.write_buffered_zip_file_content(file.zip_));
                     },
                     [&](FsZipFile& file) {
                       return FromRustResultUnit(
                           writer_.write_fs_zip_file_content(file.zip_));
                     }},
      file.zip_);
}

absl::Status BufferedZipWriter::WriteFileContent(absl::string_view path) {
  return FromRustResultUnit(
      writer_.write_file_content(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(path.data()), path.size())));
}

absl::StatusOr<FsZipWriter> FsZipWriter::NewFromPath(absl::string_view path,
                                                     bool append) {
  ASSIGN_OR_RETURN(
      rust::FsZipWriter writer,
      FromRustFsZipWriter(rust::FsZipWriter::new_from_path(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(path.data()), path.size()),
          append)));
  return FsZipWriter(std::move(writer));
}

absl::Status FsZipWriter::Finish() {
  return FromRustResultUnit(writer_.finish());
}

absl::Status FsZipWriter::StartFile(absl::string_view file_name,
                                    const ZipWriterFileOptions& options) {
  return FromRustResultUnit(writer_.start_file(
      absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(file_name.data()), file_name.size()),
      options.options_));
}

absl::Status FsZipWriter::AddDirectory(absl::string_view file_name,
                                       const ZipWriterFileOptions& options) {
  return FromRustResultUnit(writer_.add_directory(
      absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(file_name.data()), file_name.size()),
      options.options_));
}

absl::Status FsZipWriter::WriteData(absl::string_view data) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  return FromRustResultUnit(writer_.write_data(input_data));
}

absl::Status FsZipWriter::WriteZipFileContent(ZipFile& file) {
  return std::visit(
      absl::Overload{[&](BufferedZipFile& file) {
                       return FromRustResultUnit(
                           writer_.write_buffered_zip_file_content(file.zip_));
                     },
                     [&](FsZipFile& file) {
                       return FromRustResultUnit(
                           writer_.write_fs_zip_file_content(file.zip_));
                     }},
      file.zip_);
}

absl::Status FsZipWriter::WriteFileContent(absl::string_view path) {
  return FromRustResultUnit(
      writer_.write_file_content(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(path.data()), path.size())));
}

absl::StatusOr<ZipWriter> ZipWriter::FromFile(absl::string_view path,
                                              bool append) {
  ASSIGN_OR_RETURN(FsZipWriter writer, FsZipWriter::NewFromPath(path, append));
  return ZipWriter(std::move(writer));
}

absl::StatusOr<ZipWriter> ZipWriter::FromBuffer(absl::string_view data,
                                                bool append) {
  ASSIGN_OR_RETURN(BufferedZipWriter writer,
                   BufferedZipWriter::NewFromData(data, append));
  return ZipWriter(std::move(writer));
}

absl::StatusOr<RustVecU8Wrapper> ZipWriter::Finish() {
  return std::visit(
      absl::Overload{
          [](BufferedZipWriter& writer) -> absl::StatusOr<RustVecU8Wrapper> {
            return writer.Finish();
          },
          [](FsZipWriter& writer) -> absl::StatusOr<RustVecU8Wrapper> {
            RETURN_IF_ERROR(writer.Finish());
            // Empty RustVecU8Wrapper for FsZipWriter.
            // This is because FsZipWriter writes to a file on the filesystem
            // and doesn't return an owned buffer.
            return RustVecU8Wrapper();
          }},
      writer_);
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

}  // namespace security::zip
