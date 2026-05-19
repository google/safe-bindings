#include "exif_reader.h"

#include <cstdint>
#include <optional>

#include "exif_bridge.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"

namespace security::exif_bridge {

ExifReader::ExifReader(absl::Span<const uint8_t> data) {
  Reader reader;
  exif_ = reader.read_from_container(data);
}

absl::StatusOr<uint32_t> ExifReader::GetUIntTag(Tag tag, In in) const {
  if (!exif_.ok()) return exif_.status();
  std::optional<Field> field = exif_->get_field(tag, in);
  if (!field.has_value()) return absl::NotFoundError("Tag not found.");
  std::optional<uint32_t> val = field->value().get_uint(0);
  if (!val.has_value()) {
    return absl::NotFoundError("Value not found or not a uint.");
  }
  return *val;
}

}  // namespace security::exif_bridge
