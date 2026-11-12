#include "file.h"

#include <cstddef>
#include <cstdint>
#include <string>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
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
      return absl::UnimplementedError("Unsupported compression method");
  }
  return absl::InvalidArgumentError("Unknown compression method");
}

// The two backends below differ only in the Rust entry type they wrap
// (rust::{Buffered,Fs}ZipFile), so the methods whose bodies would
// otherwise be identical share these helpers.
template <typename RustZipFile>
absl::Status CheckFileOpen(const RustZipFile& zip) {
  if (zip.is_none()) {
    return absl::InternalError("ZipFile object used after being moved-out");
  }
  return absl::OkStatus();
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

template <typename RustZipFile>
absl::StatusOr<RustVecU8Wrapper> GetFileDataImpl(RustZipFile& zip) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return FromRustResultVecU8(zip.get_file_data());
}

template <typename RustZipFile>
absl::StatusOr<RustVecU8Wrapper> GetFileDataImpl(RustZipFile& zip,
                                                 size_t max_size) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return FromRustResultVecU8(zip.get_file_data_with_max_size(max_size));
}

template <typename RustZipFile>
absl::StatusOr<RustVecU8Wrapper> ReadBytesImpl(RustZipFile& zip,
                                               size_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return FromRustResultVecU8(zip.read_bytes(max_bytes));
}

// Rust writes into the caller's buffer through this; the slice borrows it and
// must not outlive the call it is passed to.
rs_std::SliceRef<uint8_t> AsMutableSlice(absl::Span<char> buf) {
  return rs_std::SliceRef<uint8_t>(reinterpret_cast<uint8_t*>(buf.data()),
                                   buf.size());
}

template <typename RustZipFile>
absl::StatusOr<size_t> ReadIntoImpl(RustZipFile& zip, absl::Span<char> buf) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  ABSL_ASSIGN_OR_RETURN(const uint64_t read,
                        FromRustResultU64(zip.read_into(AsMutableSlice(buf))));
  return static_cast<size_t>(read);
}

template <typename RustZipFile>
absl::StatusOr<uint64_t> SkipImpl(RustZipFile& zip, uint64_t max_bytes) {
  ABSL_RETURN_IF_ERROR(CheckFileOpen(zip));
  return FromRustResultU64(zip.skip(max_bytes));
}

}  // namespace

bool ZipEntryMetadata::IsNone() const { return metadata_.is_none(); }
absl::StatusOr<bool> ZipEntryMetadata::IsFile() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.is_file();
}
absl::StatusOr<bool> ZipEntryMetadata::IsDir() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.is_dir();
}
absl::StatusOr<bool> ZipEntryMetadata::IsEncrypted() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.is_encrypted();
}
absl::StatusOr<std::string> ZipEntryMetadata::GetFileName() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 name_vec = metadata_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> ZipEntryMetadata::GetCompressionMethod()
    const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return ToSecurityZipCompressionMethod(metadata_.get_compression_method());
}
absl::StatusOr<std::string> ZipEntryMetadata::GetComment() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 comment_vec = metadata_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}
absl::StatusOr<uint64_t> ZipEntryMetadata::GetUncompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_uncompressed_size();
}
absl::StatusOr<uint64_t> ZipEntryMetadata::GetCompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_compressed_size();
}
absl::StatusOr<uint32_t> ZipEntryMetadata::GetCrc32() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_crc32();
}
absl::StatusOr<uint16_t> ZipEntryMetadata::GetLastModifiedDate() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_last_modified_date();
}
absl::StatusOr<uint16_t> ZipEntryMetadata::GetLastModifiedTime() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_last_modified_time();
}
absl::StatusOr<uint32_t> ZipEntryMetadata::GetUnixMode() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_unix_mode();
}
absl::StatusOr<uint64_t> ZipEntryMetadata::GetHeaderStart() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return metadata_.get_header_start();
}

absl::Status ZipEntryMetadata::CheckNone() const {
  if (IsNone()) {
    return absl::FailedPreconditionError(
        "ZipEntryMetadata object used after being moved-out");
  }
  return absl::OkStatus();
}

absl::Status BufferedZipFile::CheckNone() const { return CheckFileOpen(zip_); }

