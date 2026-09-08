#include "fast_serde_json_bridge.h"

#include <limits>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <google/protobuf/struct.pb.h>
#include "net/proto2/contrib/parse_proto/parse_text_proto.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/strings/string_view.h"
#include "third_party/json/include/nlohmann/json.hpp"

namespace {

using ::absl_testing::StatusIs;
using ::proto2::contrib::parse_proto::ParseTextProtoOrDie;
using ::security::json::fast_serde_json_bridge::FastSerdeJson;
using ::testing::ElementsAre;
using ::testing::EqualsProto;
using ::testing::status::IsOkAndHolds;

TEST(FastSerdeJsonBridge, SimpleParse) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  EXPECT_OK(FastSerdeJson::Parse(kJsonString));
}

TEST(FastSerdeJsonBridge, FailParse) {
  constexpr absl::string_view kInvalidJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "}";

  EXPECT_THAT(FastSerdeJson::Parse(kInvalidJsonString),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, CheckFieldGetter) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_OK(json.GetField("firstName"));
  EXPECT_OK(json.GetField("lastName"));
  EXPECT_THAT(json.GetField("phone"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, CheckGetBool) {
  constexpr absl::string_view kJsonString = "true";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetBool(), IsOkAndHolds(true));
}

TEST(FastSerdeJsonBridge, CheckGetString) {
  constexpr absl::string_view kJsonString = "\"string\"";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetString(), IsOkAndHolds("string"));
}

TEST(FastSerdeJsonBridge, CheckGetInt) {
  constexpr absl::string_view kJsonString = "1234";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetInt(), IsOkAndHolds(1234));
}

TEST(FastSerdeJsonBridge, CheckGetDouble) {
  constexpr absl::string_view kJsonString = "1337.1234";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetDouble(), IsOkAndHolds(1337.1234));
}

TEST(FastSerdeJsonBridge, CheckGetArray) {
  constexpr absl::string_view kJsonString =
      "[\n"
      "  \"first\",\n"
      "  2,\n"
      "  \"third\",\n"
      "  4\n"
      "]";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(std::vector<FastSerdeJson::NodeHandle> array,
                       json.GetArray());
  ASSERT_EQ(array.size(), 4);

  EXPECT_THAT(json.GetString(array[0]), IsOkAndHolds("first"));
  EXPECT_THAT(json.GetInt(array[1]), IsOkAndHolds(2));
  EXPECT_THAT(json.GetString(array[2]), IsOkAndHolds("third"));
  EXPECT_THAT(json.GetInt(array[3]), IsOkAndHolds(4));
}

TEST(FastSerdeJsonBridge, CheckGetArrayNotFromArray) {
  constexpr absl::string_view kJsonString = "\"first\"";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetArray(), StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, CheckGetArrayElement) {
  constexpr absl::string_view kJsonString =
      "[\n"
      "  \"first\",\n"
      "  2,\n"
      "  \"third\",\n"
      "  4\n"
      "]";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem0,
                       json.GetArrayElement(0));
  EXPECT_THAT(json.GetString(elem0), IsOkAndHolds("first"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem1,
                       json.GetArrayElement(1));
  EXPECT_THAT(json.GetInt(elem1), IsOkAndHolds(2));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem2,
                       json.GetArrayElement(2));
  EXPECT_THAT(json.GetString(elem2), IsOkAndHolds("third"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem3,
                       json.GetArrayElement(3));
  EXPECT_THAT(json.GetInt(elem3), IsOkAndHolds(4));
}

TEST(FastSerdeJsonBridge, CheckGetArrayElementOutOfBounds) {
  constexpr absl::string_view kJsonString =
      "[\n"
      "  \"first\",\n"
      "  2\n"
      "]";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetArrayElement(2), StatusIs(absl::StatusCode::kOutOfRange));
}

