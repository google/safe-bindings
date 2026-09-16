#include "read.h"

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
#include "file.h"
#include "crubit/rust.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
namespace security::zip {

absl::StatusOr<BufferedZipArchive> BufferedZipArchive::NewFromData(
    absl::string_view data) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipArchive archive,
      FromRustBufferedZipArchive(
          rust::BufferedZipArchive::new_from_data(input_data)));
  return BufferedZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> BufferedZipArchive::GetLength() const {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  return archive_.get_length();
}

absl::StatusOr<std::string> BufferedZipArchive::GetComment() const {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  rust::VecU8 comment_vec = archive_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndex(uintptr_t index) {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipFile file,
      FromRustBufferedZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndexRaw(uintptr_t index) {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipFile file,
      FromRustBufferedZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromBuffer(std::move(file));
}

bool BufferedZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<FsZipArchive> FsZipArchive::NewFromPath(absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipArchive archive,
      FromRustFsZipArchive(
          rust::FsZipArchive::new_from_path(absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(path.data()), path.size()))));
  return FsZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> FsZipArchive::GetLength() const {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  return archive_.get_length();
}

absl::StatusOr<std::string> FsZipArchive::GetComment() const {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  rust::VecU8 comment_vec = archive_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndex(uintptr_t index) {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  ABSL_ASSIGN_OR_RETURN(rust::FsZipFile file,
                        FromRustFsZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndexRaw(uintptr_t index) {
  if (archive_.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipFile file,
      FromRustFsZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromFile(std::move(file));
}

bool FsZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<ZipArchive> ZipArchive::FromFile(absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(FsZipArchive archive, FsZipArchive::NewFromPath(path));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<ZipArchive> ZipArchive::FromBuffer(absl::string_view data) {
  ABSL_ASSIGN_OR_RETURN(BufferedZipArchive archive,
                        BufferedZipArchive::NewFromData(data));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> ZipArchive::GetLength() const {
  return std::visit([](const auto& arg) { return arg.GetLength(); }, archive_);
}

absl::StatusOr<std::string> ZipArchive::GetComment() const {
  return std::visit([](const auto& arg) { return arg.GetComment(); }, archive_);
}

absl::StatusOr<ZipFile> ZipArchive::GetFileByIndex(uintptr_t index) {
  return std::visit([index](auto& arg) { return arg.GetFileByIndex(index); },
                    archive_);
}

absl::StatusOr<ZipFile> ZipArchive::GetFileByIndexRaw(uintptr_t index) {
  return std::visit([index](auto& arg) { return arg.GetFileByIndexRaw(index); },
                    archive_);
}

bool ZipArchive::IsNone() const {
  return std::visit([](const auto& arg) { return arg.IsNone(); }, archive_);
}

absl::StatusOr<BufferedZipStreamReader> BufferedZipStreamReader::NewFromData(
    absl::string_view data) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipStreamReader reader,
      FromRustBufferedZipStreamReader(
          rust::BufferedZipStreamReader::new_from_data(input_data)));
  return BufferedZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> BufferedZipStreamReader::ReadNextFile() {
  if (reader_.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
  ABSL_ASSIGN_OR_RETURN(rust::BufferedZipFile file,
                        FromRustBufferedZipFile(reader_.read_next_file()));
  if (file.is_none()) {
    return std::nullopt;
  }
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<std::optional<ZipFile>>
BufferedZipStreamReader::ReadNextFileWithCompressedSize(
    uint64_t compressed_size) {
  if (reader_.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipFile file,
      FromRustBufferedZipFile(
          reader_.read_next_file_with_compressed_size(compressed_size)));
  if (file.is_none()) {
    return std::nullopt;
  }
  return ZipFile::FromBuffer(std::move(file));
}

bool BufferedZipStreamReader::IsFinished() const {
  return reader_.is_finished();
}

bool BufferedZipStreamReader::IsNone() const { return reader_.is_none(); }

absl::StatusOr<FsZipStreamReader> FsZipStreamReader::NewFromPath(
    absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipStreamReader reader,
      FromRustFsZipStreamReader(rust::FsZipStreamReader::new_from_path(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(path.data()), path.size()))));
  return FsZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> FsZipStreamReader::ReadNextFile() {
  if (reader_.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
  ABSL_ASSIGN_OR_RETURN(rust::FsZipFile file,
                        FromRustFsZipFile(reader_.read_next_file()));
  if (file.is_none()) {
    return std::nullopt;
  }
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<std::optional<ZipFile>>
FsZipStreamReader::ReadNextFileWithCompressedSize(uint64_t compressed_size) {
  if (reader_.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipFile file,
      FromRustFsZipFile(
          reader_.read_next_file_with_compressed_size(compressed_size)));
  if (file.is_none()) {
    return std::nullopt;
  }
  return ZipFile::FromFile(std::move(file));
}

bool FsZipStreamReader::IsFinished() const { return reader_.is_finished(); }

bool FsZipStreamReader::IsNone() const { return reader_.is_none(); }

absl::StatusOr<ZipStreamReader> ZipStreamReader::FromFile(
    absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(FsZipStreamReader reader,
                        FsZipStreamReader::NewFromPath(path));
  return ZipStreamReader(std::move(reader));
}

absl::StatusOr<ZipStreamReader> ZipStreamReader::FromBuffer(
    absl::string_view data) {
  ABSL_ASSIGN_OR_RETURN(BufferedZipStreamReader reader,
                        BufferedZipStreamReader::NewFromData(data));
  return ZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> ZipStreamReader::ReadNextFile() {
  return std::visit([](auto& arg) { return arg.ReadNextFile(); }, reader_);
}

absl::StatusOr<std::optional<ZipFile>>
ZipStreamReader::ReadNextFileWithCompressedSize(uint64_t compressed_size) {
  return std::visit(
      [compressed_size](auto& arg) {
        return arg.ReadNextFileWithCompressedSize(compressed_size);
      },
      reader_);
}

bool ZipStreamReader::IsFinished() const {
  return std::visit([](const auto& arg) { return arg.IsFinished(); }, reader_);
}

bool ZipStreamReader::IsNone() const {
  return std::visit([](const auto& arg) { return arg.IsNone(); }, reader_);
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    BufferedZipStreamReader& reader) {
  return reader.ReadNextFile();
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    FsZipStreamReader& reader) {
  return reader.ReadNextFile();
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStream(
    ZipStreamReader& reader) {
  return reader.ReadNextFile();
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    BufferedZipStreamReader& reader, uint64_t compressed_size) {
  return reader.ReadNextFileWithCompressedSize(compressed_size);
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    FsZipStreamReader& reader, uint64_t compressed_size) {
  return reader.ReadNextFileWithCompressedSize(compressed_size);
}

absl::StatusOr<std::optional<ZipFile>> ReadZipFileFromStreamWithCompressedSize(
    ZipStreamReader& reader, uint64_t compressed_size) {
  return reader.ReadNextFileWithCompressedSize(compressed_size);
}

}  // namespace security::zip
