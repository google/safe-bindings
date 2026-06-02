#include "gzip_wrapper.h"

#include <cstdint>
#include <utility>

#include "flate2.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::deflate {

namespace {

absl::string_view StringViewFromVecU8(const rust::vec_u8::VecU8& vec) {
  static_assert(sizeof(const char) == sizeof(const uint8_t),
                "Alignment check failed");
  static_assert(alignof(const char) <= alignof(const uint8_t),
                "We need to keep the pointer to `vec`'s data aligned after the "
                "conversion to char.");
  return absl::string_view(reinterpret_cast<const char*>(vec.as_ptr()),
                           vec.len());
}

}  // namespace

absl::StatusOr<VecU8Wrapper> CompressGzip(absl::string_view contents,
                                          int compression_level) {
  if (compression_level < 0 || compression_level > 9) {
    return absl::InvalidArgumentError("invalid compression level, must be 0-9");
  }

  auto encoder = rust::read::GzEncoder::create(
      absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(contents.data()), contents.size()),
      rust::Compression::new_(compression_level));

  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8> result =
      encoder.read_to_end();
  if (!result.has_value()) {
    // Potential errors only include system errors when reading from the
    // underlying string_view.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InternalError(StringViewFromVecU8(std::move(result).err()));
  }
  return VecU8Wrapper(std::move(result).value());
}

absl::StatusOr<VecU8Wrapper> UncompressGzip(absl::string_view compressed) {
  auto decoder = rust::read::GzDecoder::create(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(compressed.data()), compressed.size()));
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8> result =
      decoder.read_to_end();
  if (!result.has_value()) {
    // Potential errors from flate2 crate
    // - Invalid gzip header or invalid checksum. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/mod.rs:126
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:310
    // - Invalid EOF due to truncated gzip stream. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:304
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result).err()));
  }
  return VecU8Wrapper(std::move(result).value());
}

}  // namespace security::deflate
