#ifndef SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_
#define SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_

#include <cstdint>
#include <string>
#include <vector>

#include "security/json/serde_json/rust/serde_json_bridge_rs.h"
#include "third_party/absl/status/statusor.h"
#include "third_party/absl/strings/string_view.h"

namespace security::json::serde_json_bridge {

class SerdeJson final {
 public:
  // Parses a raw JSON string into a SerdeJson object.
  // Returns an absl::Status if the input is malformed.
  static absl::StatusOr<SerdeJson> Parse(absl::string_view data);

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

  // Convert this object to string.
  std::string ToString() const;

 private:
  explicit SerdeJson(serde_json_bridge_rs::json::SerdeJson);

  static std::vector<SerdeJson> ConvertVecSerdeJsonToVector(
      const serde_json_bridge_rs::json::VecSerdeJson& rs_vec_serde_json);

  serde_json_bridge_rs::json::SerdeJson json_obj_;
};

}  // namespace security::json::serde_json_bridge

#endif  // SECURITY_JSON_SERDE_JSON_SERDE_JSON_BRIDGE_H_