bool BufferedZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> BufferedZipFile::IsFile() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.is_file();
}
absl::StatusOr<bool> BufferedZipFile::IsDir() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.is_dir();
}
absl::StatusOr<std::string> BufferedZipFile::GetFileName() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 name_vec = zip_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> BufferedZipFile::GetCompressionMethod()
    const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return ToSecurityZipCompressionMethod(zip_.get_compression_method());
}
absl::StatusOr<std::string> BufferedZipFile::GetComment() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 comment_vec = zip_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}
absl::StatusOr<uint64_t> BufferedZipFile::GetUncompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_uncompressed_size();
}
absl::StatusOr<uint64_t> BufferedZipFile::GetCompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_compressed_size();
}
absl::StatusOr<uint32_t> BufferedZipFile::GetCrc32() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_crc32();
}
absl::StatusOr<uint16_t> BufferedZipFile::GetLastModifiedDate() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_last_modified_date();
}
absl::StatusOr<uint16_t> BufferedZipFile::GetLastModifiedTime() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_last_modified_time();
}
absl::StatusOr<uint32_t> BufferedZipFile::GetUnixMode() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_unix_mode();
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetExtraData() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return RustVecU8Wrapper(zip_.get_extra_data());
}
absl::StatusOr<uint64_t> BufferedZipFile::GetHeaderStart() const {
  return GetHeaderStartImpl(zip_);
}
absl::StatusOr<uint64_t> BufferedZipFile::GetDataStart() const {
  return GetDataStartImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetFileData() {
  return GetFileDataImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetFileData(size_t max_size) {
  return GetFileDataImpl(zip_, max_size);
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::ReadBytes(size_t max_bytes) {
  return ReadBytesImpl(zip_, max_bytes);
}
absl::StatusOr<size_t> BufferedZipFile::ReadInto(absl::Span<char> buf) {
  return ReadIntoImpl(zip_, buf);
}
absl::StatusOr<uint64_t> BufferedZipFile::Skip(uint64_t max_bytes) {
  return SkipImpl(zip_, max_bytes);
}

absl::Status FsZipFile::CheckNone() const { return CheckFileOpen(zip_); }

bool FsZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> FsZipFile::IsFile() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.is_file();
}
absl::StatusOr<bool> FsZipFile::IsDir() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.is_dir();
}
absl::StatusOr<std::string> FsZipFile::GetFileName() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 name_vec = zip_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> FsZipFile::GetCompressionMethod() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return ToSecurityZipCompressionMethod(zip_.get_compression_method());
}
absl::StatusOr<std::string> FsZipFile::GetComment() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  rust::VecU8 comment_vec = zip_.get_comment();
  return std::string(
      security::crubit_helpers::StringViewFromVecU8(comment_vec));
}
absl::StatusOr<uint64_t> FsZipFile::GetUncompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_uncompressed_size();
}
absl::StatusOr<uint64_t> FsZipFile::GetCompressedSize() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_compressed_size();
}
absl::StatusOr<uint32_t> FsZipFile::GetCrc32() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_crc32();
}
absl::StatusOr<uint16_t> FsZipFile::GetLastModifiedDate() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_last_modified_date();
}
absl::StatusOr<uint16_t> FsZipFile::GetLastModifiedTime() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_last_modified_time();
}
absl::StatusOr<uint32_t> FsZipFile::GetUnixMode() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return zip_.get_unix_mode();
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetExtraData() const {
  ABSL_RETURN_IF_ERROR(CheckNone());
  return RustVecU8Wrapper(zip_.get_extra_data());
}
absl::StatusOr<uint64_t> FsZipFile::GetHeaderStart() const {
  return GetHeaderStartImpl(zip_);
}
absl::StatusOr<uint64_t> FsZipFile::GetDataStart() const {
  return GetDataStartImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetFileData() {
  return GetFileDataImpl(zip_);
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetFileData(size_t max_size) {
  return GetFileDataImpl(zip_, max_size);
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::ReadBytes(size_t max_bytes) {
  return ReadBytesImpl(zip_, max_bytes);
}
absl::StatusOr<size_t> FsZipFile::ReadInto(absl::Span<char> buf) {
  return ReadIntoImpl(zip_, buf);
}
absl::StatusOr<uint64_t> FsZipFile::Skip(uint64_t max_bytes) {
  return SkipImpl(zip_, max_bytes);
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
