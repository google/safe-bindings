#include "file.h"

#include <string>
#include <variant>

#include "crubit_helpers/string_conversions.h"
#include "converters.h"
#include "rust/zip_wrapper.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "third_party/gloop/util/status/status_macros.h"

namespace security::zip {

namespace {

absl::StatusOr<CompressionMethod> ToSecurityZipCompressionMethod(
    zip_wrapper::CompressionMethod rust_method) {
  switch (rust_method.tag) {
    case zip_wrapper::CompressionMethod::Tag::Stored:
      return CompressionMethod::kStored;
    case zip_wrapper::CompressionMethod::Tag::Deflated:
      return CompressionMethod::kDeflated;
    case zip_wrapper::CompressionMethod::Tag::Bzip2:
      return CompressionMethod::kBzip2;
    case zip_wrapper::CompressionMethod::Tag::Zstd:
      return CompressionMethod::kZstd;
    case zip_wrapper::CompressionMethod::Tag::Lzma:
      return CompressionMethod::kLzma;
    case zip_wrapper::CompressionMethod::Tag::Xz:
      return CompressionMethod::kXz;
    case zip_wrapper::CompressionMethod::Tag::Unsupported:
      return absl::InvalidArgumentError("Unsupported compression method");
  }
  return absl::InvalidArgumentError("Unknown compression method");
}

}  // namespace

absl::Status BufferedZipFile::CheckNone() const {
  if (IsNone()) {
    return absl::InternalError("ZipFile object used after being moved-out");
  }
  return absl::OkStatus();
}

bool BufferedZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> BufferedZipFile::IsFile() const {
  RETURN_IF_ERROR(CheckNone());
  return zip_.is_file();
}
absl::StatusOr<bool> BufferedZipFile::IsDir() const {
  RETURN_IF_ERROR(CheckNone());
  return zip_.is_dir();
}
absl::StatusOr<std::string> BufferedZipFile::GetFileName() const {
  RETURN_IF_ERROR(CheckNone());
  zip_wrapper::VecU8 name_vec = zip_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> BufferedZipFile::GetCompressionMethod()
    const {
  RETURN_IF_ERROR(CheckNone());
  return ToSecurityZipCompressionMethod(zip_.get_compression_method());
}
absl::StatusOr<RustVecU8Wrapper> BufferedZipFile::GetFileData() {
  RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data());
}

absl::Status FsZipFile::CheckNone() const {
  if (IsNone()) {
    return absl::InternalError("ZipFile object used after being moved-out");
  }
  return absl::OkStatus();
}

bool FsZipFile::IsNone() const { return zip_.is_none(); }
absl::StatusOr<bool> FsZipFile::IsFile() const {
  RETURN_IF_ERROR(CheckNone());
  return zip_.is_file();
}
absl::StatusOr<bool> FsZipFile::IsDir() const {
  RETURN_IF_ERROR(CheckNone());
  return zip_.is_dir();
}
absl::StatusOr<std::string> FsZipFile::GetFileName() const {
  RETURN_IF_ERROR(CheckNone());
  zip_wrapper::VecU8 name_vec = zip_.get_file_name();
  return std::string(security::crubit_helpers::StringViewFromVecU8(name_vec));
}
absl::StatusOr<CompressionMethod> FsZipFile::GetCompressionMethod() const {
  RETURN_IF_ERROR(CheckNone());
  return ToSecurityZipCompressionMethod(zip_.get_compression_method());
}
absl::StatusOr<RustVecU8Wrapper> FsZipFile::GetFileData() {
  RETURN_IF_ERROR(CheckNone());
  return FromRustResultVecU8(zip_.get_file_data());
}

absl::StatusOr<bool> ZipFile::IsFile() const {
  return std::visit([](auto& zip) { return zip.IsFile(); }, zip_);
}
absl::StatusOr<bool> ZipFile::IsDir() const {
  return std::visit([](auto& zip) { return zip.IsDir(); }, zip_);
}
bool ZipFile::IsNone() const {
  return std::visit([](auto& zip) { return zip.IsNone(); }, zip_);
}
absl::StatusOr<std::string> ZipFile::GetFileName() const {
  return std::visit([](auto& zip) { return zip.GetFileName(); }, zip_);
}
absl::StatusOr<CompressionMethod> ZipFile::GetCompressionMethod() const {
  return std::visit([](auto& zip) { return zip.GetCompressionMethod(); }, zip_);
}
absl::StatusOr<RustVecU8Wrapper> ZipFile::GetFileData() {
  return std::visit([](auto& zip) { return zip.GetFileData(); }, zip_);
}

}  // namespace security::zip
