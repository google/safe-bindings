#include "security/deflate/flate2.h"
// #trigger copybara
#include <cstdint>
#include <optional>
#include <utility>

#include "base/rust/rust_vec_u8.h"
#include "security/deflate/rust/flate2_rs.h"
#include "security/ise_memory_safety/crubit_helpers/string_conversions.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/status/statusor.h"
#include "third_party/absl/strings/cord.h"
#include "third_party/absl/strings/string_view.h"

namespace security::deflate {

VecU8Wrapper::VecU8Wrapper(rust_vec_u8::VecU8 vec_u8)
    : vec_u8_(std::move(vec_u8)) {}

absl::string_view VecU8Wrapper::as_string_view() const {
  return crubit_helpers::StringViewFromVecU8(vec_u8_);
}

absl::Cord VecU8Wrapper::as_cord() && {
  absl::string_view view = as_string_view();

  // The `Cord` uses a view to the heap-allocated data managed by
  // `vec_u8_`, to avoid a copy. At the same point, the `VecU8` is
  // moved to the storage associated with the releaser lambda. The
  // heap-allocated data stays at the same place, so the view remains
  // valid. First during the destruction of the `Cord` is the
  // releaser lambda executed and the `VecU8` including the
  // heap-allocated storage is freed.
  return absl::MakeCordFromExternal(view, [b = std::move(vec_u8_)] {});
}

Compression::Compression(flate2_rs::Compression compression)
    : compression_(compression) {}
Compression::Compression(int level)
    : compression_(flate2_rs::Compression::new_(level)) {}

flate2_rs::Compression Compression::get() const { return compression_; }

Compression Compression::best() {
  return Compression(flate2_rs::Compression::best());
}
Compression Compression::none() {
  return Compression(flate2_rs::Compression::none());
}

GzHeader::GzHeader(flate2_rs::GzHeader gz_header)
    : gz_header_(std::move(gz_header)) {}
uint8_t GzHeader::operating_system() const {
  return gz_header_.operating_system();
}
uint32_t GzHeader::mtime() const { return gz_header_.mtime(); }

std::optional<GzHeader> GzHeader::FromRustOptionGzHeader(
    flate2_rs::OptionGzHeader header) {
  if (header.is_none()) {
    return std::nullopt;
  }
  return GzHeader(std::move(header).unwrap());
}

namespace read {

template <typename RustDecoder>
GzDecoderImpl<RustDecoder>::GzDecoderImpl(RustDecoder decoder)
    : decoder_(std::move(decoder)) {}

template <typename RustDecoder>
GzDecoderImpl<RustDecoder> GzDecoderImpl<RustDecoder>::create(
    absl::string_view data) {
  return GzDecoderImpl(RustDecoder::create(data));
}

template <typename RustDecoder>
std::optional<GzHeader> GzDecoderImpl<RustDecoder>::header() const {
  return GzHeader::FromRustOptionGzHeader(decoder_.header());
}

template <typename RustDecoder>
absl::StatusOr<VecU8Wrapper> GzDecoderImpl<RustDecoder>::read_to_end() {
  flate2_rs::ResultVecU8 result_vec_u8 = decoder_.read_to_end();
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

GzEncoder::GzEncoder(flate2_rs::read::GzEncoder encoder)
    : encoder_(std::move(encoder)) {}

GzEncoder GzEncoder::create(absl::string_view data, Compression level) {
  return GzEncoder(flate2_rs::read::GzEncoder::create(data, level.get()));
}

absl::StatusOr<VecU8Wrapper> GzEncoder::read_to_end() {
  flate2_rs::ResultVecU8 result_vec_u8 = encoder_.read_to_end();
  if (result_vec_u8.is_err()) {
    // Potential errors only include system errors when reading from the
    // underlying string_view.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InternalError(std::move(result_vec_u8).unwrap_err());
  }
  return VecU8Wrapper(std::move(result_vec_u8).unwrap());
}

template class GzDecoderImpl<flate2_rs::read::GzDecoder>;
template class GzDecoderImpl<flate2_rs::read::MultiGzDecoder>;

}  // namespace read

namespace write {

template <typename RustDecoder>
GzDecoderImpl<RustDecoder>::GzDecoderImpl(RustDecoder decoder)
    : decoder_(std::move(decoder)) {}

template <typename RustDecoder>
GzDecoderImpl<RustDecoder> GzDecoderImpl<RustDecoder>::create() {
  return GzDecoderImpl(RustDecoder::create());
}

template <typename RustDecoder>
std::optional<GzHeader> GzDecoderImpl<RustDecoder>::header() const {
  return GzHeader::FromRustOptionGzHeader(decoder_.header());
}

template <typename RustDecoder>
absl::Status GzDecoderImpl<RustDecoder>::write_all(absl::string_view data) {
  flate2_rs::ResultUnit result_unit = decoder_.write_all(data);
  if (result_unit.is_err()) {
    // Potential errors include:
    //   - Invalid gzip header when parsing incoming data:
    //         //third_party/rust/flate2/v1/src/gz/write.rs:319
    //   - Corrupt deflate stream:
    //         //third_party/rust/flate2/v1/src/zio.rs:235
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InvalidArgumentError(std::move(result_unit).unwrap_err());
  }
  return absl::OkStatus();
}

template <typename RustDecoder>
absl::StatusOr<VecU8Wrapper> GzDecoderImpl<RustDecoder>::finish() && {
  flate2_rs::ResultVecU8 result_vec_u8 = std::move(decoder_).finish();
  if (result_vec_u8.is_err()) {
    // Potential errors from flate2 crate
    // - Invalid checksum. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/write.rs:290
    return absl::InvalidArgumentError(std::move(result_vec_u8).unwrap_err());
  }
  return VecU8Wrapper(std::move(result_vec_u8).unwrap());
}

GzEncoder::GzEncoder(flate2_rs::write::GzEncoder encoder)
    : encoder_(std::move(encoder)) {}

GzEncoder GzEncoder::create(Compression level) {
  return GzEncoder(flate2_rs::write::GzEncoder::create(level.get()));
}

absl::Status GzEncoder::write_all(absl::string_view data) {
  flate2_rs::ResultUnit result_unit = encoder_.write_all(data);
  if (result_unit.is_err()) {
    // Potential errors include:
    //   - Corrupt deflate stream:
    //         //third_party/rust/flate2/v1/src/zio.rs:235
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InvalidArgumentError(std::move(result_unit).unwrap_err());
  }
  return absl::OkStatus();
}

absl::StatusOr<VecU8Wrapper> GzEncoder::finish() && {
  flate2_rs::ResultVecU8 result_vec_u8 = std::move(encoder_).finish();
  if (result_vec_u8.is_err()) {
    // Potential errors only include system errors when using the
    // internal Rust writer.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InternalError(std::move(result_vec_u8).unwrap_err());
  }
  return VecU8Wrapper(std::move(result_vec_u8).unwrap());
}

template class GzDecoderImpl<flate2_rs::write::GzDecoder>;
template class GzDecoderImpl<flate2_rs::write::MultiGzDecoder>;

}  // namespace write

}  // namespace security::deflate