TEST(FastSerdeJsonBridge, CheckGetArrayElementNotFromArray) {
  constexpr absl::string_view kJsonString = "\"first\"";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetArrayElement(0),
              StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, GetFieldString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldString("firstName"), IsOkAndHolds("John"));
  EXPECT_THAT(json.GetFieldString("lastName"), IsOkAndHolds("Doe"));
}

TEST(FastSerdeJsonBridge, GetFieldBool) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": true,\n"
      "  \"value2\": false\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldBool("value1"), IsOkAndHolds(true));
  EXPECT_THAT(json.GetFieldBool("value2"), IsOkAndHolds(false));
}

TEST(FastSerdeJsonBridge, GetFieldInt) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": 123,\n"
      "  \"value2\": -444\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldInt("value1"), IsOkAndHolds(123));
  EXPECT_THAT(json.GetFieldInt("value2"), IsOkAndHolds(-444));
}

TEST(FastSerdeJsonBridge, GetFieldDouble) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldDouble("value1"), IsOkAndHolds(1.0));
  EXPECT_THAT(json.GetFieldDouble("value2"), IsOkAndHolds(3.0));
}

TEST(FastSerdeJsonBridge, GetFieldObject) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"obj\": {"
      "    \"value1\": 1.0\n"
      "  }\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle obj_handle,
                       json.GetFieldObject("obj"));
  EXPECT_THAT(json.GetFieldDouble("value1", obj_handle), IsOkAndHolds(1.0));
}

TEST(FastSerdeJsonBridge, GetFieldArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": [\n"
      "    \"first\",\n"
      "    2,\n"
      "    \"third\",\n"
      "    4\n"
      " ]\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(std::vector<FastSerdeJson::NodeHandle> array,
                       json.GetFieldArray("value"));
  ASSERT_EQ(array.size(), 4);

  EXPECT_THAT(json.GetString(array[0]), IsOkAndHolds("first"));
  EXPECT_THAT(json.GetInt(array[1]), IsOkAndHolds(2));
  EXPECT_THAT(json.GetString(array[2]), IsOkAndHolds("third"));
  EXPECT_THAT(json.GetInt(array[3]), IsOkAndHolds(4));
}

TEST(FastSerdeJsonBridge, GetFieldArrayNotFromArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 1.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldArray("value"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, GetFieldArrayElement) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": [\n"
      "    \"first\",\n"
      "    2,\n"
      "    \"third\",\n"
      "    4\n"
      " ]\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem0,
                       json.GetFieldArrayElement("value", 0));
  EXPECT_THAT(json.GetString(elem0), IsOkAndHolds("first"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem1,
                       json.GetFieldArrayElement("value", 1));
  EXPECT_THAT(json.GetInt(elem1), IsOkAndHolds(2));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem2,
                       json.GetFieldArrayElement("value", 2));
  EXPECT_THAT(json.GetString(elem2), IsOkAndHolds("third"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle elem3,
                       json.GetFieldArrayElement("value", 3));
  EXPECT_THAT(json.GetInt(elem3), IsOkAndHolds(4));
}

TEST(FastSerdeJsonBridge, GetFieldArrayElementOutOfBounds) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": [\n"
      "    \"first\",\n"
      "    2\n"
      " ]\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldArrayElement("value", 2),
              StatusIs(absl::StatusCode::kOutOfRange));
}

