#include "file.h"

#include <cstddef>
#include <cstdint>
#include <string>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
#include "reader.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "support/rs_std/slice_ref.h"
namespace security::zip {

namespace {

absl::StatusOr<CompressionMethod> ToSecurityZipCompressionMethod(
    rust::CompressionMethod rust_method) {
  switch (rust_method.tag) {
    case rust::CompressionMethod::Tag::Stored:
      return CompressionMethod::kStored;
    case rust::CompressionMethod::Tag::Deflated:
      return CompressionMethod::kDeflated;
    case rust::CompressionMethod::Tag::Bzip2:
      return CompressionMethod::kBzip2;
    case rust::CompressionMethod::Tag::Zstd:
      return CompressionMethod::kZstd;
    case rust::CompressionMethod::Tag::Lzma:
      return CompressionMethod::kLzma;
    case rust::CompressionMethod::Tag::Xz:
      return CompressionMethod::kXz;
    case rust::CompressionMethod::Tag::Unsupported:
      return absl::InvalidArgumentError("Unsupported compression method");
  }
  return absl::InvalidArgumentError("Unknown compression method");
}

template <typename RustZipFile>
absl::Status CheckFileOpen(const RustZipFile& zip) {
  if (zip.is_none()) {
    return absl::InternalError("ZipFile object used after being moved-out");
  }
  return absl::OkStatus();
}

template <typename RustZipFile>
absl::StatusOr<bool> IsFileImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.is_file();
}

template <typename RustZipFile>
absl::StatusOr<bool> IsDirImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.is_dir();
}

template <typename RustZipFile>
absl::StatusOr<std::string> GetFileNameImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  rust::VecU8 name_vec = zip.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}

template <typename RustZipFile>
absl::StatusOr<CompressionMethod> GetCompressionMethodImpl(
    const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return ToSecurityZipCompressionMethod(zip.get_compression_method());
}

template <typename RustZipFile>
absl::StatusOr<std::string> GetCommentImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  rust::VecU8 comment_vec = zip.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}

template <typename RustZipFile>
absl::StatusOr<uint64_t> GetUncompressedSizeImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_uncompressed_size();
}

template <typename RustZipFile>
absl::StatusOr<uint64_t> GetCompressedSizeImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_compressed_size();
}

template <typename RustZipFile>
absl::StatusOr<uint32_t> GetCrc32Impl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_crc32();
}

template <typename RustZipFile>
absl::StatusOr<uint16_t> GetLastModifiedDateImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_last_modified_date();
}

template <typename RustZipFile>
absl::StatusOr<uint16_t> GetLastModifiedTimeImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_last_modified_time();
}

template <typename RustZipFile>
absl::StatusOr<uint32_t> GetUnixModeImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_unix_mode();
}

template <typename RustZipFile>
absl::StatusOr<RustVecU8Wrapper> GetExtraDataImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return RustVecU8Wrapper(zip.get_extra_data());
}

template <typename RustZipFile>
absl::StatusOr<uint64_t> GetHeaderStartImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_header_start();
}

template <typename RustZipFile>
absl::StatusOr<uint64_t> GetDataStartImpl(const RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return zip.get_data_start();
}

// Rust writes into the caller's buffer through this; the slice borrows it and
// must not outlive the call it is passed to.
rs_std::SliceRef<uint8_t> AsMutableSlice(absl::Span<char> buf) {
  return rs_std::SliceRef<uint8_t>(
      absl::Span<uint8_t>(reinterpret_cast<uint8_t*>(buf.data()), buf.size()));
}

// Shared by the backends' ReadInto and Skip, whose bodies are otherwise
// identical. `Zip` is a Rust entry type
// (rust::{Buffered,Fs,Callback}ZipFile).
template <typename Zip>
absl::StatusOr<size_t> ReadIntoImpl(Zip& zip, absl::Span<char> buf) {
  ABSL_ASSIGN_OR_RETURN(const uint64_t read,
                        FromRustResultU64(zip.read_into(AsMutableSlice(buf))));
  return static_cast<size_t>(read);
}

template <typename Zip>
absl::StatusOr<uint64_t> SkipImpl(Zip& zip, uint64_t max_bytes) {
  return FromRustResultU64(zip.skip(max_bytes));
}

}  // namespace

