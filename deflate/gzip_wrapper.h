#ifndef SECURITY_DEFLATE_GZIP_WRAPPER_H_
#define SECURITY_DEFLATE_GZIP_WRAPPER_H_

#include <cstddef>

#include "flate2.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::deflate {

absl::StatusOr<VecU8Wrapper> CompressGzip(absl::string_view contents,
                                          int compression_level);

// WARNING: Decompression Bomb Hazard: This overload uncompresses the entire
// payload into an unbounded heap buffer. For untrusted streams, prefer
// `UncompressGzip(absl::string_view compressed, size_t max_size)` below to
// cap memory allocation and guard against decompression bombs (zip bombs).
absl::StatusOr<VecU8Wrapper> UncompressGzip(absl::string_view compressed);

// Uncompresses at most `max_size` bytes from `compressed`, returning
// `absl::StatusCode::kOutOfRange` if the decompressed payload exceeds
// `max_size`.
absl::StatusOr<VecU8Wrapper> UncompressGzip(absl::string_view compressed,
                                            size_t max_size);

}  // namespace security::deflate

#endif  // SECURITY_DEFLATE_GZIP_WRAPPER_H_