TEST(FastSerdeJsonBridge, GetFieldArrayElementNotFromArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 1.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldArrayElement("value", 0),
              StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, GetFieldNotFromObject) {
  constexpr absl::string_view kJsonString = "1337.1234";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetFieldString("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetFieldBool("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetFieldDouble("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetFieldInt("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetFieldObject("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetFieldArray("test"),
              StatusIs(absl::StatusCode::kFailedPrecondition));
  EXPECT_THAT(json.GetDouble(), IsOkAndHolds(1337.1234));
}

TEST(FastSerdeJsonBridge, IsNull) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": null\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsNumber(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsEmpty) {
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("null"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(true))
        << "Actual JSON: " << json.ToString();
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("{}"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(true))
        << "Actual JSON: " << json.ToString();
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json,
                         FastSerdeJson::Parse("{\"a\": 1}"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("[]"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(true))
        << "Actual JSON: " << json.ToString();
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("[1]"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("\"\""));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("\"abc\""));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("0"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
  {
    ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse("true"));
    EXPECT_THAT(json.IsEmpty(), IsOkAndHolds(false));
  }
}

TEST(FastSerdeJsonBridge, IsObject) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": {}\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsObject(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsNumber(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": []\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsArray(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsNumber(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": \"\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsString(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsNumber(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsNumber) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsNumber(value));
  EXPECT_TRUE(json.IsInt(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
}

TEST(FastSerdeJsonBridge, IsBool) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": true\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsBool(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsNumber(value));
  EXPECT_FALSE(json.IsDouble(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsDouble) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsNumber(value));
  EXPECT_TRUE(json.IsDouble(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsInt(value));
}

TEST(FastSerdeJsonBridge, IsInt) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle value, json.GetField("value"));

  EXPECT_TRUE(json.IsNumber(value));
  EXPECT_TRUE(json.IsInt(value));
  EXPECT_FALSE(json.IsNull(value));
  EXPECT_FALSE(json.IsObject(value));
  EXPECT_FALSE(json.IsArray(value));
  EXPECT_FALSE(json.IsString(value));
  EXPECT_FALSE(json.IsBool(value));
  EXPECT_FALSE(json.IsDouble(value));
}

TEST(FastSerdeJsonBridge, GetKeys) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetKeys(), IsOkAndHolds(ElementsAre("firstName", "lastName",
                                                       "value1", "value2")));
}

TEST(FastSerdeJsonBridge, GetKeysPreservesInsertionOrder) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"z\": 1,\n"
      "  \"a\": 2,\n"
      "  \"m\": 3\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetKeys(), IsOkAndHolds(ElementsAre("z", "a", "m")));
}

TEST(FastSerdeJsonBridge, GetKeysNotFromObject) {
  constexpr absl::string_view kJsonString = "10";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.GetKeys(), StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(FastSerdeJsonBridge, ToString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_EQ(json.ToString(),
            "{\"firstName\":\"John\",\"lastName\":\"Doe\",\"value1\":1.0,"
            "\"value2\":3.0}");
}

TEST(FastSerdeJsonBridge, ToStringWithSorting) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"z\": 1,\n"
      "  \"a\": 2\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  // Default is sorted.
  EXPECT_EQ(json.ToString(), "{\"a\":2,\"z\":1}");
  EXPECT_EQ(json.ToString(/*sort_keys=*/true), "{\"a\":2,\"z\":1}");

  // Can opt-out of sorting.
  EXPECT_EQ(json.ToString(/*sort_keys=*/false), "{\"z\":1,\"a\":2}");
}

TEST(FastSerdeJsonBridge, ToStringWithNestedObjects) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"attributes\": [\n"
      "    {\n"
      "      \"zzz\": false,\n"
      "      \"aaa\": true\n"
      "    }\n"
      "  ]\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  // Default is sorted.
  EXPECT_EQ(json.ToString(), "{\"attributes\":[{\"aaa\":true,\"zzz\":false}]}");
}