bool ZipEntryMetadata::IsNone() const { return metadata_.is_none(); }
bool ZipEntryMetadata::IsFile() const { return metadata_.is_file(); }
bool ZipEntryMetadata::IsDir() const { return metadata_.is_dir(); }
bool ZipEntryMetadata::IsEncrypted() const { return metadata_.is_encrypted(); }
std::string ZipEntryMetadata::GetFileName() const {
  rust::VecU8 name_vec = metadata_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> ZipEntryMetadata::GetCompressionMethod()
    const {
  if (IsNone()) {
    return absl::FailedPreconditionError(
        "ZipEntryMetadata object used after being moved-out");
  }
  return ToSecurityZipCompressionMethod(metadata_.get_compression_method());
}
std::string ZipEntryMetadata::GetComment() const {
  rust::VecU8 comment_vec = metadata_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}
uint64_t ZipEntryMetadata::GetUncompressedSize() const {
  return metadata_.get_uncompressed_size();
}
uint64_t ZipEntryMetadata::GetCompressedSize() const {
  return metadata_.get_compressed_size();
}
uint32_t ZipEntryMetadata::GetCrc32() const { return metadata_.get_crc32(); }
uint16_t ZipEntryMetadata::GetLastModifiedDate() const {
  return metadata_.get_last_modified_date();
}
uint16_t ZipEntryMetadata::GetLastModifiedTime() const {
  return metadata_.get_last_modified_time();
}
uint32_t ZipEntryMetadata::GetUnixMode() const {
  return metadata_.get_unix_mode();
}
uint64_t ZipEntryMetadata::GetHeaderStart() const {
  return metadata_.get_header_start();
}

absl::Status BufferedZipFile::CheckNone() const { return CheckFileOpen(zip_); }
bool BufferedZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> BufferedZipFile::IsFile() const {
  return IsFileImpl(zip_);
}
absl::StatusOr<bool> BufferedZipFile::IsDir() const { return IsDirImpl(zip_); }
absl::StatusOr<std::string> BufferedZipFile::GetFileName() const {
  return GetFileNameImpl(zip_);
}
absl::StatusOr<CompressionMethod> BufferedZipFile::GetCompressionMethod()
    const {
  return GetCompressionMethodImpl(zip_);
}
absl::StatusOr<std::string> BufferedZipFile::GetComment() const {
  return GetCommentImpl(zip_);
}
absl::StatusOr<uint64_t> BufferedZipFile::GetUncompressedSize() const {
  return GetUncompressedSizeImpl(zip_);
}
absl::StatusOr<uint64_t> BufferedZipFile::GetCompressedSize() const {
  return GetCompressedSizeImpl(zip_);
}
absl::StatusOr<uint32_t> BufferedZipFile::GetCrc32() const {
  return GetCrc32Impl(zip_);
}
absl::StatusOr<uint16_t> BufferedZipFile::GetLastModifiedDate() const {
  return GetLastModifiedDateImpl(zip_);
}
absl::StatusOr<uint16_t> BufferedZipFile::GetLastModifiedTime() const {
  return GetLastModifiedTimeImpl(zip_);
}
absl::StatusOr<uint32_t> BufferedZipFile::GetUnixMode() const {
  return GetUnixModeImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetExtraData() const {
  return GetExtraDataImpl(zip_);
}
absl::StatusOr<uint64_t> BufferedZipFile::GetHeaderStart() const {
  return GetHeaderStartImpl(zip_);
}
absl::StatusOr<uint64_t> BufferedZipFile::GetDataStart() const {
  return GetDataStartImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetFileData() {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data());
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetFileData(size_t max_size) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data_with_max_size(max_size));
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::ReadBytes(size_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.read_bytes(max_bytes));
}
absl::StatusOr<size_t> BufferedZipFile::ReadInto(absl::Span<char> buf) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return ReadIntoImpl(zip_, buf);
}
absl::StatusOr<uint64_t> BufferedZipFile::Skip(uint64_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return SkipImpl(zip_, max_bytes);
}

