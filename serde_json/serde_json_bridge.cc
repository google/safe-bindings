#include "security/json/serde_json/serde_json_bridge.h"

#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

#include "security/json/serde_json/rust/serde_json_bridge_rs.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/status/statusor.h"
#include "third_party/absl/strings/string_view.h"

namespace security::json::serde_json_bridge {

SerdeJson::SerdeJson(serde_json_bridge_rs::json::SerdeJson sj)
    : json_obj_(std::move(sj)) {}

// This function has to be a member of `SerdeJson` class as it uses
// private constructor `SerdeJson(serde_json_bridge_rs::json::SerdeJson)`.
std::vector<SerdeJson> SerdeJson::ConvertVecSerdeJsonToVector(
    const serde_json_bridge_rs::json::VecSerdeJson& rs_vec_serde_json) {
  std::vector<SerdeJson> ret_vec;
  ret_vec.reserve(rs_vec_serde_json.len());
  for (size_t i = 0; i < rs_vec_serde_json.len(); ++i) {
    ret_vec.push_back(SerdeJson(rs_vec_serde_json.as_ptr()[i]));
  }
  return ret_vec;
}

absl::StatusOr<SerdeJson> SerdeJson::CreateObject() {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_object());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateInt(int64_t value) {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_int(value));
}

absl::StatusOr<SerdeJson> SerdeJson::CreateDouble(double value) {
  serde_json_bridge_rs::json::ResultSerdeJson rs_result =
      serde_json_bridge_rs::json::SerdeJson::create_double(value);

  if (rs_result.is_err()) {
    return absl::InvalidArgumentError(std::move(rs_result).unwrap_err());
  }
  return SerdeJson(std::move(rs_result).unwrap());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateBool(bool value) {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_bool(value));
}

absl::StatusOr<SerdeJson> SerdeJson::CreateNull() {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_null());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateString(absl::string_view value) {
  serde_json_bridge_rs::json::ResultSerdeJson rs_result =
      serde_json_bridge_rs::json::SerdeJson::create_string(value);

  if (rs_result.is_err()) {
    return absl::InvalidArgumentError(std::move(rs_result).unwrap_err());
  }
  return SerdeJson(std::move(rs_result).unwrap());
}

absl::StatusOr<SerdeJson> SerdeJson::Parse(absl::string_view data) {
  serde_json_bridge_rs::json::ResultSerdeJson rs_result =
      serde_json_bridge_rs::json::SerdeJson::parse_string(data);

  if (rs_result.is_err()) {
    return absl::InvalidArgumentError(std::move(rs_result).unwrap_err());
  }
  return SerdeJson(std::move(rs_result).unwrap());
}

absl::StatusOr<SerdeJson> SerdeJson::GetField(absl::string_view key) const {
  serde_json_bridge_rs::json::ResultSerdeJson rs_result =
      json_obj_.get_field(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return SerdeJson(std::move(rs_result).unwrap());
}

absl::StatusOr<SerdeJson> SerdeJson::GetFieldObject(
    absl::string_view key) const {
  serde_json_bridge_rs::json::ResultSerdeJson rs_result =
      json_obj_.get_field_object(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return SerdeJson(std::move(rs_result).unwrap());
}

absl::StatusOr<bool> SerdeJson::GetBool() const {
  serde_json_bridge_rs::json::ResultBool rs_result = json_obj_.get_bool();

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<std::string> SerdeJson::GetString() const {
  serde_json_bridge_rs::json::ResultString rs_result = json_obj_.get_string();

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<int64_t> SerdeJson::GetInt() const {
  serde_json_bridge_rs::json::Resulti64 rs_result = json_obj_.get_int();

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<double> SerdeJson::GetDouble() const {
  serde_json_bridge_rs::json::ResultDouble rs_result = json_obj_.get_double();

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<std::vector<SerdeJson>> SerdeJson::GetArray() const {
  serde_json_bridge_rs::json::ResultVecSerdeJson rs_result =
      json_obj_.get_array();

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return ConvertVecSerdeJsonToVector(std::move(rs_result).unwrap());
}

absl::StatusOr<std::string> SerdeJson::GetFieldString(
    absl::string_view key) const {
  serde_json_bridge_rs::json::ResultString rs_result =
      json_obj_.get_field_string(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<bool> SerdeJson::GetFieldBool(absl::string_view key) const {
  serde_json_bridge_rs::json::ResultBool rs_result =
      json_obj_.get_field_bool(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<int64_t> SerdeJson::GetFieldInt(absl::string_view key) const {
  serde_json_bridge_rs::json::Resulti64 rs_result =
      json_obj_.get_field_int(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<double> SerdeJson::GetFieldDouble(absl::string_view key) const {
  serde_json_bridge_rs::json::ResultDouble rs_result =
      json_obj_.get_field_double(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

absl::StatusOr<std::vector<SerdeJson>> SerdeJson::GetFieldArray(
    absl::string_view key) const {
  serde_json_bridge_rs::json::ResultVecSerdeJson rs_result =
      json_obj_.get_field_array(key);

  if (rs_result.is_err()) {
    return absl::FailedPreconditionError(std::move(rs_result).unwrap_err());
  }

  return ConvertVecSerdeJsonToVector(std::move(rs_result).unwrap());
}

bool SerdeJson::IsNull() const { return json_obj_.is_null(); }

bool SerdeJson::IsObject() const { return json_obj_.is_object(); }

bool SerdeJson::IsArray() const { return json_obj_.is_array(); }

bool SerdeJson::IsString() const { return json_obj_.is_string(); }

bool SerdeJson::IsNumber() const { return json_obj_.is_number(); }

bool SerdeJson::IsInt() const { return json_obj_.is_i64(); }

bool SerdeJson::IsDouble() const { return json_obj_.is_f64(); }

bool SerdeJson::IsBool() const { return json_obj_.is_boolean(); }

std::string SerdeJson::ToString() const { return json_obj_.to_string(); }

absl::StatusOr<bool> SerdeJson::HasField(absl::string_view key) const {
  serde_json_bridge_rs::json::ResultBool rs_result = json_obj_.has_field(key);

  if (rs_result.is_err()) {
    return absl::InvalidArgumentError(std::move(rs_result).unwrap_err());
  }

  return std::move(rs_result).unwrap();
}

}  // namespace security::json::serde_json_bridge