TEST(FastSerdeJsonBridge, JsonBoolToProto) {
  google::protobuf::Value expected_value =
      ParseTextProtoOrDie(R"pb(bool_value: true)pb");
  constexpr absl::string_view kJsonString = "true";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonStringToProto) {
  google::protobuf::Value expected_value =
      ParseTextProtoOrDie(R"pb(string_value: "string")pb");
  constexpr absl::string_view kJsonString = "\"string\"";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonIntToProto) {
  google::protobuf::Value expected_value =
      ParseTextProtoOrDie(R"pb(number_value: 1234)pb");
  constexpr absl::string_view kJsonString = "1234";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonDoubleToProto) {
  google::protobuf::Value expected_value =
      ParseTextProtoOrDie(R"pb(number_value: 1337.1234)pb");
  constexpr absl::string_view kJsonString = "1337.1234";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonArrayToProto) {
  google::protobuf::Value expected_value = ParseTextProtoOrDie(R"pb(
    list_value {
      values { string_value: "first" }
      values { number_value: 2 }
      values { string_value: "third" }
      values { number_value: 4 }
    }
  )pb");

  constexpr absl::string_view kJsonString =
      "[\n"
      "  \"first\",\n"
      "  2,\n"
      "  \"third\",\n"
      "  4\n"
      "]";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonNULLToProto) {
  google::protobuf::Value expected_value =
      ParseTextProtoOrDie(R"pb(null_value: NULL_VALUE)pb");
  constexpr absl::string_view kJsonString = "null";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonObjectToValue) {
  google::protobuf::Value expected_value = ParseTextProtoOrDie(R"pb(
    struct_value {
      fields {
        key: "firstName"
        value { string_value: "John" }
      }
      fields {
        key: "lastName"
        value { string_value: "Doe" }
      }
      fields {
        key: "value1"
        value { number_value: 1.0 }
      }
      fields {
        key: "value2"
        value { number_value: 3.0 }
      }
    }
  )pb");

  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoValue(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, JsonObjectToProtoStruct) {
  google::protobuf::Struct expected_value = ParseTextProtoOrDie(R"pb(
    fields {
      key: "firstName"
      value { string_value: "John" }
    }
    fields {
      key: "lastName"
      value { string_value: "Doe" }
    }
    fields {
      key: "value1"
      value { number_value: 1.0 }
    }
    fields {
      key: "value2"
      value { number_value: 3.0 }
    }
  )pb");

  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));
  EXPECT_THAT(json.ToProtoStruct(), IsOkAndHolds(EqualsProto(expected_value)));
}

TEST(FastSerdeJsonBridge, CreateInt) {
  FastSerdeJson json = FastSerdeJson::CreateInt(123);

  EXPECT_TRUE(json.IsNumber());
  EXPECT_TRUE(json.IsInt());
  EXPECT_THAT(json.GetInt(), IsOkAndHolds(123));
  EXPECT_EQ(json.ToString(), "123");
}

TEST(FastSerdeJsonBridge, CreateBool) {
  FastSerdeJson json = FastSerdeJson::CreateBool(false);

  EXPECT_TRUE(json.IsBool());
  EXPECT_THAT(json.GetBool(), IsOkAndHolds(false));
  EXPECT_EQ(json.ToString(), "false");
}

TEST(FastSerdeJsonBridge, CreateDouble) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson json,
                       FastSerdeJson::CreateDouble(1337.1337));

  EXPECT_TRUE(json.IsNumber());
  EXPECT_TRUE(json.IsDouble());
  EXPECT_THAT(json.GetDouble(), IsOkAndHolds(1337.1337));
  EXPECT_EQ(json.ToString(), "1337.1337");
}

TEST(FastSerdeJsonBridge, CreateDoubleWithNaN) {
  EXPECT_THAT(
      FastSerdeJson::CreateDouble(std::numeric_limits<double>::quiet_NaN()),
      StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, CreateDoubleWithInfinity) {
  EXPECT_THAT(
      FastSerdeJson::CreateDouble(std::numeric_limits<double>::infinity()),
      StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, CreateDoubleWithNegativeInfinity) {
  EXPECT_THAT(
      FastSerdeJson::CreateDouble(-std::numeric_limits<double>::infinity()),
      StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, CreateNull) {
  FastSerdeJson json = FastSerdeJson::CreateNull();

  EXPECT_TRUE(json.IsNull());
  EXPECT_EQ(json.ToString(), "null");
}

TEST(FastSerdeJsonBridge, CreateString) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::CreateString("text"));

  EXPECT_TRUE(json.IsString());
  EXPECT_THAT(json.GetString(), IsOkAndHolds("text"));
  EXPECT_EQ(json.ToString(), "\"text\"");
}

TEST(FastSerdeJsonBridge, CreateArray) {
  FastSerdeJson json = FastSerdeJson::CreateArray();

  EXPECT_TRUE(json.IsArray());
  EXPECT_EQ(json.ToString(), "[]");
}

TEST(FastSerdeJsonBridge, HasField) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_THAT(json.HasField("not_existing"), IsOkAndHolds(false));
}

