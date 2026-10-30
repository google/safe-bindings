#include "read.h"

#include <cstddef>
#include <cstdint>
#include <limits>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
#include "file.h"
#include "reader.h"
#include "crubit/rust.h"
#include "absl/functional/overload.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
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

template <typename RustArchive>
absl::StatusOr<std::string> GetCommentImpl(const absl::Status& open_status,
                                           const RustArchive& archive) {
  ABSL_RETURN_IF_ERROR(open_status);
  rust::VecU8 comment_vec = archive.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}

template <typename RustArchive>
absl::StatusOr<ZipEntryMetadata> GetEntryMetadataImpl(
    const absl::Status& open_status, const RustArchive& archive,
    uintptr_t index) {
  ABSL_RETURN_IF_ERROR(open_status);
  ABSL_ASSIGN_OR_RETURN(
      rust::ZipEntryMetadata metadata,
      FromRustZipEntryMetadata(archive.get_entry_metadata(index)));
  return ZipEntryMetadata(std::move(metadata));
}

template <typename StreamReader, typename ConvertFn, typename WrapFn>
absl::StatusOr<std::optional<ZipFile>> ReadNextFileImpl(
    StreamReader& reader, std::optional<uint64_t> compressed_size,
    ConvertFn convert, WrapFn wrap) {
  if (reader.is_none()) {
    return absl::FailedPreconditionError("Zip stream reader is not open");
  }
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
  return wrap(std::move(file));
}

// The three callbacks the Rust archive reads through. `context` is always the
// RandomAccessReader the archive was opened with, whose ownership passed to the
// Rust archive; ReleaseReader is what eventually destroys it.
//
// They are plain C++ functions, not `extern "C"`: Crubit lowers the Rust side's
// `unsafe extern "C" fn` parameters to ordinary function-pointer types, and
// their signatures use only C-ABI-compatible types.
//
// They are noexcept because an exception unwinding into Rust is undefined
// behavior. google3 C++ does not throw, so this only turns a contract violation
// somewhere in a reader implementation into a crash instead of silent
// corruption.

int64_t ReadFromReader(void* context, uint64_t offset, uint8_t* buffer,
                       uintptr_t length) noexcept {
  auto* reader = static_cast<RandomAccessReader*>(context);
  absl::StatusOr<size_t> bytes_read = reader->ReadAt(
      offset, absl::MakeSpan(reinterpret_cast<char*>(buffer), length));
  if (!bytes_read.ok()) {
    internal::SetReaderStatus(std::move(bytes_read).status());
    return -1;
  }
  if (*bytes_read > length) {
    // Passing this on would hand the archive bytes that were never written.
    internal::SetReaderStatus(absl::InternalError(
        absl::StrCat("RandomAccessReader::ReadAt reported reading ",
                     *bytes_read, " bytes into a buffer of ", length)));
    return -1;
  }
  return static_cast<int64_t>(*bytes_read);
}

int64_t SizeOfReader(void* context) noexcept {
  auto* reader = static_cast<RandomAccessReader*>(context);
  absl::StatusOr<uint64_t> size = reader->Size();
  if (!size.ok()) {
    internal::SetReaderStatus(std::move(size).status());
    return -1;
  }
  if (*size > static_cast<uint64_t>(std::numeric_limits<int64_t>::max())) {
    internal::SetReaderStatus(absl::OutOfRangeError(
        absl::StrCat("RandomAccessReader::Size reported ", *size,
                     " bytes, which does not fit in an int64")));
    return -1;
  }
  return static_cast<int64_t>(*size);
}

void ReleaseReader(void* context) noexcept {
  delete static_cast<RandomAccessReader*>(context);
}

}  // namespace

absl::StatusOr<BufferedZipArchive> BufferedZipArchive::NewFromData(
    absl::string_view data) {
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(ToByteSpan(data));
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipArchive archive,
      FromRustBufferedZipArchive(
          rust::BufferedZipArchive::new_from_data(input_data)));
  return BufferedZipArchive(std::move(archive));
}

absl::Status BufferedZipArchive::CheckNone() const {
  return CheckArchiveOpen(archive_);
}