absl::Status FsZipFile::CheckNone() const { return CheckFileOpen(zip_); }
bool FsZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> FsZipFile::IsFile() const { return IsFileImpl(zip_); }
absl::StatusOr<bool> FsZipFile::IsDir() const { return IsDirImpl(zip_); }
absl::StatusOr<std::string> FsZipFile::GetFileName() const {
  return GetFileNameImpl(zip_);
}
absl::StatusOr<CompressionMethod> FsZipFile::GetCompressionMethod() const {
  return GetCompressionMethodImpl(zip_);
}
absl::StatusOr<std::string> FsZipFile::GetComment() const {
  return GetCommentImpl(zip_);
}
absl::StatusOr<uint64_t> FsZipFile::GetUncompressedSize() const {
  return GetUncompressedSizeImpl(zip_);
}
absl::StatusOr<uint64_t> FsZipFile::GetCompressedSize() const {
  return GetCompressedSizeImpl(zip_);
}
absl::StatusOr<uint32_t> FsZipFile::GetCrc32() const {
  return GetCrc32Impl(zip_);
}
absl::StatusOr<uint16_t> FsZipFile::GetLastModifiedDate() const {
  return GetLastModifiedDateImpl(zip_);
}
absl::StatusOr<uint16_t> FsZipFile::GetLastModifiedTime() const {
  return GetLastModifiedTimeImpl(zip_);
}
absl::StatusOr<uint32_t> FsZipFile::GetUnixMode() const {
  return GetUnixModeImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetExtraData() const {
  return GetExtraDataImpl(zip_);
}
absl::StatusOr<uint64_t> FsZipFile::GetHeaderStart() const {
  return GetHeaderStartImpl(zip_);
}
absl::StatusOr<uint64_t> FsZipFile::GetDataStart() const {
  return GetDataStartImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetFileData() {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data());
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetFileData(size_t max_size) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data_with_max_size(max_size));
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::ReadBytes(size_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.read_bytes(max_bytes));
}
absl::StatusOr<size_t> FsZipFile::ReadInto(absl::Span<char> buf) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return ReadIntoImpl(zip_, buf);
}
absl::StatusOr<uint64_t> FsZipFile::Skip(uint64_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return SkipImpl(zip_, max_bytes);
}

absl::Status CallbackZipFile::CheckNone() const { return CheckFileOpen(zip_); }
bool CallbackZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> CallbackZipFile::IsFile() const {
  return IsFileImpl(zip_);
}
absl::StatusOr<bool> CallbackZipFile::IsDir() const { return IsDirImpl(zip_); }
absl::StatusOr<std::string> CallbackZipFile::GetFileName() const {
  return GetFileNameImpl(zip_);
}
absl::StatusOr<CompressionMethod> CallbackZipFile::GetCompressionMethod()
    const {
  return GetCompressionMethodImpl(zip_);
}
absl::StatusOr<std::string> CallbackZipFile::GetComment() const {
  return GetCommentImpl(zip_);
}
absl::StatusOr<uint64_t> CallbackZipFile::GetUncompressedSize() const {
  return GetUncompressedSizeImpl(zip_);
}
absl::StatusOr<uint64_t> CallbackZipFile::GetCompressedSize() const {
  return GetCompressedSizeImpl(zip_);
}
absl::StatusOr<uint32_t> CallbackZipFile::GetCrc32() const {
  return GetCrc32Impl(zip_);
}
absl::StatusOr<uint16_t> CallbackZipFile::GetLastModifiedDate() const {
  return GetLastModifiedDateImpl(zip_);
}
absl::StatusOr<uint16_t> CallbackZipFile::GetLastModifiedTime() const {
  return GetLastModifiedTimeImpl(zip_);
}
absl::StatusOr<uint32_t> CallbackZipFile::GetUnixMode() const {
  return GetUnixModeImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> CallbackZipFile::GetExtraData() const {
  return GetExtraDataImpl(zip_);
}
absl::StatusOr<uint64_t> CallbackZipFile::GetHeaderStart() const {
  return GetHeaderStartImpl(zip_);
}
absl::StatusOr<uint64_t> CallbackZipFile::GetDataStart() const {
  return GetDataStartImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> CallbackZipFile::GetFileData() {
  ABSL_RETURN_IF_ERROR(CheckNone());
  // Unlike the other two backends this one reads through the caller's
  // RandomAccessReader, whose failures reach us as a generic bridge error; see
  // security/zip/reader.h.
  return internal::ThroughReader(
      [&] { return FromRustResultVecU8(zip_.get_file_data()); });
}
absl::StatusOr<RustVecU8Wrapper> CallbackZipFile::GetFileData(size_t max_size) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return internal::ThroughReader([&] {
    return FromRustResultVecU8(zip_.get_file_data_with_max_size(max_size));
  });
}
absl::StatusOr<RustVecU8Wrapper> CallbackZipFile::ReadBytes(size_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return internal::ThroughReader(
      [&] { return FromRustResultVecU8(zip_.read_bytes(max_bytes)); });
}
absl::StatusOr<size_t> CallbackZipFile::ReadInto(absl::Span<char> buf) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return internal::ThroughReader([&] { return ReadIntoImpl(zip_, buf); });
}
absl::StatusOr<uint64_t> CallbackZipFile::Skip(uint64_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return internal::ThroughReader([&] { return SkipImpl(zip_, max_bytes); });
}

