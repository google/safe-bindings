#include "fast_serde_json_bridge.h"

#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

#include <google/protobuf/struct.pb.h>
#include "rust/fast_serde_json_rs.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::json::fast_serde_json_bridge {

namespace {

std::string StringFromVec(const std::vector<uint8_t>& bytes) {
  if (bytes.empty()) return "";
  return std::string(reinterpret_cast<const char*>(bytes.data()), bytes.size());
}

absl::Span<const uint8_t> ToBytesSpan(absl::string_view view) {
  return absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(view.data()), view.size());
}

}  // namespace

FastSerdeJson::FastSerdeJson(fast_serde_json_rs::FastSerdeJson sj)
    : json_obj_(std::move(sj)) {}

absl::StatusOr<FastSerdeJson> FastSerdeJson::CloneSubtree(NodeHandle handle) {
  rs_std::Result<fast_serde_json_rs::FastSerdeJson, fast_serde_json_rs::Status>
      rs_result = json_obj_.clone_subtree(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }
  return FastSerdeJson(std::move(rs_result).value());
}

FastSerdeJson FastSerdeJson::CreateObject() {
  return FastSerdeJson(fast_serde_json_rs::FastSerdeJson::new_object());
}

FastSerdeJson FastSerdeJson::CreateArray() {
  return FastSerdeJson(fast_serde_json_rs::FastSerdeJson::new_array());
}

FastSerdeJson FastSerdeJson::CreateInt(int64_t value) {
  return FastSerdeJson(fast_serde_json_rs::FastSerdeJson::from_i64(value));
}

absl::StatusOr<FastSerdeJson> FastSerdeJson::CreateDouble(double value) {
  rs_std::Result<fast_serde_json_rs::FastSerdeJson, fast_serde_json_rs::Status>
      rs_result = fast_serde_json_rs::FastSerdeJson::try_from_f64(value);

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }
  return FastSerdeJson(std::move(rs_result).value());
}

FastSerdeJson FastSerdeJson::CreateBool(bool value) {
  return FastSerdeJson(fast_serde_json_rs::FastSerdeJson::from_bool(value));
}

FastSerdeJson FastSerdeJson::CreateNull() {
  return FastSerdeJson(fast_serde_json_rs::FastSerdeJson::new_null());
}

absl::StatusOr<FastSerdeJson> FastSerdeJson::CreateString(
    absl::string_view value) {
  rs_std::Result<fast_serde_json_rs::FastSerdeJson, fast_serde_json_rs::Status>
      rs_result =
          fast_serde_json_rs::FastSerdeJson::try_from_utf8(ToBytesSpan(value));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }
  return FastSerdeJson(std::move(rs_result).value());
}

absl::StatusOr<FastSerdeJson> FastSerdeJson::Parse(absl::string_view data) {
  rs_std::Result<fast_serde_json_rs::FastSerdeJson, fast_serde_json_rs::Status>
      rs_result =
          fast_serde_json_rs::FastSerdeJson::try_parse(ToBytesSpan(data));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }
  return FastSerdeJson(std::move(rs_result).value());
}