TEST(FastSerdeJsonBridge, HasFieldNotObject) {
  constexpr absl::string_view kJsonString = "10";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json, FastSerdeJson::Parse(kJsonString));

  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(false));
}

TEST(FastSerdeJsonBridge, AddInt) {
  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldInt("value", 123));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_THAT(json.GetFieldInt("value"), IsOkAndHolds(123));
  EXPECT_EQ(json.ToString(), "{\"value\":123}");
}

TEST(FastSerdeJsonBridge, AddBool) {
  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldBool("value", true));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_THAT(json.GetFieldBool("value"), IsOkAndHolds(true));
  EXPECT_EQ(json.ToString(), "{\"value\":true}");
}

TEST(FastSerdeJsonBridge, AddDouble) {
  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldDouble("value", 1337.1337));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_THAT(json.GetFieldDouble("value"), IsOkAndHolds(1337.1337));
  EXPECT_EQ(json.ToString(), "{\"value\":1337.1337}");
}

TEST(FastSerdeJsonBridge, AddDoubleInvalid) {
  FastSerdeJson json = FastSerdeJson::CreateObject();
  EXPECT_THAT(
      json.AddFieldDouble("nan", std::numeric_limits<double>::quiet_NaN()),
      StatusIs(absl::StatusCode::kInvalidArgument));
  EXPECT_THAT(
      json.AddFieldDouble("inf", std::numeric_limits<double>::infinity()),
      StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, AddNull) {
  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldNull("value"));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_EQ(json.ToString(), "{\"value\":null}");
}

TEST(FastSerdeJsonBridge, AddString) {
  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldString("value", "text"));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  EXPECT_THAT(json.GetFieldString("value"), IsOkAndHolds("text"));
  EXPECT_EQ(json.ToString(), "{\"value\":\"text\"}");
}

TEST(FastSerdeJsonBridge, AddObject) {
  FastSerdeJson v1 = FastSerdeJson::CreateObject();
  EXPECT_OK(v1.AddFieldString("value", "text"));

  FastSerdeJson json = FastSerdeJson::CreateObject();

  EXPECT_OK(json.AddFieldObject("obj", std::move(v1)));
  EXPECT_THAT(json.HasField("obj"), IsOkAndHolds(true));
  EXPECT_EQ(json.ToString(), "{\"obj\":{\"value\":\"text\"}}");
}

TEST(FastSerdeJsonBridge, AddArray) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson v1_text,
                       FastSerdeJson::CreateString("text"));
  FastSerdeJson v2_int = FastSerdeJson::CreateInt(213);
  ASSERT_OK_AND_ASSIGN(FastSerdeJson v3_double,
                       FastSerdeJson::CreateDouble(1337.1337));
  FastSerdeJson json = FastSerdeJson::CreateObject();

  std::vector<FastSerdeJson> items;
  items.push_back(std::move(v1_text));
  items.push_back(std::move(v2_int));
  items.push_back(std::move(v3_double));

  ASSERT_OK(json.AddFieldArray("value", std::move(items)));
  EXPECT_THAT(json.HasField("value"), IsOkAndHolds(true));
  ASSERT_OK_AND_ASSIGN(std::vector<FastSerdeJson::NodeHandle> array,
                       json.GetFieldArray("value"));
  ASSERT_EQ(array.size(), 3);
  EXPECT_THAT(json.GetString(array[0]), IsOkAndHolds("text"));
  EXPECT_THAT(json.GetInt(array[1]), IsOkAndHolds(213));
  EXPECT_THAT(json.GetDouble(array[2]), IsOkAndHolds(1337.1337));

  EXPECT_EQ(json.ToString(), "{\"value\":[\"text\",213,1337.1337]}");
}

