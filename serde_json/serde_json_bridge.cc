#include "serde_json_bridge.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <iterator>
#include <string>
#include <utility>
#include <vector>

#include <google/protobuf/struct.pb.h>
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::json::serde_json_bridge {

static std::string FromRustRawString(
    const rust::raw_string::RawString& raw_string) {
  return std::string(reinterpret_cast<const char*>(raw_string.as_ptr()),
                     raw_string.len());
}

inline absl::Status ToStatus(rust::json::Status status) {
  if (!status.has_value()) {
    return absl::OkStatus();
  }
  return absl::InvalidArgumentError(
      FromRustRawString(std::move(status).err()));
}

SerdeJson::SerdeJson(rust::json::SerdeJson sj)
    : json_obj_(std::move(sj)) {}

// This function has to be a member of `SerdeJson` class as it uses
// private constructor `SerdeJson(rust::json::SerdeJson)`.
std::vector<SerdeJson> SerdeJson::ConvertVecSerdeJsonToVector(
    const rust::json::VecSerdeJson& rs_vec_serde_json) {
  std::vector<SerdeJson> ret_vec;
  ret_vec.reserve(rs_vec_serde_json.len());
  for (size_t i = 0; i < rs_vec_serde_json.len(); ++i) {
    ret_vec.push_back(SerdeJson(rs_vec_serde_json.as_ptr()[i]));
  }
  return ret_vec;
}

absl::StatusOr<SerdeJson> SerdeJson::CreateObject() {
  return SerdeJson(rust::json::SerdeJson::create_object());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateInt(int64_t value) {
  return SerdeJson(rust::json::SerdeJson::create_int(value));
}

absl::StatusOr<SerdeJson> SerdeJson::CreateDouble(double value) {
  rs_std::Result<rust::json::SerdeJson,
                 rust::raw_string::RawString>
      rs_result = rust::json::SerdeJson::create_double(value);

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateBool(bool value) {
  return SerdeJson(rust::json::SerdeJson::create_bool(value));
}

absl::StatusOr<SerdeJson> SerdeJson::CreateNull() {
  return SerdeJson(rust::json::SerdeJson::create_null());
}

absl::StatusOr<SerdeJson> SerdeJson::CreateString(absl::string_view value) {
  rs_std::Result<rust::json::SerdeJson,
                 rust::raw_string::RawString>
      rs_result = rust::json::SerdeJson::create_string(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(value.data()), value.size()));

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::Parse(absl::string_view data) {
  rs_std::Result<rust::json::SerdeJson,
                 rust::raw_string::RawString>
      rs_result = rust::json::SerdeJson::parse_string(
          absl::Span<const uint8_t>(
              reinterpret_cast<const uint8_t*>(data.data()), data.size()));

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }
  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<SerdeJson> SerdeJson::GetField(absl::string_view key) const {
  rs_std::Result<rust::json::SerdeJson,
                 rust::raw_string::RawString>
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
  rs_std::Result<rust::json::SerdeJson,
                 rust::raw_string::RawString>
      rs_result = json_obj_.get_field_object(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return SerdeJson(std::move(rs_result).value());
}

absl::StatusOr<bool> SerdeJson::GetBool() const {
  rs_std::Result<bool, rust::raw_string::RawString> rs_result =
      json_obj_.get_bool();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::string> SerdeJson::GetString() const {
  rs_std::Result<rust::raw_string::RawString,
                 rust::raw_string::RawString>
      rs_result = json_obj_.get_string();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return FromRustRawString(std::move(rs_result).value());
}

absl::StatusOr<int64_t> SerdeJson::GetInt() const {
  rs_std::Result<int64_t, rust::raw_string::RawString>
      rs_result = json_obj_.get_int();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> SerdeJson::GetDouble() const {
  rs_std::Result<double, rust::raw_string::RawString>
      rs_result = json_obj_.get_double();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::vector<SerdeJson>> SerdeJson::GetArray() const {
  rs_std::Result<rust::json::VecSerdeJson,
                 rust::raw_string::RawString>
      rs_result = json_obj_.get_array();

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return ConvertVecSerdeJsonToVector(std::move(rs_result).value());
}

absl::StatusOr<std::string> SerdeJson::GetFieldString(
    absl::string_view key) const {
  rs_std::Result<rust::raw_string::RawString,
                 rust::raw_string::RawString>
      rs_result = json_obj_.get_field_string(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return FromRustRawString(std::move(rs_result).value());
}

absl::StatusOr<bool> SerdeJson::GetFieldBool(absl::string_view key) const {
  rs_std::Result<bool, rust::raw_string::RawString> rs_result =
      json_obj_.get_field_bool(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<int64_t> SerdeJson::GetFieldInt(absl::string_view key) const {
  rs_std::Result<int64_t, rust::raw_string::RawString>
      rs_result = json_obj_.get_field_int(absl::Span<const uint8_t>(
          reinterpret_cast<const uint8_t*>(key.data()), key.size()));

  if (!rs_result.has_value()) {
    return absl::FailedPreconditionError(
        FromRustRawString(std::move(rs_result).err()));
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> SerdeJson::GetFieldDouble(absl::string_view key) const {
  rs_std::Result<double, rust::raw_string::RawString>
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
  rs_std::Result<rust::json::VecSerdeJson,
                 rust::raw_string::RawString>
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
  rs_std::Result<rust::json::VecRawString,
                 rust::raw_string::RawString>
      rs_result = json_obj_.get_keys();

  if (!rs_result.has_value()) {
    return absl::InvalidArgumentError(
        FromRustRawString(std::move(rs_result).err()));
  }

  rust::json::VecRawString rust_raw_strings =
      std::move(rs_result).value();
  std::vector<std::string> keys;
  keys.reserve(rust_raw_strings.len());
  std::transform(rust_raw_strings.as_ptr(),
                 rust_raw_strings.as_ptr() + rust_raw_strings.len(),
                 std::back_inserter(keys), FromRustRawString);
  return keys;
}

absl::StatusOr<bool> SerdeJson::HasField(absl::string_view key) const {
  rs_std::Result<bool, rust::raw_string::RawString> rs_result =
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
  absl::StatusOr<std::vector<std::string>> keys_or = GetKeys();
  if (!keys_or.ok()) return keys_or.status();
  for (absl::string_view key : *keys_or) {
    absl::StatusOr<SerdeJson> field_or = GetField(key);
    if (!field_or.ok()) return field_or.status();
    absl::StatusOr<::google::protobuf::Value> value_or =
        field_or->ToProtoValue();
    if (!value_or.ok()) return value_or.status();
    result.mutable_fields()->insert({std::string(key), std::move(*value_or)});
  }
  return result;
}

absl::StatusOr<::google::protobuf::Value> SerdeJson::ToProtoValue() const {
  google::protobuf::Value result;

  if (IsObject()) {
    absl::StatusOr<::google::protobuf::Struct> struct_or = ToProtoStruct();
    if (!struct_or.ok()) return struct_or.status();
    *result.mutable_struct_value() = std::move(*struct_or);
  } else if (IsString()) {
    absl::StatusOr<std::string> string_or = GetString();
    if (!string_or.ok()) return string_or.status();
    result.set_string_value(std::move(*string_or));
  } else if (IsInt()) {
    absl::StatusOr<int64_t> int_or = GetInt();
    if (!int_or.ok()) return int_or.status();
    result.set_number_value(*int_or);
  } else if (IsDouble()) {
    absl::StatusOr<double> double_or = GetDouble();
    if (!double_or.ok()) return double_or.status();
    result.set_number_value(*double_or);
  } else if (IsBool()) {
    absl::StatusOr<bool> bool_or = GetBool();
    if (!bool_or.ok()) return bool_or.status();
    result.set_bool_value(*bool_or);
  } else if (IsArray()) {
    ::google::protobuf::ListValue* list_value = result.mutable_list_value();
    absl::StatusOr<std::vector<SerdeJson>> array_or = GetArray();
    if (!array_or.ok()) return array_or.status();
    for (const SerdeJson& element : *array_or) {
      absl::StatusOr<::google::protobuf::Value> val_or = element.ToProtoValue();
      if (!val_or.ok()) return val_or.status();
      *list_value->add_values() = std::move(*val_or);
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
  return ToStatus(json_obj_.add_field_int(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      value));
}

absl::Status SerdeJson::AddFieldBool(absl::string_view key, bool value) {
  return ToStatus(json_obj_.add_field_bool(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      value));
}

absl::Status SerdeJson::AddFieldString(absl::string_view key,
                                       const absl::string_view value) {
  return ToStatus(json_obj_.add_field_string(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(value.data()),
                                value.size())));
}

absl::Status SerdeJson::AddFieldDouble(absl::string_view key, double value) {
  return ToStatus(json_obj_.add_field_double(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      value));
}

absl::Status SerdeJson::AddFieldNull(absl::string_view key) {
  return ToStatus(json_obj_.add_field_null(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(key.data()), key.size())));
}

absl::Status SerdeJson::AddFieldObject(absl::string_view key,
                                       const SerdeJson& value) {
  return ToStatus(json_obj_.add_field_object(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      value.json_obj_));
}

absl::Status SerdeJson::AddFieldArray(absl::string_view key,
                                      absl::Span<const SerdeJson> value) {
  std::vector<SerdeJson> arr(value.begin(), value.end());
  return AddFieldArray(key, std::move(arr));
}

absl::Status SerdeJson::AddFieldArray(absl::string_view key,
                                      std::vector<SerdeJson> value) {
  std::vector<rust::json::SerdeJson> arr;
  arr.reserve(value.size());
  for (auto& v : value) {
    arr.push_back(std::move(v.json_obj_));
  }
  return ToStatus(json_obj_.add_field_array(
      absl::Span<const uint8_t>(reinterpret_cast<const uint8_t*>(key.data()),
                                key.size()),
      arr));
}

bool SerdeJson::operator==(const SerdeJson& other) const {
  return json_obj_.is_json_equal(other.json_obj_);
}

bool SerdeJson::operator!=(const SerdeJson& other) const {
  return !json_obj_.is_json_equal(other.json_obj_);
}

}  // namespace security::json::serde_json_bridge
