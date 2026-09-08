#include "contrib/message_to_json.h"

#include <string>

#include "serde_json_bridge.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "third_party/protobuf/json/json.h"
#include "third_party/protobuf/message.h"

namespace security::json::serde_json_bridge::contrib {

absl::StatusOr<SerdeJson> MessageToJson(
    const proto2::Message& message, const proto2::json::PrintOptions& options) {
  std::string json_string;
  absl::Status status =
      proto2::json::MessageToJsonString(message, &json_string, options);
  if (!status.ok()) {
    return status;
  }
  return SerdeJson::Parse(json_string);
}

absl::Status JsonToMessage(const SerdeJson& json, proto2::Message* message,
                           const proto2::json::ParseOptions& options) {
  std::string json_string = json.ToString(/*sort_keys=*/false);
  return proto2::json::JsonStringToMessage(json_string, message, options);
}

}  // namespace security::json::serde_json_bridge::contrib
