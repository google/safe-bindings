#ifndef SAFE_BINDINGS_KAMADAK_EXIF_EXIF_READER_H_
#define SAFE_BINDINGS_KAMADAK_EXIF_EXIF_READER_H_

#include <cstdint>
#include <optional>

#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "exif_bridge.h"

namespace security::exif_bridge {

/**
 * A utility wrapper around `Reader` and `Exif` to mimic the usage pattern
 * of `image_exif`.
 */
class ExifReader {
 public:
  explicit ExifReader(absl::Span<const uint8_t> data);

  /**
   * Gets an unsigned integer tag.
   */
  absl::StatusOr<uint32_t> GetUIntTag(Tag tag, In in = In::kPrimary) const;

 private:
  absl::StatusOr<Exif> exif_;
};

}  // namespace security::exif_bridge

#endif  // SAFE_BINDINGS_KAMADAK_EXIF_EXIF_READER_H_
