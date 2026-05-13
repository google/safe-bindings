#ifndef SECURITY_DEFLATE_FLATE2_H_
#define SECURITY_DEFLATE_FLATE2_H_

#include <cstdint>
#include <optional>

#include "base/rust/rust_vec_u8.h"
#include "rust/flate2_rs.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/cord.h"
#include "absl/strings/string_view.h"

namespace security::deflate {

class GzHeader;
class Compression;
class VecU8Wrapper;

namespace read {

template <typename RustDecoder>
class GzDecoderImpl final {
 public:
  static GzDecoderImpl create(absl::string_view data);
  std::optional<GzHeader> header() const;
  absl::StatusOr<VecU8Wrapper> read_to_end();

 private:
  explicit GzDecoderImpl(RustDecoder decoder);
  RustDecoder decoder_;
};

using GzDecoder = GzDecoderImpl<flate2_rs::read::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<flate2_rs::read::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(absl::string_view data, Compression level);
  absl::StatusOr<VecU8Wrapper> read_to_end();

 private:
  explicit GzEncoder(flate2_rs::read::GzEncoder encoder);
  flate2_rs::read::GzEncoder encoder_;
};

}  // namespace read

namespace write {

template <typename RustDecoder>
class GzDecoderImpl final {
 public:
  static GzDecoderImpl create();
  std::optional<GzHeader> header() const;
  absl::Status write_all(absl::string_view data);
  absl::StatusOr<VecU8Wrapper> finish() &&;
  GzDecoderImpl(GzDecoderImpl&&) = default;

 private:
  explicit GzDecoderImpl(RustDecoder decoder);
  RustDecoder decoder_;
};

using GzDecoder = GzDecoderImpl<flate2_rs::write::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<flate2_rs::write::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(Compression level);
  absl::Status write_all(absl::string_view data);
  absl::StatusOr<VecU8Wrapper> finish() &&;

 private:
  explicit GzEncoder(flate2_rs::write::GzEncoder encoder);
  flate2_rs::write::GzEncoder encoder_;
};

}  // namespace write

class Compression final {
 public:
  static Compression best();
  static Compression none();
  explicit Compression(int level);

 private:
  friend class read::GzEncoder;
  friend class write::GzEncoder;
  explicit Compression(flate2_rs::Compression compression);
  flate2_rs::Compression get() const;

  flate2_rs::Compression compression_;
};

class GzHeader final {
 public:
  uint8_t operating_system() const;
  uint32_t mtime() const;

  static std::optional<GzHeader> FromRustOptionGzHeader(
      flate2_rs::OptionGzHeader header);

 private:
  explicit GzHeader(flate2_rs::GzHeader gz_header);

  flate2_rs::GzHeader gz_header_;
};

class VecU8Wrapper {
 public:
  explicit VecU8Wrapper(rust_vec_u8::VecU8 vec_u8);
  absl::string_view as_string_view() const;
  absl::Cord as_cord() &&;

 private:
  rust_vec_u8::VecU8 vec_u8_;
};

}  // namespace security::deflate
#endif  // SECURITY_DEFLATE_FLATE2_H_
