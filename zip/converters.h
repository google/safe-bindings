#ifndef SECURITY_ZIP_CONVERTERS_H_
#define SECURITY_ZIP_CONVERTERS_H_

#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "rust/zip_wrapper.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

// A wrapper around zip_wrapper::VecU8 that provides an interface to view the
// underlying data as an absl::string_view.
class RustVecU8Wrapper {
 public:
  RustVecU8Wrapper() = default;
  explicit RustVecU8Wrapper(zip_wrapper::VecU8 vec_u8)
      : vec_u8_(std::move(vec_u8)) {}

  absl::string_view AsStringView() const {
    return security::crubit_helpers::StringViewFromVecU8(vec_u8_);
  }

 private:
  zip_wrapper::VecU8 vec_u8_;
};

absl::StatusOr<RustVecU8Wrapper> FromRustResultVecU8(
    rs_std::Result<zip_wrapper::VecU8, zip_wrapper::VecU8> result_vec_u8);
absl::Status FromRustResultUnit(
    rs_std::Result<uint8_t, zip_wrapper::VecU8> result_unit);

absl::StatusOr<zip_wrapper::BufferedZipArchive> FromRustBufferedZipArchive(
    rs_std::Result<zip_wrapper::BufferedZipArchive, zip_wrapper::VecU8>
        result_buffered_zip_archive);
absl::StatusOr<zip_wrapper::FsZipArchive> FromRustFsZipArchive(
    rs_std::Result<zip_wrapper::FsZipArchive, zip_wrapper::VecU8>
        result_fs_zip_archive);

absl::StatusOr<zip_wrapper::BufferedZipFile> FromRustBufferedZipFile(
    rs_std::Result<zip_wrapper::BufferedZipFile, zip_wrapper::VecU8>
        result_buffered_zip_file);
absl::StatusOr<zip_wrapper::FsZipFile> FromRustFsZipFile(
    rs_std::Result<zip_wrapper::FsZipFile, zip_wrapper::VecU8>
        result_fs_zip_file);

absl::StatusOr<zip_wrapper::BufferedZipWriter> FromRustBufferedZipWriter(
    rs_std::Result<zip_wrapper::BufferedZipWriter, zip_wrapper::VecU8>
        result_buffered_zip_writer);
absl::StatusOr<zip_wrapper::FsZipWriter> FromRustFsZipWriter(
    rs_std::Result<zip_wrapper::FsZipWriter, zip_wrapper::VecU8>
        result_fs_zip_writer);

}  // namespace security::zip

#endif  // SECURITY_ZIP_CONVERTERS_H_
