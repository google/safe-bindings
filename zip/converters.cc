#include "converters.h"

#include <cstdint>
#include <string>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "rust/zip_wrapper.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/match.h"

namespace security::zip {

namespace {

enum class RustCallSite {
  kBufferedZipArchive,
  kFsZipArchive,
  kBufferedZipFile,
  kFsZipFile,
  kBufferedZipWriter,
  kFsZipWriter,
  kResultVecU8,
  kResultUnit,
};

std::string FromRustVecU8(const zip_wrapper::VecU8& vec) {
  return std::string(security::crubit_helpers::StringViewFromVecU8(vec));
}

// NOTE: b/351976355 - Send the code from Rust to return a more
// concrete & informative status.
absl::Status RustErrorToStatus(std::string err_str, RustCallSite call_site) {
  switch (call_site) {
    // From: read.rs:BufferedZipArchive::new_from_data
    case RustCallSite::kBufferedZipArchive:
    // From: read.rs:FsZipArchive::new_from_path
    case RustCallSite::kFsZipArchive:
      return absl::InvalidArgumentError(err_str);
    // From: read.rs:BufferedZipArchive::get_file_by_index
    case RustCallSite::kBufferedZipFile:
    // From: read.rs:FsZipArchive::get_file_by_index
    case RustCallSite::kFsZipFile:
      return absl::OutOfRangeError(err_str);
    // From: write.rs:BufferedZipWriter::new_from_data
    case RustCallSite::kBufferedZipWriter:
    // From: write.rs:FsZipWriter::new_from_path
    case RustCallSite::kFsZipWriter:
      return absl::InvalidArgumentError(err_str);
    case RustCallSite::kResultVecU8:
      // From: write.rs:BufferedZipWriter::finish
      if (absl::StrContains(err_str, "writer is not open")) {
        return absl::FailedPreconditionError(err_str);
      }
      return absl::InternalError(err_str);
    case RustCallSite::kResultUnit:
      // From: write.rs:FsZipWriter::finish
      // From: write.rs:add_directory_impl
      // From: write.rs:start_file_impl
      // From: write.rs:write_data_impl
      // From: write.rs:do_copy_impl
      // From: write.rs:write_file_content_impl
      if (absl::StrContains(err_str, "writer is not open")) {
        return absl::FailedPreconditionError(err_str);
      }
      return absl::InternalError(err_str);
    default:
      return absl::InternalError(err_str);
  }
}

}  // namespace

absl::StatusOr<RustVecU8Wrapper> FromRustResultVecU8(
    rs_std::Result<zip_wrapper::VecU8, zip_wrapper::VecU8> result_vec_u8) {
  if (!result_vec_u8.has_value()) {
    return RustErrorToStatus(FromRustVecU8(std::move(result_vec_u8).err()),
                             RustCallSite::kResultVecU8);
  }
  return RustVecU8Wrapper(std::move(result_vec_u8).value());
}

absl::Status FromRustResultUnit(
    rs_std::Result<uint8_t, zip_wrapper::VecU8> result_unit) {
  if (!result_unit.has_value()) {
    return RustErrorToStatus(FromRustVecU8(std::move(result_unit).err()),
                             RustCallSite::kResultUnit);
  }
  return absl::OkStatus();
}

absl::StatusOr<zip_wrapper::BufferedZipArchive> FromRustBufferedZipArchive(
    rs_std::Result<zip_wrapper::BufferedZipArchive, zip_wrapper::VecU8>
        result_buffered_zip_archive) {
  if (!result_buffered_zip_archive.has_value()) {
    return RustErrorToStatus(
        FromRustVecU8(std::move(result_buffered_zip_archive).err()),
        RustCallSite::kBufferedZipArchive);
  }
  return std::move(result_buffered_zip_archive).value();
}

absl::StatusOr<zip_wrapper::FsZipArchive> FromRustFsZipArchive(
    rs_std::Result<zip_wrapper::FsZipArchive, zip_wrapper::VecU8>
        result_fs_zip_archive) {
  if (!result_fs_zip_archive.has_value()) {
    return RustErrorToStatus(
        FromRustVecU8(std::move(result_fs_zip_archive).err()),
        RustCallSite::kFsZipArchive);
  }
  return std::move(result_fs_zip_archive).value();
}

absl::StatusOr<zip_wrapper::BufferedZipFile> FromRustBufferedZipFile(
    rs_std::Result<zip_wrapper::BufferedZipFile, zip_wrapper::VecU8>
        result_buffered_zip_file) {
  if (!result_buffered_zip_file.has_value()) {
    return RustErrorToStatus(
        FromRustVecU8(std::move(result_buffered_zip_file).err()),
        RustCallSite::kBufferedZipFile);
  }
  return std::move(result_buffered_zip_file).value();
}

absl::StatusOr<zip_wrapper::FsZipFile> FromRustFsZipFile(
    rs_std::Result<zip_wrapper::FsZipFile, zip_wrapper::VecU8>
        result_fs_zip_file) {
  if (!result_fs_zip_file.has_value()) {
    return RustErrorToStatus(FromRustVecU8(std::move(result_fs_zip_file).err()),
                             RustCallSite::kFsZipFile);
  }
  return std::move(result_fs_zip_file).value();
}

absl::StatusOr<zip_wrapper::BufferedZipWriter> FromRustBufferedZipWriter(
    rs_std::Result<zip_wrapper::BufferedZipWriter, zip_wrapper::VecU8>
        result_buffered_zip_writer) {
  if (!result_buffered_zip_writer.has_value()) {
    return RustErrorToStatus(
        FromRustVecU8(std::move(result_buffered_zip_writer).err()),
        RustCallSite::kBufferedZipWriter);
  }
  return std::move(result_buffered_zip_writer).value();
}

absl::StatusOr<zip_wrapper::FsZipWriter> FromRustFsZipWriter(
    rs_std::Result<zip_wrapper::FsZipWriter, zip_wrapper::VecU8>
        result_fs_zip_writer) {
  if (!result_fs_zip_writer.has_value()) {
    return RustErrorToStatus(
        FromRustVecU8(std::move(result_fs_zip_writer).err()),
        RustCallSite::kFsZipWriter);
  }
  return std::move(result_fs_zip_writer).value();
}

}  // namespace security::zip
