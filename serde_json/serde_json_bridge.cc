#include "serde_json_bridge.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <iterator>
#include <string>
#include <utility>
#include <vector>

#include "google/protobuf/struct.proto.h"
#include "rust/serde_json_bridge_rs.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "third_party/gloop/util/status/status_macros.h"

namespace security::json::serde_json_bridge {

static std::string FromRustRawString(
    const serde_json_bridge_rs::raw_string::RawString& raw_string) {
  return std::string(reinterpret_cast<const char*>(raw_string.as_ptr()),
                     raw_string.len());
}

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
  rs_std::Result<serde_json_bridge_rs::json::SerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = serde_json_bridge_rs::json::SerdeJson::create_double(value);

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateBool(bool value) {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_bool(value));
}

absl::StatusOr<SerdeJson> SerdeJson::CreateNull() {
  return SerdeJson(serde_json_bridge_rs::json::SerdeJson::create_null());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateString(absl::string_view value) {
  rs_std::Result<serde_json_bridge_rs::json::SerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = serde_json_bridge_rs::json::SerdeJson::create_string(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(value.data()), value.size()));

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::Parse(absl::string_view data) {
  rs_std::Result<serde_json_bridge_rs::json::SerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = serde_json_bridge_rs::json::SerdeJson::parse_string(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(data.data()), data.size()));

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::GetField(absl::string_view key) const {
  rs_std::Result<serde_json_bridge_rs::json::SerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::GetFieldObject(
    absl::string_view key) const {
  rs_std::Result<serde_json_bridge_rs::json::SerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field_object(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<bool> SerdeJson::GetBool() const {
  rs_std::Result<bool, serde_json_bridge_rs::raw_string::RawString> rs_result =
      json_obj_.get_bool();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::string> SerdeJson::GetString() const {
  rs_std::Result<serde_json_bridge_rs::raw_string::RawString,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_string();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return FromRustRawString(std::move(rs_result).value());
}

absl::StatusOr<int64_t> SerdeJson::GetInt() const {
  rs_std::Result<int64_t, serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_int();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> SerdeJson::GetDouble() const {
  rs_std::Result<double, serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_double();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::vector<SerdeJson>> SerdeJson::GetArray() const {
  rs_std::Result<serde_json_bridge_rs::json::VecSerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_array();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return ConvertVecSerdeJsonToVector(std::move(rs_result).value());
}

absl::StatusOr<std::string> SerdeJson::GetFieldString(
    absl::string_view key) const {
  rs_std::Result<serde_json_bridge_rs::raw_string::RawString,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field_string(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return FromRustRawString(std::move(rs_result).value());
}

absl::StatusOr<bool> SerdeJson::GetFieldBool(absl::string_view key) const {
  rs_std::Result<bool, serde_json_bridge_rs::raw_string::RawString> rs_result =
      json_obj_.get_field_bool(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<int64_t> SerdeJson::GetFieldInt(absl::string_view key) const {
  rs_std::Result<int64_t, serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field_int(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> SerdeJson::GetFieldDouble(absl::string_view key) const {
  rs_std::Result<double, serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field_double(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::vector<SerdeJson>> SerdeJson::GetFieldArray(
    absl::string_view key) const {
  rs_std::Result<serde_json_bridge_rs::json::VecSerdeJson,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_field_array(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return ConvertVecSerdeJsonToVector(std::move(rs_result).value());
}

bool SerdeJson::IsNull() const { return json_obj_.is_null(); }

bool SerdeJson::IsObject() const { return json_obj_.is_object(); }

bool SerdeJson::IsArray() const { return json_obj_.is_array(); }

bool SerdeJson::IsString() const { return json_obj_.is_string(); }

bool SerdeJson::IsNumber() const { return json_obj_.is_number(); }

bool SerdeJson::IsInt() const { return json_obj_.is_i64(); }

bool SerdeJson::IsDouble() const { return json_obj_.is_f64(); }

bool SerdeJson::IsBool() const { return json_obj_.is_boolean(); }

std::string SerdeJson::ToString(bool sort_keys) const {
  return FromRustRawString(json_obj_.to_string(sort_keys));
}

absl::StatusOr<std::vector<std::string>> SerdeJson::GetKeys() const {
  rs_std::Result<serde_json_bridge_rs::json::VecRawString,
                 serde_json_bridge_rs::raw_string::RawString>
      rs_result = json_obj_.get_keys();

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }

  serde_json_bridge_rs::json::VecRawString rust_raw_strings =
      std::move(rs_result).value();
  std::vector<std::string> keys;
  keys.reserve(rust_raw_strings.len());
  std::transform(rust_raw_strings.as_ptr(),
                 rust_raw_strings.as_ptr() + rust_raw_strings.len(),
                 std::back_inserter(keys), FromRustRawString);
  return keys;
}

absl::StatusOr<bool> SerdeJson::HasField(absl::string_view key) const {
  rs_std::Result<bool, serde_json_bridge_rs::raw_string::RawString> rs_result =
      json_obj_.has_field(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<::google::protobuf::Struct> SerdeJson::ToProtoStruct() const {
  google::protobuf::Struct result;
  ASSIGN_OR_RETURN(const std::vector<std::string> keys, GetKeys());
  for (absl::string_view key : keys) {
    ASSIGN_OR_RETURN(const SerdeJson field, GetField(key));
    ASSIGN_OR_RETURN(::google::protobuf::Value value, field.ToProtoValue());
    result.mutable_fields()->insert({std::string(key), std::move(value)});
  }
  return result;
}

absl::StatusOr<::google::protobuf::Value> SerdeJson::ToProtoValue() const {
  google::protobuf::Value result;

  if (IsObject()) {
    ASSIGN_OR_RETURN(*result.mutable_struct_value(), ToProtoStruct());
  } else if (IsString()) {
    ASSIGN_OR_RETURN(*result.mutable_string_value(), GetString());
  } else if (IsInt()) {
    ASSIGN_OR_RETURN(int64_t number_value, GetInt());
    result.set_number_value(number_value);
  } else if (IsDouble()) {
    ASSIGN_OR_RETURN(double number_value, GetDouble());
    result.set_number_value(number_value);
  } else if (IsBool()) {
    ASSIGN_OR_RETURN(bool bool_value, GetBool());
    result.set_bool_value(bool_value);
  } else if (IsArray()) {
    ::google::protobuf::ListValue* list_value = result.mutable_list_value();
    ASSIGN_OR_RETURN(std::vector<SerdeJson> array_value, GetArray());
    for (const SerdeJson& element : array_value) {
      ASSIGN_OR_RETURN(*list_value->add_values(), element.ToProtoValue());
    }
  } else if (IsNull()) {
    result.set_null_value(::google::protobuf::NullValue::NULL_VALUE);
  } else {
    return absl::FailedPreconditionError(
        absl::StrCat("Unexpected type in the object: ", ToString()));
  }

  return result;
}

absl::Status SerdeJson::AddFieldInt(absl::string_view key, int64_t value) {
  return json_obj_
      .add_field_int(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          value)
      .status();
}

absl::Status SerdeJson::AddFieldBool(absl::string_view key, bool value) {
  return json_obj_
      .add_field_bool(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          value)
      .status();
}

absl::Status SerdeJson::AddFieldString(absl::string_view key,
                                       const absl::string_view value) {
  return json_obj_
      .add_field_string(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(value.data()), value.size()))
      .status();
}

absl::Status SerdeJson::AddFieldDouble(absl::string_view key, double value) {
  return json_obj_
      .add_field_double(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          value)
      .status();
}

absl::Status SerdeJson::AddFieldNull(absl::string_view key) {
  return json_obj_
      .add_field_null(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()))
      .status();
}

absl::Status SerdeJson::AddFieldObject(absl::string_view key,
                                       const SerdeJson& value) {
  return json_obj_
      .add_field_object(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          value.json_obj_)
      .status();
}

absl::Status SerdeJson::AddFieldArray(absl::string_view key,
                                      absl::Span<const SerdeJson> value) {
  std::vector<SerdeJson> arr(value.begin(), value.end());
  return AddFieldArray(key, std::move(arr));
}

absl::Status SerdeJson::AddFieldArray(absl::string_view key,
                                      std::vector<SerdeJson> value) {
  std::vector<serde_json_bridge_rs::json::SerdeJson> arr;
  arr.reserve(value.size());
  for (auto& v : value) {
    arr.push_back(std::move(v.json_obj_));
  }
  return json_obj_
      .add_field_array(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(key.data()), key.size()),
          arr)
      .status();
}

bool SerdeJson::operator==(const SerdeJson& other) const {
  return json_obj_.is_json_equal(other.json_obj_);
}

bool SerdeJson::operator!=(const SerdeJson& other) const {
  return !json_obj_.is_json_equal(other.json_obj_);
}

}  // namespace security::json::serde_json_bridge
