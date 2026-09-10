#include "converters.h"

#include <cstdint>
#include <string>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "reader.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "support/rs_std/result.h"
#include "support/rs_std/unit.h"

namespace security::zip {

namespace {

std::string FromRustVecU8(const rust::VecU8& vec) {
  return std::string(security::crubit_helpers::StringViewFromVecU8(vec));
}

absl::StatusCode ZipErrorCodeToStatusCode(rust::ZipErrorCode code) {
  switch (code.tag) {
    case rust::ZipErrorCode::Tag::InvalidArgument:
      return absl::StatusCode::kInvalidArgument;
    case rust::ZipErrorCode::Tag::DeadlineExceeded:
      return absl::StatusCode::kDeadlineExceeded;
    case rust::ZipErrorCode::Tag::NotFound:
      return absl::StatusCode::kNotFound;
    case rust::ZipErrorCode::Tag::AlreadyExists:
      return absl::StatusCode::kAlreadyExists;
    case rust::ZipErrorCode::Tag::PermissionDenied:
      return absl::StatusCode::kPermissionDenied;
    case rust::ZipErrorCode::Tag::FailedPrecondition:
      return absl::StatusCode::kFailedPrecondition;
    case rust::ZipErrorCode::Tag::OutOfRange:
      return absl::StatusCode::kOutOfRange;
    case rust::ZipErrorCode::Tag::Unimplemented:
      return absl::StatusCode::kUnimplemented;
    case rust::ZipErrorCode::Tag::Internal:
      return absl::StatusCode::kInternal;
  }
  return absl::StatusCode::kInternal;
}

absl::Status ZipErrorToStatus(rust::ZipError err) {
  absl::Status status(ZipErrorCodeToStatusCode(err.code),
                      FromRustVecU8(err.message));
  if (err.is_reader_callback_failure()) {
    // Lets internal::ThroughReader tell an error caused by a failed
    // RandomAccessReader callback, for which the reader's own status is the
    // better report, from one the archive raised on its own.
    return internal::MarkReaderCallbackFailure(std::move(status));
  }
  return status;
}

template <typename T>
absl::StatusOr<T> FromRustResult(
    rs_std::Result<T, rust::ZipError>&& result) {
  if (!result.has_value()) {
    return ZipErrorToStatus(std::move(result).err());
  }
  return std::move(result).value();
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
    rs_std::Result<rs_std::unit_t, rust::ZipError> result_unit) {
  if (!result_unit.has_value()) {
    return ZipErrorToStatus(std::move(result_unit).err());
  }
  return absl::OkStatus();
}

absl::StatusOr<uint64_t> FromRustResultU64(
    rs_std::Result<uint64_t, rust::ZipError> result_u64) {
  return FromRustResult(std::move(result_u64));
}

absl::StatusOr<rust::BufferedZipArchive> FromRustBufferedZipArchive(
    rs_std::Result<rust::BufferedZipArchive, rust::ZipError>
        result_buffered_zip_archive) {
  return FromRustResult(std::move(result_buffered_zip_archive));
}

absl::StatusOr<rust::FsZipArchive> FromRustFsZipArchive(
    rs_std::Result<rust::FsZipArchive, rust::ZipError>
        result_fs_zip_archive) {
  return FromRustResult(std::move(result_fs_zip_archive));
}

absl::StatusOr<rust::CallbackZipArchive> FromRustCallbackZipArchive(
    rs_std::Result<rust::CallbackZipArchive, rust::ZipError>
        result_callback_zip_archive) {
  return FromRustResult(std::move(result_callback_zip_archive));
}

absl::StatusOr<rust::BufferedZipFile> FromRustBufferedZipFile(
    rs_std::Result<rust::BufferedZipFile, rust::ZipError>
        result_buffered_zip_file) {
  return FromRustResult(std::move(result_buffered_zip_file));
}

absl::StatusOr<rust::FsZipFile> FromRustFsZipFile(
    rs_std::Result<rust::FsZipFile, rust::ZipError>
        result_fs_zip_file) {
  return FromRustResult(std::move(result_fs_zip_file));
}

absl::StatusOr<rust::CallbackZipFile> FromRustCallbackZipFile(
    rs_std::Result<rust::CallbackZipFile, rust::ZipError>
        result_callback_zip_file) {
  return FromRustResult(std::move(result_callback_zip_file));
}

absl::StatusOr<rust::ZipEntryMetadata> FromRustZipEntryMetadata(
    rs_std::Result<rust::ZipEntryMetadata, rust::ZipError>
        result_zip_entry_metadata) {
  return FromRustResult(std::move(result_zip_entry_metadata));
}

absl::StatusOr<rust::BufferedZipWriter> FromRustBufferedZipWriter(
    rs_std::Result<rust::BufferedZipWriter, rust::ZipError>
        result_buffered_zip_writer) {
  return FromRustResult(std::move(result_buffered_zip_writer));
}

absl::StatusOr<rust::FsZipWriter> FromRustFsZipWriter(
    rs_std::Result<rust::FsZipWriter, rust::ZipError>
        result_fs_zip_writer) {
  return FromRustResult(std::move(result_fs_zip_writer));
}

absl::StatusOr<rust::BufferedZipStreamReader>
FromRustBufferedZipStreamReader(
    rs_std::Result<rust::BufferedZipStreamReader, rust::ZipError>
        result_buffered_zip_stream_reader) {
  return FromRustResult(std::move(result_buffered_zip_stream_reader));
}

absl::StatusOr<rust::FsZipStreamReader> FromRustFsZipStreamReader(
    rs_std::Result<rust::FsZipStreamReader, rust::ZipError>
        result_fs_zip_stream_reader) {
  return FromRustResult(std::move(result_fs_zip_stream_reader));
}

absl::StatusOr<rust::FsZipStreamWriter> FromRustFsZipStreamWriter(
    rs_std::Result<rust::FsZipStreamWriter, rust::ZipError>
        result_fs_zip_stream_writer) {
  return FromRustResult(std::move(result_fs_zip_stream_writer));
}

}  // namespace security::zip
