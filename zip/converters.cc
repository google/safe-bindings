#include "converters.h"

#include <string>
#include <utility>

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
    zip_wrapper::ResultVecU8 result_vec_u8) {
  if (result_vec_u8.is_err()) {
    return RustErrorToStatus(std::move(result_vec_u8).unwrap_err(),
                             RustCallSite::kResultVecU8);
  }
  return RustVecU8Wrapper(std::move(result_vec_u8).unwrap());
}

absl::Status FromRustResultUnit(zip_wrapper::ResultUnit result_unit) {
  if (result_unit.is_err()) {
    return RustErrorToStatus(std::move(result_unit).unwrap_err(),
                             RustCallSite::kResultUnit);
  }
  return absl::OkStatus();
}

absl::StatusOr<zip_wrapper::BufferedZipArchive> FromRustBufferedZipArchive(
    zip_wrapper::ResultBufferedZipArchive result_buffered_zip_archive) {
  if (result_buffered_zip_archive.is_err()) {
    return RustErrorToStatus(
        std::move(result_buffered_zip_archive).unwrap_err(),
        RustCallSite::kBufferedZipArchive);
  }
  return std::move(result_buffered_zip_archive).unwrap();
}

absl::StatusOr<zip_wrapper::FsZipArchive> FromRustFsZipArchive(
    zip_wrapper::ResultFsZipArchive result_fs_zip_archive) {
  if (result_fs_zip_archive.is_err()) {
    return RustErrorToStatus(std::move(result_fs_zip_archive).unwrap_err(),
                             RustCallSite::kFsZipArchive);
  }
  return std::move(result_fs_zip_archive).unwrap();
}

absl::StatusOr<zip_wrapper::BufferedZipFile> FromRustBufferedZipFile(
    zip_wrapper::ResultBufferedZipFile result_buffered_zip_file) {
  if (result_buffered_zip_file.is_err()) {
    return RustErrorToStatus(std::move(result_buffered_zip_file).unwrap_err(),
                             RustCallSite::kBufferedZipFile);
  }
  return std::move(result_buffered_zip_file).unwrap();
}

absl::StatusOr<zip_wrapper::FsZipFile> FromRustFsZipFile(
    zip_wrapper::ResultFsZipFile result_fs_zip_file) {
  if (result_fs_zip_file.is_err()) {
    return RustErrorToStatus(std::move(result_fs_zip_file).unwrap_err(),
                             RustCallSite::kFsZipFile);
  }
  return std::move(result_fs_zip_file).unwrap();
}

absl::StatusOr<zip_wrapper::BufferedZipWriter> FromRustBufferedZipWriter(
    zip_wrapper::ResultBufferedZipWriter result_buffered_zip_writer) {
  if (result_buffered_zip_writer.is_err()) {
    return RustErrorToStatus(std::move(result_buffered_zip_writer).unwrap_err(),
                             RustCallSite::kBufferedZipWriter);
  }
  return std::move(result_buffered_zip_writer).unwrap();
}

absl::StatusOr<zip_wrapper::FsZipWriter> FromRustFsZipWriter(
    zip_wrapper::ResultFsZipWriter result_fs_zip_writer) {
  if (result_fs_zip_writer.is_err()) {
    return RustErrorToStatus(std::move(result_fs_zip_writer).unwrap_err(),
                             RustCallSite::kFsZipWriter);
  }
  return std::move(result_fs_zip_writer).unwrap();
}

}  // namespace security::zip
