#ifndef SECURITY_DEFLATE_GZIP_WRAPPER_H_
#define SECURITY_DEFLATE_GZIP_WRAPPER_H_

#include "flate2.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::deflate {

absl::StatusOr<VecU8Wrapper> CompressGzip(absl::string_view contents,
                                          int compression_level);

absl::StatusOr<VecU8Wrapper> UncompressGzip(absl::string_view compressed);

}  // namespace security::deflate

#endif  // SECURITY_DEFLATE_GZIP_WRAPPER_H_
