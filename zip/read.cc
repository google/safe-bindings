#include "read.h"

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
#include "file.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

namespace {

template <typename RustArchive>
absl::Status CheckArchiveOpen(const RustArchive& archive) {
  if (archive.is_none()) {
    return absl::FailedPreconditionError("Zip archive is not open");
  }
  return absl::OkStatus();
}

// Rejects opening a second entry while one returned by the same handle is
// still alive (see internal::ActiveEntryGuard). Must be called after the
// is_none check: `active_entry` is null on a moved-from handle.
absl::Status CheckNoActiveEntry(const std::shared_ptr<bool>& active_entry) {
  if (active_entry != nullptr && *active_entry) {
    return absl::FailedPreconditionError(
        "Another entry of this archive/stream is still open; destroy it "
        "before opening the next one");
  }
  return absl::OkStatus();
}

// The archive backends differ only in the Rust archive type they wrap
// (rust::{Buffered,Fs}ZipArchive), so the methods whose bodies would
// otherwise be identical share the following helpers.
template <typename RustArchive>
absl::StatusOr<uintptr_t> GetLengthImpl(const RustArchive& archive) {
  ABSL_RETURN_IF_ERROR(CheckArchiveOpen(archive));
  return archive.get_length();
}

template <typename RustArchive>
absl::StatusOr<std::string> GetCommentImpl(const RustArchive& archive) {
  ABSL_RETURN_IF_ERROR(CheckArchiveOpen(archive));
  rust::VecU8 comment_vec = archive.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}

// Shared by GetFileByIndex (`raw == false`) and GetFileByIndexRaw
// (`raw == true`). `convert` is the FromRust*ZipFile converter for the
// backend's entry type and `wrap` the matching ZipFile factory. The returned
// ZipFile holds a guard on `active_entry` until it is destroyed.
template <typename RustArchive, typename ConvertFn, typename WrapFn>
absl::StatusOr<ZipFile> GetFileByIndexImpl(
    RustArchive& archive, const std::shared_ptr<bool>& active_entry,
    uintptr_t index, bool raw, ConvertFn convert, WrapFn wrap) {
  ABSL_RETURN_IF_ERROR(CheckArchiveOpen(archive));
  ABSL_RETURN_IF_ERROR(CheckNoActiveEntry(active_entry));
  using RustZipFile =
      typename decltype(convert(archive.get_file_by_index(index)))::value_type;
  ABSL_ASSIGN_OR_RETURN(RustZipFile file,
                        raw ? convert(archive.get_file_by_index_raw(index))
                            : convert(archive.get_file_by_index(index)));
  return wrap(std::move(file), internal::ActiveEntryGuard(active_entry));
}

template <typename RustArchive>
absl::StatusOr<ZipEntryMetadata> GetEntryMetadataImpl(
    const RustArchive& archive, uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckArchiveOpen(archive));
  ABSL_ASSIGN_OR_RETURN(
      rust::ZipEntryMetadata metadata,
      FromRustZipEntryMetadata(archive.get_entry_metadata(index)));
  return ZipEntryMetadata(std::move(metadata));
}

template <typename StreamReader, typename ConvertFn, typename WrapFn>
absl::StatusOr<std::optional<ZipFile>> ReadNextFileImpl(
    StreamReader& reader, const std::shared_ptr<bool>& active_entry,
    std::optional<uint64_t> compressed_size, ConvertFn convert, WrapFn wrap) {
  if (reader.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
  ABSL_RETURN_IF_ERROR(CheckNoActiveEntry(active_entry));
  using RustZipFile =
      typename decltype(convert(reader.read_next_file()))::value_type;
  ABSL_ASSIGN_OR_RETURN(
      RustZipFile file,
      compressed_size.has_value()
          ? convert(
                reader.read_next_file_with_compressed_size(*compressed_size))
          : convert(reader.read_next_file()));
  if (file.is_none()) {
    return std::nullopt;
  }
  return wrap(std::move(file), internal::ActiveEntryGuard(active_entry));
}

}  // namespace

