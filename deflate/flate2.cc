#include "flate2.h"

#include <cstdint>
#include <optional>
#include <utility>

#include "rust.h"
#include "crubit_helpers/string_conversions.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/cord.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::deflate {

using ::security::crubit_helpers::StringViewFromVecU8;

VecU8Wrapper::VecU8Wrapper(rust::vec_u8::VecU8 vec_u8)
    : vec_u8_(std::move(vec_u8)) {}

absl::string_view VecU8Wrapper::as_string_view() const {
  return StringViewFromVecU8(vec_u8_);
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

Compression::Compression(rust::Compression compression)
    : compression_(compression) {}
Compression::Compression(int level)
    : compression_(rust::Compression::new_(level)) {}

rust::Compression Compression::get() const { return compression_; }

Compression Compression::best() {
  return Compression(rust::Compression::best());
}
Compression Compression::none() {
  return Compression(rust::Compression::none());
}

GzHeader::GzHeader(rust::GzHeader gz_header) : gz_header_(gz_header) {}

uint8_t GzHeader::operating_system() const {
  return gz_header_.operating_system();
}
uint32_t GzHeader::mtime() const { return gz_header_.mtime(); }

std::optional<GzHeader> GzHeader::FromRustOptionGzHeader(
    std::optional<rust::GzHeader> header) {
  if (!header.has_value()) {
    return std::nullopt;
  }
  return GzHeader(std::move(header).value());
}

namespace read {

template <typename RustDecoder>
GzDecoderImpl<RustDecoder>::GzDecoderImpl(RustDecoder decoder)
    : decoder_(std::move(decoder)) {}

template <typename RustDecoder>
GzDecoderImpl<RustDecoder> GzDecoderImpl<RustDecoder>::create(
    absl::string_view data) {
  return GzDecoderImpl(RustDecoder::create(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(data.data()), data.size())));
}

template <typename RustDecoder>
std::optional<GzHeader> GzDecoderImpl<RustDecoder>::header() const {
  return GzHeader::FromRustOptionGzHeader(decoder_.header());
}

template <typename RustDecoder>
absl::StatusOr<VecU8Wrapper> GzDecoderImpl<RustDecoder>::read_to_end() {
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8>
      result_vec_u8 = decoder_.read_to_end();
  if (!result_vec_u8.has_value()) {
    // Potential errors from flate2 crate
    // - Invalid gzip header or invalid checksum. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/mod.rs:126
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:310
    // - Invalid EOF due to truncated gzip stream. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/bufread.rs:304
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result_vec_u8).err()));
  }
  return VecU8Wrapper(std::move(result_vec_u8).value());
}

GzEncoder::GzEncoder(rust::read::GzEncoder encoder)
    : encoder_(std::move(encoder)) {}

GzEncoder GzEncoder::create(absl::string_view data, Compression level) {
  return GzEncoder(rust::read::GzEncoder::create(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(data.data()),
                                data.size()),
      level.get()));
}

absl::StatusOr<VecU8Wrapper> GzEncoder::read_to_end() {
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8>
      result_vec_u8 = encoder_.read_to_end();
  if (!result_vec_u8.has_value()) {
    // Potential errors only include system errors when reading from the
    // underlying string_view.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result_vec_u8).err()));
  }
  return VecU8Wrapper(std::move(result_vec_u8).value());
}

template class GzDecoderImpl<rust::read::GzDecoder>;
template class GzDecoderImpl<rust::read::MultiGzDecoder>;

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
  rs_std::Result<uint8_t, rust::vec_u8::VecU8> result_unit =
      decoder_.write_all(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  if (!result_unit.has_value()) {
    // Potential errors include:
    //   - Invalid gzip header when parsing incoming data:
    //         //third_party/rust/flate2/v1/src/gz/write.rs:319
    //   - Corrupt deflate stream:
    //         //third_party/rust/flate2/v1/src/zio.rs:235
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result_unit).err()));
  }
  return absl::OkStatus();
}

template <typename RustDecoder>
absl::StatusOr<VecU8Wrapper> GzDecoderImpl<RustDecoder>::finish() && {
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8>
      result_vec_u8 = std::move(decoder_).finish();
  if (!result_vec_u8.has_value()) {
    // Potential errors from flate2 crate
    // - Invalid checksum. Error locations:
    //    - //third_party/rust/flate2/v1/src/gz/write.rs:290
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result_vec_u8).err()));
  }
  return VecU8Wrapper(std::move(result_vec_u8).value());
}

GzEncoder::GzEncoder(rust::write::GzEncoder encoder)
    : encoder_(std::move(encoder)) {}

GzEncoder GzEncoder::create(Compression level) {
  return GzEncoder(rust::write::GzEncoder::create(level.get()));
}

absl::Status GzEncoder::write_all(absl::string_view data) {
  rs_std::Result<uint8_t, rust::vec_u8::VecU8> result_unit =
      encoder_.write_all(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(data.data()), data.size()));
  if (!result_unit.has_value()) {
    // Potential errors include:
    //   - Corrupt deflate stream:
    //         //third_party/rust/flate2/v1/src/zio.rs:235
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result_unit).err()));
  }
  return absl::OkStatus();
}

absl::StatusOr<VecU8Wrapper> GzEncoder::finish() && {
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8>
      result_vec_u8 = std::move(encoder_).finish();
  if (!result_vec_u8.has_value()) {
    // Potential errors only include system errors when using the
    // internal Rust writer.
    // NOTE: b/351976355 - Send the error code from Rust.
    return absl::InternalError(
        StringViewFromVecU8(std::move(result_vec_u8).err()));
  }
  return VecU8Wrapper(std::move(result_vec_u8).value());
}

template class GzDecoderImpl<rust::write::GzDecoder>;
template class GzDecoderImpl<rust::write::MultiGzDecoder>;

}  // namespace write

}  // namespace security::deflate
