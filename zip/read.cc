#include "read.h"

#include <cstddef>
#include <cstdint>
#include <limits>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>
#include <vector>

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
#include "absl/types/span.h"

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
// (rust::{Buffered,Fs,Callback}ZipArchive), so the methods whose bodies
// would otherwise be identical share the following helpers.
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

// The three callbacks the Rust archive reads through. `context` is always the
// RandomAccessReader the archive was opened with (encoded as an opaque
// `uintptr_t` token so the Rust crate needs no raw pointers or `unsafe`), whose
// ownership passed to the Rust archive; ReleaseReader is what eventually
// destroys it.
//
// They are plain C++ functions, not `extern "C"`: Crubit lowers the Rust side's
// `extern "C" fn` parameters to ordinary function-reference types, and their
// signatures use only C-ABI-compatible scalar types.
//
// They are noexcept because an exception unwinding into Rust is undefined
// behavior. google3 C++ does not throw, so this only turns a contract violation
// somewhere in a reader implementation into a crash instead of silent
// corruption.
//
// A failing callback records the reader's status in the per-thread slot and
// returns -1. internal::ThroughReader, which wraps every operation that may
// call back, clears the slot before and after the operation and only reports
// the recorded status when the Rust error says a callback failed; see
// security/zip/reader.h for why that distinction matters.

int64_t ReadFromReader(uintptr_t context, uint64_t offset,
                       uintptr_t length) noexcept {
  auto* reader = reinterpret_cast<RandomAccessReader*>(context);
  constexpr size_t kStackBufferSize = 16 * 1024;
  char stack_buffer[kStackBufferSize];
  std::vector<char> heap_buffer;
  absl::Span<char> buffer;
  if (length <= kStackBufferSize) {
    buffer = absl::MakeSpan(stack_buffer, length);
  } else {
    heap_buffer.resize(length);
    buffer = absl::MakeSpan(heap_buffer);
  }
  absl::StatusOr<size_t> bytes_read = reader->ReadAt(offset, buffer);
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
  if (*bytes_read > 0) {
    rust::stage_callback_chunk(
        ToByteSpan(absl::string_view(buffer.data(), *bytes_read)));
  }
  return static_cast<int64_t>(*bytes_read);
}

int64_t SizeOfReader(uintptr_t context) noexcept {
  auto* reader = reinterpret_cast<RandomAccessReader*>(context);
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

void ReleaseReader(uintptr_t context) noexcept {
  delete reinterpret_cast<RandomAccessReader*>(context);
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
                ReadFromReader, SizeOfReader, ReleaseReader,
                reinterpret_cast<uintptr_t>(reader.release())));
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
            ReadFromReader, SizeOfReader, ReleaseReader,
            reinterpret_cast<uintptr_t>(reader.release())));
      }));
  return CallbackZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> CallbackZipArchive::GetLength() const {
  return GetLengthImpl(archive_);
}

absl::StatusOr<std::string> CallbackZipArchive::GetComment() const {
  return GetCommentImpl(archive_);
}

absl::StatusOr<ZipFile> CallbackZipArchive::GetFileByIndex(uintptr_t index) {
  // Opening an entry reads its local file header through the reader, so a
  // failure here may be the reader's rather than the archive's.
  return internal::ThroughReader([&] {
    return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/false,
                              FromRustCallbackZipFile, ZipFile::FromCallback);
  });
}

absl::StatusOr<ZipFile> CallbackZipArchive::GetFileByIndexRaw(uintptr_t index) {
  return internal::ThroughReader([&] {
    return GetFileByIndexImpl(archive_, active_entry_, index, /*raw=*/true,
                              FromRustCallbackZipFile, ZipFile::FromCallback);
  });
}

absl::StatusOr<ZipEntryMetadata> CallbackZipArchive::GetEntryMetadata(
    uintptr_t index) const {
  return GetEntryMetadataImpl(archive_, index);
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
