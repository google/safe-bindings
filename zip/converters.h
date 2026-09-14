#ifndef SECURITY_ZIP_CONVERTERS_H_
#define SECURITY_ZIP_CONVERTERS_H_

#include <cstdint>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/base/attributes.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

// A wrapper around rust::VecU8 that provides an interface to view the
// underlying data as an absl::string_view.
class RustVecU8Wrapper {
 public:
  RustVecU8Wrapper() = default;
  explicit RustVecU8Wrapper(rust::VecU8 vec_u8)
      : vec_u8_(std::move(vec_u8)) {}

  absl::string_view AsStringView() const ABSL_ATTRIBUTE_LIFETIME_BOUND {
    return security::crubit_helpers::StringViewFromVecU8(vec_u8_);
  }

 private:
  rust::VecU8 vec_u8_;
};

absl::StatusOr<RustVecU8Wrapper> FromRustResultVecU8(
    rs_std::Result<rust::VecU8, rust::ZipError> result_vec_u8);
absl::Status FromRustResultUnit(
    rs_std::Result<uint8_t, rust::ZipError> result_unit);

absl::StatusOr<rust::BufferedZipArchive> FromRustBufferedZipArchive(
    rs_std::Result<rust::BufferedZipArchive, rust::ZipError>
        result_buffered_zip_archive);
absl::StatusOr<rust::FsZipArchive> FromRustFsZipArchive(
    rs_std::Result<rust::FsZipArchive, rust::ZipError>
        result_fs_zip_archive);

absl::StatusOr<rust::BufferedZipFile> FromRustBufferedZipFile(
    rs_std::Result<rust::BufferedZipFile, rust::ZipError>
        result_buffered_zip_file);
absl::StatusOr<rust::FsZipFile> FromRustFsZipFile(
    rs_std::Result<rust::FsZipFile, rust::ZipError>
        result_fs_zip_file);

absl::StatusOr<rust::BufferedZipWriter> FromRustBufferedZipWriter(
    rs_std::Result<rust::BufferedZipWriter, rust::ZipError>
        result_buffered_zip_writer);
absl::StatusOr<rust::FsZipWriter> FromRustFsZipWriter(
    rs_std::Result<rust::FsZipWriter, rust::ZipError>
        result_fs_zip_writer);

}  // namespace security::zip

#endif  // SECURITY_ZIP_CONVERTERS_H_
