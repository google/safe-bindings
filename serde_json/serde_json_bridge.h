#ifndef SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_
#define SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_

#include <cstdint>
#include <string>
#include <vector>

#include "google/protobuf/struct.pb.h"
#include "rust/serde_json_bridge_rs.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::json::serde_json_bridge {

class SerdeJson final {
 public:
  // Parses a raw JSON string into a SerdeJson object.
  // Returns an absl::Status if the input is malformed.
  static absl::StatusOr<SerdeJson> Parse(absl::string_view data);

  // Creates a new SerdeJson of a given type.
  static absl::StatusOr<SerdeJson> CreateObject();
  static absl::StatusOr<SerdeJson> CreateInt(int64_t value);
  static absl::StatusOr<SerdeJson> CreateBool(bool value);
  static absl::StatusOr<SerdeJson> CreateDouble(double value);
  static absl::StatusOr<SerdeJson> CreateNull();
  static absl::StatusOr<SerdeJson> CreateString(absl::string_view value);

  // Returns the value of the current json node.
  absl::StatusOr<int64_t> GetInt() const;
  absl::StatusOr<bool> GetBool() const;
  absl::StatusOr<std::string> GetString() const;
  absl::StatusOr<double> GetDouble() const;
  absl::StatusOr<std::vector<SerdeJson>> GetArray() const;

  // Returns a node of the corresponding `key` field of this json object.
  // Used when we don't know the type or for objects.
  absl::StatusOr<SerdeJson> GetField(absl::string_view key) const;

  // Returns the value of the corresponding field of this json object.
  absl::StatusOr<std::string> GetFieldString(absl::string_view key) const;
  absl::StatusOr<bool> GetFieldBool(absl::string_view key) const;
  absl::StatusOr<int64_t> GetFieldInt(absl::string_view key) const;
  absl::StatusOr<double> GetFieldDouble(absl::string_view key) const;
  absl::StatusOr<SerdeJson> GetFieldObject(absl::string_view key) const;
  absl::StatusOr<std::vector<SerdeJson>> GetFieldArray(
      absl::string_view key) const;

  // Methods for checking the type of this json node.
  bool IsNull() const;
  bool IsObject() const;
  bool IsArray() const;
  bool IsString() const;
  bool IsNumber() const;
  bool IsDouble() const;
  bool IsBool() const;
  bool IsInt() const;

  // If the current node is an object, returns whether the field exists.
  absl::StatusOr<bool> HasField(absl::string_view key) const;

  // Returns the keys of a json object, or non-ok status is this instance
  // is not an object.
  absl::StatusOr<std::vector<std::string>> GetKeys() const;

  // Convert this object to string.
  std::string ToString() const;
  absl::StatusOr<::google::protobuf::Struct> ToProtoStruct() const;
  absl::StatusOr<::google::protobuf::Value> ToProtoValue() const;

  // Methods for adding fields to the JSON object.
  absl::Status AddFieldBool(absl::string_view key, bool value);
  absl::Status AddFieldDouble(absl::string_view key, double value);
  absl::Status AddFieldInt(absl::string_view key, int64_t value);
  absl::Status AddFieldNull(absl::string_view key);
  absl::Status AddFieldObject(absl::string_view key, const SerdeJson& value);
  absl::Status AddFieldString(absl::string_view key, absl::string_view value);
  absl::Status AddFieldArray(absl::string_view key,
                             absl::Span<const SerdeJson> value);
  absl::Status AddFieldArray(absl::string_view key,
                             std::vector<SerdeJson> value);

 private:
  explicit SerdeJson(serde_json_bridge_rs::json::SerdeJson);

  static std::vector<SerdeJson> ConvertVecSerdeJsonToVector(
      const serde_json_bridge_rs::json::VecSerdeJson& rs_vec_serde_json);

  serde_json_bridge_rs::json::SerdeJson json_obj_;
};

}  // namespace security::json::serde_json_bridge

#endif  // SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_
