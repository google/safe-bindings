#include "read.h"

#include <cstdint>
#include <utility>
#include <variant>

#include "converters.h"
#include "file.h"
#include "rust/zip_wrapper.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "third_party/gloop/util/status/status_macros.h"

namespace security::zip {

absl::StatusOr<BufferedZipArchive> BufferedZipArchive::NewFromData(
    absl::string_view data) {
  zip_wrapper::VecU8 input_data =
      zip_wrapper::VecU8::copy_from_slice(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  ASSIGN_OR_RETURN(
      zip_wrapper::BufferedZipArchive archive,
      FromRustBufferedZipArchive(
          zip_wrapper::BufferedZipArchive::new_from_data(input_data)));
  return BufferedZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> BufferedZipArchive::GetLength() {
  return archive_.get_length();
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndex(uintptr_t index) {
  ASSIGN_OR_RETURN(zip_wrapper::BufferedZipFile file,
                   FromRustBufferedZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<ZipFile> BufferedZipArchive::GetFileByIndexRaw(uintptr_t index) {
  ASSIGN_OR_RETURN(
      zip_wrapper::BufferedZipFile file,
      FromRustBufferedZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromBuffer(std::move(file));
}

absl::StatusOr<FsZipArchive> FsZipArchive::NewFromPath(absl::string_view path) {
  ASSIGN_OR_RETURN(
      zip_wrapper::FsZipArchive archive,
      FromRustFsZipArchive(
          zip_wrapper::FsZipArchive::new_from_path(absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(path.data()), path.size()))));
  return FsZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> FsZipArchive::GetLength() {
  return archive_.get_length();
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndex(uintptr_t index) {
  ASSIGN_OR_RETURN(zip_wrapper::FsZipFile file,
                   FromRustFsZipFile(archive_.get_file_by_index(index)));
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<ZipFile> FsZipArchive::GetFileByIndexRaw(uintptr_t index) {
  ASSIGN_OR_RETURN(zip_wrapper::FsZipFile file,
                   FromRustFsZipFile(archive_.get_file_by_index_raw(index)));
  return ZipFile::FromFile(std::move(file));
}

absl::StatusOr<ZipArchive> ZipArchive::FromFile(absl::string_view path) {
  ASSIGN_OR_RETURN(FsZipArchive archive, FsZipArchive::NewFromPath(path));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<ZipArchive> ZipArchive::FromBuffer(absl::string_view data) {
  ASSIGN_OR_RETURN(BufferedZipArchive archive,
                   BufferedZipArchive::NewFromData(data));
  return ZipArchive(std::move(archive));
}

absl::StatusOr<uintptr_t> ZipArchive::GetLength() {
  return std::visit([](auto& arg) { return arg.GetLength(); }, archive_);
}

absl::StatusOr<ZipFile> ZipArchive::GetFileByIndex(uintptr_t index) {
  return std::visit([index](auto& arg) { return arg.GetFileByIndex(index); },
                    archive_);
}

absl::StatusOr<ZipFile> ZipArchive::GetFileByIndexRaw(uintptr_t index) {
  return std::visit([index](auto& arg) { return arg.GetFileByIndexRaw(index); },
                    archive_);
}

}  // namespace security::zip