TEST(FastSerdeJsonBridge, EqualsSimple) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson first, FastSerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson second, FastSerdeJson::Parse(kJsonString));
  EXPECT_EQ(first, second);
}

TEST(FastSerdeJsonBridge, EqualsDifferentOrder) {
  constexpr absl::string_view kJsonString1 =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";
  constexpr absl::string_view kJsonString2 =
      "{\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"firstName\": \"John\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson first, FastSerdeJson::Parse(kJsonString1));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson second,
                       FastSerdeJson::Parse(kJsonString2));
  EXPECT_EQ(first, second);
}

TEST(FastSerdeJsonBridge, NotEqualsSimple) {
  constexpr absl::string_view kJsonString1 =
      "{\n"
      "  \"lastName\": \"Doe\"\n"
      "}";
  constexpr absl::string_view kJsonString2 =
      "{\n"
      "  \"firstName\": \"John\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson first, FastSerdeJson::Parse(kJsonString1));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson second,
                       FastSerdeJson::Parse(kJsonString2));
  EXPECT_NE(first, second);
}

TEST(FastSerdeJsonBridge, NotEqualsOneEmptyObj) {
  constexpr absl::string_view kJsonString1 = "{}";
  constexpr absl::string_view kJsonString2 =
      "{\n"
      "  \"firstName\": \"John\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(FastSerdeJson first, FastSerdeJson::Parse(kJsonString1));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson second,
                       FastSerdeJson::Parse(kJsonString2));
  EXPECT_NE(first, second);
}

// Note on handle semantics and safety across document instances:
// A NodeHandle is an opaque 64-bit index into a document's internal handle
// registry, representing a structural path from root rather than an absolute
// memory address or document-bound reference. By design, NodeHandle does not
// contain a document instance ID in order to keep handles lightweight (cheap to
// copy/move) and reusable across document clones. Consequently, using a handle
// ID on an unrelated document will fail with an out-of-bounds error if the ID
// exceeds that document's registry size, but if the unrelated document has
// created an identical handle ID index in its own registry, resolving the
// handle will evaluate against that document's path in its registry.
TEST(FastSerdeJsonBridge, NodeHandleSafetyAndValidity) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc1, FastSerdeJson::Parse("{\"a\": 1}"));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc2, FastSerdeJson::Parse("{\"b\": 2}"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle h1, doc1.GetField("a"));

  // Valid handle on doc1
  EXPECT_THAT(doc1.GetInt(h1), IsOkAndHolds(1));

  // Using h1 from doc1 on doc2 fails because doc2 has not registered any child
  // handles yet (its handle registry only contains the root handle at index 0).
  EXPECT_THAT(doc2.GetInt(h1), StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, MoveSemanticsAndCloneSubtree) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc,
                       FastSerdeJson::Parse("{\"key\": \"value\"}"));
  FastSerdeJson moved_doc = std::move(doc);

  EXPECT_THAT(moved_doc.GetFieldString("key"), IsOkAndHolds("value"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson cloned_doc, moved_doc.CloneSubtree());
  EXPECT_EQ(moved_doc.ToString(), cloned_doc.ToString());
  EXPECT_EQ(moved_doc, cloned_doc);
}

