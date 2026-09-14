#include "converters.h"

#include <cstdint>
#include <string>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"

namespace security::zip {

namespace {

std::string FromRustVecU8(const rust::VecU8& vec) {
  return std::string(security::crubit_helpers::StringViewFromVecU8(vec));
}

absl::Status ZipErrorToStatus(rust::ZipError err) {
  std::string msg = FromRustVecU8(err.message);
  switch (err.code.tag) {
    case rust::ZipErrorCode::Tag::InvalidArgument:
      return absl::InvalidArgumentError(msg);
    case rust::ZipErrorCode::Tag::OutOfRange:
      return absl::OutOfRangeError(msg);
    case rust::ZipErrorCode::Tag::FailedPrecondition:
      return absl::FailedPreconditionError(msg);
    case rust::ZipErrorCode::Tag::Internal:
      return absl::InternalError(msg);
  }
  return absl::InternalError(msg);
}

}  // namespace

absl::StatusOr<RustVecU8Wrapper> FromRustResultVecU8(
    rs_std::Result<rust::VecU8, rust::ZipError> result_vec_u8) {
  if (!result_vec_u8.has_value()) {
    return ZipErrorToStatus(std::move(result_vec_u8).err());
  }
  return RustVecU8Wrapper(std::move(result_vec_u8).value());
}

absl::Status FromRustResultUnit(
    rs_std::Result<uint8_t, rust::ZipError> result_unit) {
  if (!result_unit.has_value()) {
    return ZipErrorToStatus(std::move(result_unit).err());
  }
  return absl::OkStatus();
}

absl::StatusOr<rust::BufferedZipArchive> FromRustBufferedZipArchive(
    rs_std::Result<rust::BufferedZipArchive, rust::ZipError>
        result_buffered_zip_archive) {
  if (!result_buffered_zip_archive.has_value()) {
    return ZipErrorToStatus(std::move(result_buffered_zip_archive).err());
  }
  return std::move(result_buffered_zip_archive).value();
}

absl::StatusOr<rust::FsZipArchive> FromRustFsZipArchive(
    rs_std::Result<rust::FsZipArchive, rust::ZipError>
        result_fs_zip_archive) {
  if (!result_fs_zip_archive.has_value()) {
    return ZipErrorToStatus(std::move(result_fs_zip_archive).err());
  }
  return std::move(result_fs_zip_archive).value();
}

absl::StatusOr<rust::BufferedZipFile> FromRustBufferedZipFile(
    rs_std::Result<rust::BufferedZipFile, rust::ZipError>
        result_buffered_zip_file) {
  if (!result_buffered_zip_file.has_value()) {
    return ZipErrorToStatus(std::move(result_buffered_zip_file).err());
  }
  return std::move(result_buffered_zip_file).value();
}

absl::StatusOr<rust::FsZipFile> FromRustFsZipFile(
    rs_std::Result<rust::FsZipFile, rust::ZipError>
        result_fs_zip_file) {
  if (!result_fs_zip_file.has_value()) {
    return ZipErrorToStatus(std::move(result_fs_zip_file).err());
  }
  return std::move(result_fs_zip_file).value();
}

absl::StatusOr<rust::BufferedZipWriter> FromRustBufferedZipWriter(
    rs_std::Result<rust::BufferedZipWriter, rust::ZipError>
        result_buffered_zip_writer) {
  if (!result_buffered_zip_writer.has_value()) {
    return ZipErrorToStatus(std::move(result_buffered_zip_writer).err());
  }
  return std::move(result_buffered_zip_writer).value();
}

absl::StatusOr<rust::FsZipWriter> FromRustFsZipWriter(
    rs_std::Result<rust::FsZipWriter, rust::ZipError>
        result_fs_zip_writer) {
  if (!result_fs_zip_writer.has_value()) {
    return ZipErrorToStatus(std::move(result_fs_zip_writer).err());
  }
  return std::move(result_fs_zip_writer).value();
}

}  // namespace security::zip
