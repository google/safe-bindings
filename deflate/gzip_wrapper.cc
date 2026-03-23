#include "security/deflate/gzip_wrapper.h"

#include <utility>

#include "security/deflate/flate2.h"
#include "security/deflate/rust/flate2_rs.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/status/statusor.h"
#include "third_party/absl/strings/string_view.h"

namespace security::deflate {

absl::StatusOr<VecU8Wrapper> CompressGzip(absl::string_view contents,
                                          int compression_level) {
  if (compression_level < 0 || compression_level > 9) {
    return absl::InvalidArgumentError("invalid compression level, must be 0-9");
  }

  auto encoder = flate2_rs::read::GzEncoder::create(
      contents, flate2_rs::Compression::new_(compression_level));

  flate2_rs::ResultVecU8 result_vec_u8 = encoder.read_to_end();
  if (result_vec_u8.is_err()) {
    // Potential errors only include system errors when reading from the
    // underlying string_view.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InternalError(std::move(result_vec_u8).unwrap_err());
  }
  return VecU8Wrapper(std::move(result_vec_u8).unwrap());
}

absl::StatusOr<VecU8Wrapper> UncompressGzip(absl::string_view compressed) {
  auto decoder = flate2_rs::read::GzDecoder::create(compressed);
  flate2_rs::ResultVecU8 result_vec_u8 = decoder.read_to_end();
  if (result_vec_u8.is_err()) {
    // Potential errors from flate2 crate
    // - Invalid gzip header or invalid checksum. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/mod.rs:126
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:310
    // - Invalid EOF due to truncated gzip stream. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:304
    return absl::InvalidArgumentError(std::move(result_vec_u8).unwrap_err());
  }
  return VecU8Wrapper(std::move(result_vec_u8).unwrap());
}

}  // namespace security::deflate
