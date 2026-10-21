#ifndef SECURITY_DEFLATE_FLATE2_H_
#define SECURITY_DEFLATE_FLATE2_H_

#include <cstddef>
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

  // WARNING: Decompression Bomb Hazard: `read_to_end()` decompresses until EOF
  // into an unbounded heap buffer. For untrusted streams, prefer
  // `read_to_end(size_t max_size)` below to cap memory allocation and guard
  // against decompression bombs (zip bombs).
  absl::StatusOr<VecU8Wrapper> read_to_end();

  // Decompresses at most `max_size` bytes from the stream, returning
  // `absl::StatusCode::kOutOfRange` if the decompressed output exceeds
  // `max_size`.
  absl::StatusOr<VecU8Wrapper> read_to_end(size_t max_size);

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
  // Returns a non-owning view of the decompressed/compressed bytes.
  //
  // WARNING: The returned string_view is NOT null-terminated and may contain
  // embedded null characters (`\0`). Do NOT pass `.data()` to C-style APIs
  // expecting null-terminated strings (`open`, `strcmp`, `printf("%s")`).
  // Lifetime: bound to this VecU8Wrapper instance.
  //
  // Two compile-time guards apply. The `const&&` overload is deleted, so
  // calling on a temporary is rejected outright; and
  // ABSL_ATTRIBUTE_LIFETIME_BOUND lets Clang reject (-Werror,-Wdangling)
  // dangling *initializations*, including ones reached through
  // `absl::StatusOr::operator->`, which absl annotates:
  //     absl::string_view v = UncompressGzip(d)->as_string_view();  // error
  //
  // Three cases are NOT diagnosed and remain the caller's responsibility:
  //     absl::string_view v;
  //     v = UncompressGzip(d)->as_string_view();   // assignment: dangles
  //     { auto w = *UncompressGzip(d); v = w.as_string_view(); }  // scope exit
  //     absl::string_view u =
  //         Trim(UncompressGzip(d)->as_string_view());  // laundered: dangles
  //
  // The third is the general shape: passing the view through any intervening
  // function that is not itself lifetime-annotated breaks the chain.
  //
  // Consuming inside the full-expression is safe and is not diagnosed:
  //     Handle(UncompressGzip(d)->as_string_view());               // OK
  //     std::string owned(UncompressGzip(d)->as_string_view());    // OK
  //
  // To keep the bytes past the statement, name the owner first, or use
  // `std::move(wrapper).as_cord()`. Note `as_cord()` is `&&`-qualified: it
  // consumes the wrapper, so it is an alternative to holding a view, not a
  // way to get one alongside it.
  absl::string_view as_string_view() const& ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view as_string_view() const&& = delete;
  absl::Cord as_cord() &&;

 private:
  rust::vec_u8::VecU8 vec_u8_;
};

}  // namespace security::deflate
#endif  // SECURITY_DEFLATE_FLATE2_H_
