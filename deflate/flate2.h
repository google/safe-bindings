#ifndef SECURITY_DEFLATE_FLATE2_H_
#define SECURITY_DEFLATE_FLATE2_H_

#include <cstdint>
#include <optional>

#include "crubit/rust.h"
#include "absl/base/attributes.h"
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

using GzDecoder = GzDecoderImpl<rust::read::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<rust::read::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(absl::string_view data, Compression level);
  absl::StatusOr<VecU8Wrapper> read_to_end();

 private:
  explicit GzEncoder(rust::read::GzEncoder encoder);
  rust::read::GzEncoder encoder_;
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

using GzDecoder = GzDecoderImpl<rust::write::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<rust::write::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(Compression level);
  absl::Status write_all(absl::string_view data);
  absl::StatusOr<VecU8Wrapper> finish() &&;

 private:
  explicit GzEncoder(rust::write::GzEncoder encoder);
  rust::write::GzEncoder encoder_;
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
  explicit Compression(rust::Compression compression);
  rust::Compression get() const;

  rust::Compression compression_;
};

class GzHeader final {
 public:
  uint8_t operating_system() const;
  uint32_t mtime() const;

  static std::optional<GzHeader> FromRustOptionGzHeader(
      std::optional<rust::GzHeader> header);

 private:
  explicit GzHeader(rust::GzHeader gz_header);

  rust::GzHeader gz_header_;
};

class VecU8Wrapper {
 public:
  explicit VecU8Wrapper(rust::vec_u8::VecU8 vec_u8);
  absl::string_view as_string_view() const ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::Cord as_cord() &&;

 private:
  rust::vec_u8::VecU8 vec_u8_;
};

}  // namespace security::deflate
#endif  // SECURITY_DEFLATE_FLATE2_H_
