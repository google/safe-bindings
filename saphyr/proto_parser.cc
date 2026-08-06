#include "proto_parser.h"

#include <cstdint>
#include <string>
#include <utility>

#include "support/rs_std/slice_ref.h"
#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::yaml {

using ::security::crubit_helpers::StringViewFromVecU8;

absl::StatusOr<std::string> ConvertYamlToJson(absl::string_view yaml_text,
                                              const ParseOptions& options) {
  auto bytes_slice = rs_std::SliceRef<const uint8_t>(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(yaml_text.data()), yaml_text.size()));
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8> result =
      rust::convert_yaml_to_json(bytes_slice, options.recursion_depth_limit);

  if (!result.has_value()) {
    rust::vec_u8::VecU8 err = std::move(result).err();
    return absl::InvalidArgumentError(StringViewFromVecU8(err));
  }
  rust::vec_u8::VecU8 val = std::move(result).value();
  return std::string(StringViewFromVecU8(val));
}

}  // namespace security::yaml