absl::StatusOr<FastSerdeJson::NodeHandle> FastSerdeJson::GetField(
    absl::string_view key, NodeHandle handle) {
  rs_std::Result<fast_serde_json_rs::NodeHandle, fast_serde_json_rs::Status>
      rs_result = json_obj_.get_field(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<FastSerdeJson::NodeHandle> FastSerdeJson::GetFieldObject(
    absl::string_view key, NodeHandle handle) {
  rs_std::Result<fast_serde_json_rs::NodeHandle, fast_serde_json_rs::Status>
      rs_result =
          json_obj_.get_field_object(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<bool> FastSerdeJson::GetBool(NodeHandle handle) {
  rs_std::Result<bool, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_bool(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::string> FastSerdeJson::GetString(NodeHandle handle) {
  rs_std::Result<std::vector<uint8_t>, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_string(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return StringFromVec(std::move(rs_result).value());
}

absl::StatusOr<int64_t> FastSerdeJson::GetInt(NodeHandle handle) {
  rs_std::Result<int64_t, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_int(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> FastSerdeJson::GetDouble(NodeHandle handle) {
  rs_std::Result<double, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_double(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::vector<FastSerdeJson::NodeHandle>> FastSerdeJson::GetArray(
    NodeHandle handle) {
  rs_std::Result<std::vector<fast_serde_json_rs::NodeHandle>,
                 fast_serde_json_rs::Status>
      rs_result = json_obj_.get_array(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<FastSerdeJson::NodeHandle> FastSerdeJson::GetArrayElement(
    size_t index, NodeHandle handle) {
  rs_std::Result<fast_serde_json_rs::NodeHandle, fast_serde_json_rs::Status>
      rs_result = json_obj_.get_array_element(index, std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::string> FastSerdeJson::GetFieldString(absl::string_view key,
                                                          NodeHandle handle) {
  rs_std::Result<std::vector<uint8_t>, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_field_string(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return StringFromVec(std::move(rs_result).value());
}

absl::StatusOr<bool> FastSerdeJson::GetFieldBool(absl::string_view key,
                                                 NodeHandle handle) {
  rs_std::Result<bool, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_field_bool(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<int64_t> FastSerdeJson::GetFieldInt(absl::string_view key,
                                                   NodeHandle handle) {
  rs_std::Result<int64_t, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_field_int(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<double> FastSerdeJson::GetFieldDouble(absl::string_view key,
                                                     NodeHandle handle) {
  rs_std::Result<double, fast_serde_json_rs::Status> rs_result =
      json_obj_.get_field_double(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<std::vector<FastSerdeJson::NodeHandle>>
FastSerdeJson::GetFieldArray(absl::string_view key, NodeHandle handle) {
  rs_std::Result<std::vector<fast_serde_json_rs::NodeHandle>,
                 fast_serde_json_rs::Status>
      rs_result =
          json_obj_.get_field_array(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<FastSerdeJson::NodeHandle> FastSerdeJson::GetFieldArrayElement(
    absl::string_view key, size_t index, NodeHandle handle) {
  rs_std::Result<fast_serde_json_rs::NodeHandle, fast_serde_json_rs::Status>
      rs_result = json_obj_.get_field_array_element(ToBytesSpan(key), index,
                                                    std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

bool FastSerdeJson::IsNull(NodeHandle handle) {
  return json_obj_.is_null(std::move(handle));
}

absl::StatusOr<bool> FastSerdeJson::IsEmpty(NodeHandle handle) {
  rs_std::Result<bool, fast_serde_json_rs::Status> rs_result =
      json_obj_.is_empty(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

bool FastSerdeJson::IsObject(NodeHandle handle) {
  return json_obj_.is_object(std::move(handle));
}

bool FastSerdeJson::IsArray(NodeHandle handle) {
  return json_obj_.is_array(std::move(handle));
}

bool FastSerdeJson::IsString(NodeHandle handle) {
  return json_obj_.is_string(std::move(handle));
}

bool FastSerdeJson::IsNumber(NodeHandle handle) {
  return json_obj_.is_number(std::move(handle));
}

bool FastSerdeJson::IsInt(NodeHandle handle) {
  return json_obj_.is_i64(std::move(handle));
}

bool FastSerdeJson::IsDouble(NodeHandle handle) {
  return json_obj_.is_f64(std::move(handle));
}

bool FastSerdeJson::IsBool(NodeHandle handle) {
  return json_obj_.is_boolean(std::move(handle));
}

std::string FastSerdeJson::ToString(bool sort_keys, NodeHandle handle) {
  return StringFromVec(json_obj_.to_string(sort_keys, std::move(handle)));
}

absl::StatusOr<std::vector<std::string>> FastSerdeJson::GetKeys(
    NodeHandle handle) {
  rs_std::Result<std::vector<std::vector<uint8_t>>, fast_serde_json_rs::Status>
      rs_result = json_obj_.keys(std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  const std::vector<std::vector<uint8_t>>& rust_raw_strings = rs_result.value();
  std::vector<std::string> keys;
  keys.reserve(rust_raw_strings.size());
  for (const std::vector<uint8_t>& raw : rust_raw_strings) {
    keys.push_back(StringFromVec(raw));
  }
  return keys;
}

absl::StatusOr<bool> FastSerdeJson::HasField(absl::string_view key,
                                             NodeHandle handle) {
  rs_std::Result<bool, fast_serde_json_rs::Status> rs_result =
      json_obj_.has_field(ToBytesSpan(key), std::move(handle));

  if (!rs_result.has_value()) {
    return rs_result.err().status();
  }

  return std::move(rs_result).value();
}

absl::StatusOr<google::protobuf::Struct> FastSerdeJson::ToProtoStruct(
    NodeHandle handle) {
  google::protobuf::Struct result;
  absl::StatusOr<std::vector<std::string>> keys = GetKeys(handle);
  if (!keys.ok()) return keys.status();
  for (std::string& key : *keys) {
    absl::StatusOr<NodeHandle> field_handle = GetField(key, handle);
    if (!field_handle.ok()) return field_handle.status();
    absl::StatusOr<google::protobuf::Value> value = ToProtoValue(*field_handle);
    if (!value.ok()) return value.status();
    result.mutable_fields()->insert({std::move(key), std::move(*value)});
  }
  return result;
}

absl::StatusOr<google::protobuf::Value> FastSerdeJson::ToProtoValue(
    NodeHandle handle) {
  google::protobuf::Value result;

  if (IsObject(handle)) {
    absl::StatusOr<google::protobuf::Struct> proto_struct =
        ToProtoStruct(handle);
    if (!proto_struct.ok()) return proto_struct.status();
    *result.mutable_struct_value() = std::move(*proto_struct);
  } else if (IsString(handle)) {
    absl::StatusOr<std::string> str = GetString(handle);
    if (!str.ok()) return str.status();
    result.set_string_value(std::move(*str));
  } else if (IsInt(handle)) {
    absl::StatusOr<int64_t> int_val = GetInt(handle);
    if (!int_val.ok()) return int_val.status();
    result.set_number_value(*int_val);
  } else if (IsDouble(handle)) {
    absl::StatusOr<double> double_val = GetDouble(handle);
    if (!double_val.ok()) return double_val.status();
    result.set_number_value(*double_val);
  } else if (IsBool(handle)) {
    absl::StatusOr<bool> bool_val = GetBool(handle);
    if (!bool_val.ok()) return bool_val.status();
    result.set_bool_value(*bool_val);
  } else if (IsArray(handle)) {
    google::protobuf::ListValue* list_value = result.mutable_list_value();
    absl::StatusOr<std::vector<NodeHandle>> array = GetArray(handle);
    if (!array.ok()) return array.status();
    for (const NodeHandle& element_handle : *array) {
      absl::StatusOr<google::protobuf::Value> val =
          ToProtoValue(element_handle);
      if (!val.ok()) return val.status();
      *list_value->add_values() = std::move(*val);
    }
  } else if (IsNull(handle)) {
    result.set_null_value(google::protobuf::NullValue::NULL_VALUE);
  } else {
    return absl::FailedPreconditionError(
        absl::StrCat("Unexpected type in the object: ",
                     ToString(/*sort_keys=*/true, handle)));
  }

  return result;
}

absl::Status FastSerdeJson::AddFieldInt(absl::string_view key, int64_t value,
                                        NodeHandle handle) {
  return json_obj_.add_field_int(ToBytesSpan(key), value, std::move(handle))
      .status();
}

absl::Status FastSerdeJson::AddFieldBool(absl::string_view key, bool value,
                                         NodeHandle handle) {
  return json_obj_.add_field_bool(ToBytesSpan(key), value, std::move(handle))
      .status();
}

absl::Status FastSerdeJson::AddFieldString(absl::string_view key,
                                           const absl::string_view value,
                                           NodeHandle handle) {
  return json_obj_
      .add_field_string(ToBytesSpan(key), ToBytesSpan(value), std::move(handle))
      .status();
}

absl::Status FastSerdeJson::AddFieldDouble(absl::string_view key, double value,
                                           NodeHandle handle) {
  return json_obj_.add_field_double(ToBytesSpan(key), value, std::move(handle))
      .status();
}

absl::Status FastSerdeJson::AddFieldNull(absl::string_view key,
                                         NodeHandle handle) {
  return json_obj_.add_field_null(ToBytesSpan(key), std::move(handle)).status();
}

absl::Status FastSerdeJson::AddFieldObject(absl::string_view key,
                                           FastSerdeJson value,
                                           NodeHandle handle) {
  return json_obj_
      .add_field_object(ToBytesSpan(key), std::move(value.json_obj_),
                        std::move(handle))
      .status();
}

absl::Status FastSerdeJson::AddFieldArray(absl::string_view key,
                                          std::vector<FastSerdeJson> value,
                                          NodeHandle handle) {
  std::vector<fast_serde_json_rs::FastSerdeJson> arr;
  arr.reserve(value.size());
  for (FastSerdeJson& v : value) {
    arr.push_back(std::move(v.json_obj_));
  }
  return json_obj_
      .add_field_array(ToBytesSpan(key), std::move(arr), std::move(handle))
      .status();
}

bool FastSerdeJson::operator==(const FastSerdeJson& other) const {
  return json_obj_.is_json_equal(other.json_obj_);
}

}  // namespace security::json::fast_serde_json_bridge