absl::StatusOr<BufferedZipArchive> BufferedZipArchive::NewFromData(
    absl::string_view data) {
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipArchive archive,
      FromRustBufferedZipArchive(rust::BufferedZipArchive::new_from_data(
          rust::VecU8::copy_from_slice(ToByteSpan(data)))));
  return BufferedZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> BufferedZipArchive::GetLength() const {
  return GetLengthImpl(archive_);
}

absl::StatusOr<std::string> BufferedZipArchive::GetComment() const {
  return GetCommentImpl(archive_);
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndex(uintptr_t index) {
  return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/false,
                            FromRustBufferedZipFile, ZipFile::FromBuffer);
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndexRaw(uintptr_t index) {
  return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/true,
                            FromRustBufferedZipFile, ZipFile::FromBuffer);
}

absl::StatusOr<ZipEntryMetadata> BufferedZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(archive_, index);
}

bool BufferedZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<FsZipArchive> FsZipArchive::NewFromPath(absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipArchive archive,
      FromRustFsZipArchive(
          rust::FsZipArchive::new_from_path(ToByteSpan(path))));
  return FsZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> FsZipArchive::GetLength() const {
  return GetLengthImpl(archive_);
}

absl::StatusOr<std::string> FsZipArchive::GetComment() const {
  return GetCommentImpl(archive_);
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndex(uintptr_t index) {
  return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/false,
                            FromRustFsZipFile, ZipFile::FromFile);
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndexRaw(uintptr_t index) {
  return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/true,
                            FromRustFsZipFile, ZipFile::FromFile);
}

absl::StatusOr<ZipEntryMetadata> FsZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(archive_, index);
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

absl::StatusOr<ZipEntryMetadata> ZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return std::visit(
      [index](const auto& arg) { return arg.GetEntryMetadata(index); },
      archive_);
}

bool ZipArchive::IsNone() const {
  return std::visit([](const auto& arg) { return arg.IsNone(); }, archive_);
}

absl::StatusOr<BufferedZipStreamReader> BufferedZipStreamReader::NewFromData(
    absl::string_view data) {
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipStreamReader reader,
      FromRustBufferedZipStreamReader(
          rust::BufferedZipStreamReader::new_from_data(
              rust::VecU8::copy_from_slice(ToByteSpan(data)))));
  return BufferedZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> BufferedZipStreamReader::ReadNextFile() {
  return ReadNextFileImpl(reader_, active_entry_, std::nullopt,
                          FromRustBufferedZipFile, ZipFile::FromBuffer);
}

absl::StatusOr<std::optional<ZipFile>>
BufferedZipStreamReader::ReadNextFileWithCompressedSize(
    uint64_t compressed_size) {
  return ReadNextFileImpl(reader_, active_entry_, compressed_size,
                          FromRustBufferedZipFile, ZipFile::FromBuffer);
}

bool BufferedZipStreamReader::IsFinished() const {
  return reader_.is_finished();
}

bool BufferedZipStreamReader::IsNone() const { return reader_.is_none(); }

absl::StatusOr<FsZipStreamReader> FsZipStreamReader::NewFromPath(
    absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipStreamReader reader,
      FromRustFsZipStreamReader(
          rust::FsZipStreamReader::new_from_path(ToByteSpan(path))));
  return FsZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> FsZipStreamReader::ReadNextFile() {
  return ReadNextFileImpl(reader_, active_entry_, std::nullopt,
                          FromRustFsZipFile, ZipFile::FromFile);
}

absl::StatusOr<std::optional<ZipFile>>
FsZipStreamReader::ReadNextFileWithCompressedSize(uint64_t compressed_size) {
  return ReadNextFileImpl(reader_, active_entry_, compressed_size,
                          FromRustFsZipFile, ZipFile::FromFile);
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

}  // namespace security::zip
