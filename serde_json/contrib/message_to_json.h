#ifndef SECURITY_JSON_SERDE_JSON_CONTRIB_MESSAGE_TO_JSON_H_
#define SECURITY_JSON_SERDE_JSON_CONTRIB_MESSAGE_TO_JSON_H_

#include <concepts>

#include "serde_json_bridge.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "third_party/protobuf/json/json.h"
#include "third_party/protobuf/message.h"

namespace security::json::serde_json_bridge::contrib {

// Converts a proto message to a SerdeJson object.
absl::StatusOr<SerdeJson> MessageToJson(
    const proto2::Message& message,
    const proto2::json::PrintOptions& options = proto2::json::PrintOptions());

// Converts a SerdeJson object to a proto message.
absl::Status JsonToMessage(
    const SerdeJson& json, proto2::Message* message,
    const proto2::json::ParseOptions& options = proto2::json::ParseOptions());

// Converts a SerdeJson object to a proto message. This will construct a proto
// message to populate with the JSON data.
template <typename T>
  requires std::derived_from<T, proto2::Message>
absl::StatusOr<T> JsonToMessage(
    const SerdeJson& json,
    const proto2::json::ParseOptions& options = proto2::json::ParseOptions()) {
  T message;
  absl::Status status = JsonToMessage(json, &message, options);
  if (!status.ok()) return status;
  return message;
}

}  // namespace security::json::serde_json_bridge::contrib

#endif  // SECURITY_JSON_SERDE_JSON_CONTRIB_MESSAGE_TO_JSON_H_
