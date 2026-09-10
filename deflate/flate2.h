#ifndef SECURITY_DEFLATE_FLATE2_H_
#define SECURITY_DEFLATE_FLATE2_H_

#include <cstdint>
#include <optional>

#include "crubit/deflate_cpp_bindings.h"
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

using GzDecoder = GzDecoderImpl<deflate_cpp_bindings::read::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<deflate_cpp_bindings::read::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(absl::string_view data, Compression level);
  absl::StatusOr<VecU8Wrapper> read_to_end();

 private:
  explicit GzEncoder(deflate_cpp_bindings::read::GzEncoder encoder);
  deflate_cpp_bindings::read::GzEncoder encoder_;
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

using GzDecoder = GzDecoderImpl<deflate_cpp_bindings::write::GzDecoder>;
using MultiGzDecoder = GzDecoderImpl<deflate_cpp_bindings::write::MultiGzDecoder>;

class GzEncoder final {
 public:
  static GzEncoder create(Compression level);
  absl::Status write_all(absl::string_view data);
  absl::StatusOr<VecU8Wrapper> finish() &&;

 private:
  explicit GzEncoder(deflate_cpp_bindings::write::GzEncoder encoder);
  deflate_cpp_bindings::write::GzEncoder encoder_;
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
  explicit Compression(deflate_cpp_bindings::Compression compression);
  deflate_cpp_bindings::Compression get() const;

  deflate_cpp_bindings::Compression compression_;
};

class GzHeader final {
 public:
  uint8_t operating_system() const;
  uint32_t mtime() const;

  static std::optional<GzHeader> FromRustOptionGzHeader(
      std::optional<deflate_cpp_bindings::GzHeader> header);

 private:
  explicit GzHeader(deflate_cpp_bindings::GzHeader gz_header);

  deflate_cpp_bindings::GzHeader gz_header_;
};

class VecU8Wrapper {
 public:
  explicit VecU8Wrapper(deflate_cpp_bindings::vec_u8::VecU8 vec_u8);
  absl::string_view as_string_view() const ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::Cord as_cord() &&;

 private:
  deflate_cpp_bindings::vec_u8::VecU8 vec_u8_;
};

}  // namespace security::deflate
#endif  // SECURITY_DEFLATE_FLATE2_H_
