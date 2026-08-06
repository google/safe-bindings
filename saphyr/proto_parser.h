#ifndef SECURITY_YAML_PROTO_PARSER_H_
#define SECURITY_YAML_PROTO_PARSER_H_

#include <string>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "third_party/gloop/util/status/status_macros.h"
#include "third_party/protobuf/util/json_util.h"

namespace security::yaml {

struct ParseOptions {
  int recursion_depth_limit = 30;
  bool ignore_unknown_fields = true;
};

// Converts a YAML string to JSON.
absl::StatusOr<std::string> ConvertYamlToJson(
    absl::string_view yaml_text, const ParseOptions& options = ParseOptions());

// Parses a YAML string into any protobuf message type.
template <typename ProtoT>
absl::StatusOr<ProtoT> ParseYaml(absl::string_view yaml_text,
                                 const ParseOptions& options = ParseOptions()) {
  ASSIGN_OR_RETURN(std::string json, ConvertYamlToJson(yaml_text, options));
  ProtoT proto;
  proto2::util::JsonParseOptions json_options;
  json_options.ignore_unknown_fields = options.ignore_unknown_fields;
  RETURN_IF_ERROR(
      proto2::util::JsonStringToMessage(json, &proto, json_options));
  return proto;
}

}  // namespace security::yaml

#endif  // SECURITY_YAML_PROTO_PARSER_H_