absl::StatusOr<bool> ZipFile::IsFile() const {
  return std::visit([](const auto& zip) { return zip.IsFile(); }, zip_);
}
absl::StatusOr<bool> ZipFile::IsDir() const {
  return std::visit([](const auto& zip) { return zip.IsDir(); }, zip_);
}
bool ZipFile::IsNone() const {
  return std::visit([](const auto& zip) { return zip.IsNone(); }, zip_);
}
absl::StatusOr<std::string> ZipFile::GetFileName() const {
  return std::visit([](const auto& zip) { return zip.GetFileName(); }, zip_);
}
absl::StatusOr<CompressionMethod> ZipFile::GetCompressionMethod() const {
  return std::visit([](const auto& zip) { return zip.GetCompressionMethod(); },
                    zip_);
}
absl::StatusOr<std::string> ZipFile::GetComment() const {
  return std::visit([](const auto& zip) { return zip.GetComment(); }, zip_);
}
absl::StatusOr<uint64_t> ZipFile::GetUncompressedSize() const {
  return std::visit([](const auto& zip) { return zip.GetUncompressedSize(); },
                    zip_);
}
absl::StatusOr<uint64_t> ZipFile::GetCompressedSize() const {
  return std::visit([](const auto& zip) { return zip.GetCompressedSize(); },
                    zip_);
}
absl::StatusOr<uint32_t> ZipFile::GetCrc32() const {
  return std::visit([](const auto& zip) { return zip.GetCrc32(); }, zip_);
}
absl::StatusOr<uint16_t> ZipFile::GetLastModifiedDate() const {
  return std::visit([](const auto& zip) { return zip.GetLastModifiedDate(); },
                    zip_);
}
absl::StatusOr<uint16_t> ZipFile::GetLastModifiedTime() const {
  return std::visit([](const auto& zip) { return zip.GetLastModifiedTime(); },
                    zip_);
}
absl::StatusOr<uint32_t> ZipFile::GetUnixMode() const {
  return std::visit([](const auto& zip) { return zip.GetUnixMode(); }, zip_);
}
absl::StatusOr<RustVecU8Wrapper> ZipFile::GetExtraData() const {
  return std::visit([](const auto& zip) { return zip.GetExtraData(); }, zip_);
}
absl::StatusOr<uint64_t> ZipFile::GetHeaderStart() const {
  return std::visit([](const auto& zip) { return zip.GetHeaderStart(); }, zip_);
}
absl::StatusOr<uint64_t> ZipFile::GetDataStart() const {
  return std::visit([](const auto& zip) { return zip.GetDataStart(); }, zip_);
}
absl::StatusOr<RustVecU8Wrapper> ZipFile::GetFileData() {
  return std::visit([](auto& zip) { return zip.GetFileData(); }, zip_);
}
absl::StatusOr<RustVecU8Wrapper> ZipFile::GetFileData(size_t max_size) {
  return std::visit([max_size](auto& zip) { return zip.GetFileData(max_size); },
                    zip_);
}
absl::StatusOr<RustVecU8Wrapper> ZipFile::ReadBytes(size_t max_bytes) {
  return std::visit([max_bytes](auto& zip) { return zip.ReadBytes(max_bytes); },
                    zip_);
}
absl::StatusOr<size_t> ZipFile::ReadInto(absl::Span<char> buf) {
  return std::visit([buf](auto& zip) { return zip.ReadInto(buf); }, zip_);
}
absl::StatusOr<uint64_t> ZipFile::Skip(uint64_t max_bytes) {
  return std::visit([max_bytes](auto& zip) { return zip.Skip(max_bytes); },
                    zip_);
}

}  // namespace security::zip