absl::StatusOr<uintptr_t> BufferedZipArchive::GetLength() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return archive_.get_length();
}

absl::StatusOr<std::string> BufferedZipArchive::GetComment() const {
  return GetCommentImpl(CheckNone(), archive_);
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndex(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipFile file,
      FromRustBufferedZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndexRaw(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipFile file,
      FromRustBufferedZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<ZipEntryMetadata> BufferedZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(CheckNone(), archive_, index);
}

bool BufferedZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<FsZipArchive> FsZipArchive::NewFromPath(absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipArchive archive,
      FromRustFsZipArchive(
          rust::FsZipArchive::new_from_path(ToByteSpan(path))));
  return FsZipArchive(std::move(archive));
}

absl::Status FsZipArchive::CheckNone() const {
  return CheckArchiveOpen(archive_);
}

absl::StatusOr<FsZipArchive> FsZipArchive::TryClone() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(rust::FsZipArchive archive,
                        FromRustFsZipArchive(archive_.try_clone()));
  return FsZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> FsZipArchive::GetLength() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return archive_.get_length();
}

absl::StatusOr<std::string> FsZipArchive::GetComment() const {
  return GetCommentImpl(CheckNone(), archive_);
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndex(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(rust::FsZipFile file,
                        FromRustFsZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndexRaw(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(
      rust::FsZipFile file,
      FromRustFsZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<ZipEntryMetadata> FsZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(CheckNone(), archive_, index);
}

bool FsZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<CallbackZipArchive> CallbackZipArchive::NewFromReader(
    std::unique_ptr<RandomAccessReader> reader) {
  if (reader == nullptr) {
    return absl::InvalidArgumentError("Reader must not be null");
  }
  // Ownership moves to the Rust archive here, which calls ReleaseReader exactly
  // once -- including when this call fails, so releasing `reader` is safe even
  // on the error path.
  ABSL_ASSIGN_OR_RETURN(
      rust::CallbackZipArchive archive, internal::ThroughReader([&] {
        return FromRustCallbackZipArchive(
            rust::CallbackZipArchive::new_from_callbacks(
                ReadFromReader, SizeOfReader, ReleaseReader, reader.release()));
      }));
  return CallbackZipArchive(std::move(archive));
}

absl::Status CallbackZipArchive::CheckNone() const {
  return CheckArchiveOpen(archive_);
}

absl::StatusOr<CallbackZipArchive> CallbackZipArchive::TryClone() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(rust::CallbackZipArchive archive,
                        FromRustCallbackZipArchive(archive_.try_clone()));
  return CallbackZipArchive(std::move(archive));
}

absl::StatusOr<CallbackZipArchive> CallbackZipArchive::TryCloneWithReader(
    std::unique_ptr<RandomAccessReader> reader) const {
  if (reader == nullptr) {
    return absl::InvalidArgumentError("Reader must not be null");
  }
  ABSL_RETURN_IF_ERROR(CheckNone());
  // As in NewFromReader, ownership moves to the Rust archive, which releases it
  // even if this call fails.
  ABSL_ASSIGN_OR_RETURN(
      rust::CallbackZipArchive archive, internal::ThroughReader([&] {
        return FromRustCallbackZipArchive(archive_.try_clone_with_callbacks(
            ReadFromReader, SizeOfReader, ReleaseReader, reader.release()));
      }));
  return CallbackZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> CallbackZipArchive::GetLength() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return archive_.get_length();
}

absl::StatusOr<std::string> CallbackZipArchive::GetComment() const {
  return GetCommentImpl(CheckNone(), archive_);
}

absl::StatusOr<ZipFile> CallbackZipArchive::GetFileByIndex(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  // Opening an entry reads its local file header through the reader, so a
  // failure here may be the reader's rather than the archive's.
  ABSL_ASSIGN_OR_RETURN(
      rust::CallbackZipFile file, internal::ThroughReader([&] {
        return FromRustCallbackZipFile(archive_.get_file_by_index(index));
      }));
  return ZipFile::FromCallback(std::move(file));
}

absl::StatusOr<ZipFile> CallbackZipArchive::GetFileByIndexRaw(uintptr_t index) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  ABSL_ASSIGN_OR_RETURN(
      rust::CallbackZipFile file, internal::ThroughReader([&] {
        return FromRustCallbackZipFile(archive_.get_file_by_index_raw(index));
      }));
  return ZipFile::FromCallback(std::move(file));
}

absl::StatusOr<ZipEntryMetadata> CallbackZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(CheckNone(), archive_, index);
}

bool CallbackZipArchive::IsNone() const { return archive_.is_none(); }

absl::StatusOr<ZipArchive> ZipArchive::FromFile(absl::string_view path) {
  ABSL_ASSIGN_OR_RETURN(FsZipArchive archive, FsZipArchive::NewFromPath(path));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<ZipArchive> ZipArchive::FromBuffer(absl::string_view data) {
  ABSL_ASSIGN_OR_RETURN(BufferedZipArchive archive,
                        BufferedZipArchive::NewFromData(data));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<ZipArchive> ZipArchive::FromReader(
    std::unique_ptr<RandomAccessReader> reader) {
  ABSL_ASSIGN_OR_RETURN(CallbackZipArchive archive,
                        CallbackZipArchive::NewFromReader(std::move(reader)));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<ZipArchive> ZipArchive::TryClone() const {
  return std::visit(
      absl::Overload(
          [](const BufferedZipArchive&) -> absl::StatusOr<ZipArchive> {
            return absl::UnimplementedError(
                "TryClone is not supported for archives opened from a buffer");
          },
          [](const FsZipArchive& archive) -> absl::StatusOr<ZipArchive> {
            ABSL_ASSIGN_OR_RETURN(FsZipArchive clone, archive.TryClone());
            return ZipArchive(std::move(clone));
          },
          [](const CallbackZipArchive& archive) -> absl::StatusOr<ZipArchive> {
            ABSL_ASSIGN_OR_RETURN(CallbackZipArchive clone, archive.TryClone());
            return ZipArchive(std::move(clone));
          }),
      archive_);
}

absl::StatusOr<ZipArchive> ZipArchive::TryCloneWithReader(
    std::unique_ptr<RandomAccessReader> reader) const {
  const auto* callback_archive = std::get_if<CallbackZipArchive>(&archive_);
  if (callback_archive == nullptr) {
    return absl::UnimplementedError(
        "TryCloneWithReader is only supported for archives opened from a "
        "reader");
  }
  ABSL_ASSIGN_OR_RETURN(
      CallbackZipArchive clone,
      callback_archive->TryCloneWithReader(std::move(reader)));
  return ZipArchive(std::move(clone));
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
  rust::VecU8 input_data =
      rust::VecU8::copy_from_slice(ToByteSpan(data));
  ABSL_ASSIGN_OR_RETURN(
      rust::BufferedZipStreamReader reader,
      FromRustBufferedZipStreamReader(
          rust::BufferedZipStreamReader::new_from_data(input_data)));
  return BufferedZipStreamReader(std::move(reader));
}

absl::StatusOr<std::optional<ZipFile>> BufferedZipStreamReader::ReadNextFile() {
  return ReadNextFileImpl(reader_, std::nullopt, FromRustBufferedZipFile,
                          ZipFile::FromBuffer);
}

absl::StatusOr<std::optional<ZipFile>>
BufferedZipStreamReader::ReadNextFileWithCompressedSize(
    uint64_t compressed_size) {
  return ReadNextFileImpl(reader_, compressed_size, FromRustBufferedZipFile,
                          ZipFile::FromBuffer);
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
  return ReadNextFileImpl(reader_, std::nullopt, FromRustFsZipFile,
                          ZipFile::FromFile);
}

absl::StatusOr<std::optional<ZipFile>>
FsZipStreamReader::ReadNextFileWithCompressedSize(uint64_t compressed_size) {
  return ReadNextFileImpl(reader_, compressed_size, FromRustFsZipFile,
                          ZipFile::FromFile);
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
