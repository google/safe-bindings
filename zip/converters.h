#ifndef SECURITY_ZIP_CONVERTERS_H_
#define SECURITY_ZIP_CONVERTERS_H_

#include <utility>

#include "base/rust/rust_vec_u8.h"
#include "crubit_helpers/string_conversions.h"
#include "rust/zip_wrapper.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

// A wrapper around rust_vec_u8::VecU8 that provides an interface to view the
// underlying data as an absl::string_view.
class RustVecU8Wrapper {
 public:
  RustVecU8Wrapper() = default;
  explicit RustVecU8Wrapper(rust_vec_u8::VecU8 vec_u8)
      : vec_u8_(std::move(vec_u8)) {}

  absl::string_view AsStringView() const {
    return security::crubit_helpers::StringViewFromVecU8(vec_u8_);
  }

 private:
  rust_vec_u8::VecU8 vec_u8_;
};

absl::StatusOr<RustVecU8Wrapper> FromRustResultVecU8(
    zip_wrapper::ResultVecU8 result_vec_u8);
absl::Status FromRustResultUnit(zip_wrapper::ResultUnit result_unit);

absl::StatusOr<zip_wrapper::BufferedZipArchive> FromRustBufferedZipArchive(
    zip_wrapper::ResultBufferedZipArchive result_buffered_zip_archive);
absl::StatusOr<zip_wrapper::FsZipArchive> FromRustFsZipArchive(
    zip_wrapper::ResultFsZipArchive result_fs_zip_archive);

absl::StatusOr<zip_wrapper::BufferedZipFile> FromRustBufferedZipFile(
    zip_wrapper::ResultBufferedZipFile result_buffered_zip_file);
absl::StatusOr<zip_wrapper::FsZipFile> FromRustFsZipFile(
    zip_wrapper::ResultFsZipFile result_fs_zip_file);

absl::StatusOr<zip_wrapper::BufferedZipWriter> FromRustBufferedZipWriter(
    zip_wrapper::ResultBufferedZipWriter result_buffered_zip_writer);
absl::StatusOr<zip_wrapper::FsZipWriter> FromRustFsZipWriter(
    zip_wrapper::ResultFsZipWriter result_fs_zip_writer);

}  // namespace security::zip

#endif  // SECURITY_ZIP_CONVERTERS_H_