TEST(FastSerdeJsonBridge, SubtreeClone) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc,
                       FastSerdeJson::Parse("{\"sub\": {\"num\": 42}}"));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle sub_h, doc.GetField("sub"));

  ASSERT_OK_AND_ASSIGN(FastSerdeJson sub_doc, doc.CloneSubtree(sub_h));
  EXPECT_EQ(sub_doc.ToString(), "{\"num\":42}");
  EXPECT_THAT(sub_doc.GetFieldInt("num"), IsOkAndHolds(42));
}

TEST(FastSerdeJsonBridge, InvalidHandleCloneFails) {
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc1, FastSerdeJson::Parse("{\"a\": 1}"));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson doc2, FastSerdeJson::Parse("{\"b\": 2}"));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle h1, doc1.GetField("a"));

  EXPECT_THAT(doc2.CloneSubtree(h1),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(FastSerdeJsonBridge, HandleValidityAfterMove) {
  constexpr absl::string_view kJsonString =
      "{\"root_field\": \"hello\", \"child\": {\"num\": 123}}";
  ASSERT_OK_AND_ASSIGN(FastSerdeJson parsed_doc,
                       FastSerdeJson::Parse(kJsonString));
  std::optional<FastSerdeJson> doc = std::move(parsed_doc);

  FastSerdeJson::NodeHandle root_handle = {};
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle child_handle,
                       doc->GetField("child", root_handle));
  ASSERT_OK_AND_ASSIGN(FastSerdeJson::NodeHandle num_handle,
                       doc->GetField("num", child_handle));

  // Perform initial lookups to populate cached pointers in HandleRegistry
  EXPECT_THAT(doc->GetFieldString("root_field", root_handle),
              IsOkAndHolds("hello"));
  EXPECT_THAT(doc->GetInt(num_handle), IsOkAndHolds(123));

  // Move the document instance to a new location in memory and explicitly reset
  // the old optional object. This destroys the previous object memory location,
  // making memory bugs more apparent if cached pointers were not invalidated.
  FastSerdeJson moved_doc = std::move(*doc);
  doc.reset();

  // Verifying both root and non-root handles succeed after move, confirming
  // HandleRegistry detects the move and invalidates cached pointers properly.
  EXPECT_THAT(moved_doc.GetFieldString("root_field", root_handle),
              IsOkAndHolds("hello"));
  EXPECT_THAT(moved_doc.GetInt(num_handle), IsOkAndHolds(123));
}

TEST(FastSerdeJsonBridge, FloatPrecisionDifference) {
  double d1 = 0.082788195087703992;
  std::string s1 = "0.082788195087703992";
  double d2 = 0.082788195087704;
  std::string s2 = "0.082788195087704";

  // FastSerdeJson.
  ASSERT_OK_AND_ASSIGN(FastSerdeJson json1, FastSerdeJson::Parse(s1));
  ASSERT_OK_AND_ASSIGN(double val1, json1.GetDouble());

  ASSERT_OK_AND_ASSIGN(FastSerdeJson json2, FastSerdeJson::Parse(s2));
  ASSERT_OK_AND_ASSIGN(double val2, json2.GetDouble());

  // Nlohmann.
  nlohmann::json nlohmann_json1 = nlohmann::json::parse(s1);
  double nlohmann_val1 = nlohmann_json1.get<double>();

  nlohmann::json nlohmann_json2 = nlohmann::json::parse(s2);
  double nlohmann_val2 = nlohmann_json2.get<double>();

  // FastSerdeJson parses both strings to the same value.
  EXPECT_EQ(val1, val2);
  // Standard C++ literals distinguish them.
  EXPECT_NE(d1, d2);
  // FastSerdeJson parses both strings to the same value, which matches the less
  // precise literal in C++.
  EXPECT_EQ(val1, d2);
  EXPECT_NE(val1, d1);
  // Nlohmann distinguishes them, matching standard C++ literals.
  EXPECT_NE(nlohmann_val1, nlohmann_val2);
  EXPECT_EQ(nlohmann_val1, d1);
  EXPECT_EQ(nlohmann_val2, d2);
}

}  // namespace
