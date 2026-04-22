#include "security/json/serde_json/serde_json_bridge.h"

#include <limits>
#include <string>
#include <vector>

#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "third_party/absl/status/status.h"
#include "third_party/absl/status/status_matchers.h"
#include "third_party/absl/strings/string_view.h"

namespace {

using ::testing::status::IsOkAndHolds;

TEST(SerdeJsonBridge, SimpleParse) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK(security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
}

TEST(SerdeJsonBridge, FailParse) {
  constexpr absl::string_view kInvalidJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "}";

  ASSERT_THAT(
      security::json::serde_json_bridge::SerdeJson::Parse(kInvalidJsonString),
      absl_testing::StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(SerdeJsonBridge, CheckFieldGetter) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK(json.GetField("firstName"));
  ASSERT_OK(json.GetField("lastName"));
  ASSERT_THAT(json.GetField("phone"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(SerdeJsonBridge, CheckGetBool) {
  constexpr absl::string_view kJsonString = "true";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetBool(), IsOkAndHolds(true));
}

TEST(SerdeJsonBridge, CheckGetString) {
  constexpr absl::string_view kJsonString = "\"string\"";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetString(), IsOkAndHolds("string"));
}

TEST(SerdeJsonBridge, CheckGetInt) {
  constexpr absl::string_view kJsonString = "1234";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetInt(), IsOkAndHolds(1234));
}

TEST(SerdeJsonBridge, CheckGetDouble) {
  constexpr absl::string_view kJsonString = "1337.1234";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetDouble(), IsOkAndHolds(1337.1234));
}

TEST(SerdeJsonBridge, CheckGetArray) {
  constexpr absl::string_view kJsonString =
      "[\n"
      "  \"first\",\n"
      "  2,\n"
      "  \"third\",\n"
      "  4\n"
      "]";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(
      std::vector<security::json::serde_json_bridge::SerdeJson> array,
      json.GetArray());

  ASSERT_THAT(array[0].GetString(), IsOkAndHolds("first"));
  ASSERT_THAT(array[1].GetInt(), IsOkAndHolds(2));
  ASSERT_THAT(array[2].GetString(), IsOkAndHolds("third"));
  ASSERT_THAT(array[3].GetInt(), IsOkAndHolds(4));
}

TEST(SerdeJsonBridge, CheckGetArrayNotFromArray) {
  constexpr absl::string_view kJsonString = "\"first\"";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetArray(),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(SerdeJsonBridge, GetFieldString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldString("firstName"), IsOkAndHolds("John"));
  ASSERT_THAT(json.GetFieldString("lastName"), IsOkAndHolds("Doe"));
}

TEST(SerdeJsonBridge, GetFieldBool) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": true,\n"
      "  \"value2\": false\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldBool("value1"), IsOkAndHolds(true));
  ASSERT_THAT(json.GetFieldBool("value2"), IsOkAndHolds(false));
}

TEST(SerdeJsonBridge, GetFieldInt) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": 123,\n"
      "  \"value2\": -444\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldInt("value1"), IsOkAndHolds(123));
  ASSERT_THAT(json.GetFieldInt("value2"), IsOkAndHolds(-444));
}

TEST(SerdeJsonBridge, GetFieldDouble) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldDouble("value1"), IsOkAndHolds(1.0));
  ASSERT_THAT(json.GetFieldDouble("value2"), IsOkAndHolds(3.0));
}

TEST(SerdeJsonBridge, GetFieldObject) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"obj\": {"
      "    \"value1\": 1.0\n"
      "  }\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson obj,
                       json.GetFieldObject("obj"));
  ASSERT_THAT(obj.GetFieldDouble("value1"), IsOkAndHolds(1.0));
}

TEST(SerdeJsonBridge, GetFieldArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": [\n"
      "    \"first\",\n"
      "    2,\n"
      "    \"third\",\n"
      "    4\n"
      " ]\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(
      std::vector<security::json::serde_json_bridge::SerdeJson> array,
      json.GetFieldArray("value"));

  ASSERT_THAT(array[0].GetString(), IsOkAndHolds("first"));
  ASSERT_THAT(array[1].GetInt(), IsOkAndHolds(2));
  ASSERT_THAT(array[2].GetString(), IsOkAndHolds("third"));
  ASSERT_THAT(array[3].GetInt(), IsOkAndHolds(4));
}

TEST(SerdeJsonBridge, GetFieldArrayNotFromArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 1.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldArray("value"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
}

TEST(SerdeJsonBridge, GetFieldNotFromObject) {
  constexpr absl::string_view kJsonString = "1337.1234";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_THAT(json.GetFieldString("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetFieldBool("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetFieldDouble("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetFieldInt("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetFieldObject("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetFieldArray("test"),
              absl_testing::StatusIs(absl::StatusCode::kFailedPrecondition));
  ASSERT_THAT(json.GetDouble(), IsOkAndHolds(1337.1234));
}

TEST(SerdeJsonBridge, IsNull) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": null\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsNumber());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsObject) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": {}\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsObject());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsNumber());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsArray) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": []\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsArray());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsNumber());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": \"\"\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsString());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsNumber());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsNumber) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsNumber());
  EXPECT_TRUE(value.IsInt());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
}

TEST(SerdeJsonBridge, IsBool) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": true\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsBool());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsNumber());
  EXPECT_FALSE(value.IsDouble());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsDouble) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsNumber());
  EXPECT_TRUE(value.IsDouble());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsInt());
}

TEST(SerdeJsonBridge, IsInt) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  ASSERT_OK_AND_ASSIGN(security::json::serde_json_bridge::SerdeJson value,
                       json.GetField("value"));

  EXPECT_TRUE(value.IsNumber());
  EXPECT_TRUE(value.IsInt());
  EXPECT_FALSE(value.IsNull());
  EXPECT_FALSE(value.IsObject());
  EXPECT_FALSE(value.IsArray());
  EXPECT_FALSE(value.IsString());
  EXPECT_FALSE(value.IsBool());
  EXPECT_FALSE(value.IsDouble());
}

TEST(SerdeJsonBridge, ToString) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"firstName\": \"John\",\n"
      "  \"lastName\": \"Doe\",\n"
      "  \"value1\": 1.0,\n"
      "  \"value2\": 3.0\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));
  EXPECT_EQ(json.ToString(),
            "{\"firstName\":\"John\",\"lastName\":\"Doe\",\"value1\":1.0,"
            "\"value2\":3.0}");
}

TEST(SerdeJsonBridge, CreateInt) {
  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::CreateInt(123));

  ASSERT_EQ(json.IsNumber(), true);
  ASSERT_EQ(json.IsInt(), true);
  ASSERT_THAT(json.GetInt(), IsOkAndHolds(123));
  EXPECT_EQ(json.ToString(), "123");
}

TEST(SerdeJsonBridge, CreateBool) {
  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::CreateBool(false));

  ASSERT_EQ(json.IsBool(), true);
  ASSERT_THAT(json.GetBool(), IsOkAndHolds(false));
  EXPECT_EQ(json.ToString(), "false");
}

TEST(SerdeJsonBridge, CreateDouble) {
  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::CreateDouble(1337.1337));

  ASSERT_EQ(json.IsNumber(), true);
  ASSERT_EQ(json.IsDouble(), true);
  ASSERT_THAT(json.GetDouble(), IsOkAndHolds(1337.1337));
  EXPECT_EQ(json.ToString(), "1337.1337");
}

TEST(SerdeJsonBridge, CreateDoubleWithNaN) {
  ASSERT_THAT(security::json::serde_json_bridge::SerdeJson::CreateDouble(
                  std::numeric_limits<double>::quiet_NaN()),
              absl_testing::StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(SerdeJsonBridge, CreateDoubleWithInfinity) {
  ASSERT_THAT(security::json::serde_json_bridge::SerdeJson::CreateDouble(
                  std::numeric_limits<double>::infinity()),
              absl_testing::StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(SerdeJsonBridge, CreateDoubleWithNegativeInfinity) {
  ASSERT_THAT(security::json::serde_json_bridge::SerdeJson::CreateDouble(
                  -std::numeric_limits<double>::infinity()),
              absl_testing::StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(SerdeJsonBridge, CreateNull) {
  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::CreateNull());

  ASSERT_EQ(json.IsNull(), true);
  EXPECT_EQ(json.ToString(), "null");
}

TEST(SerdeJsonBridge, CreateString) {
  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::CreateString("text"));

  ASSERT_EQ(json.IsString(), true);
  ASSERT_THAT(json.GetString(), IsOkAndHolds("text"));
  EXPECT_EQ(json.ToString(), "\"text\"");
}

TEST(SerdeJsonBridge, HasField) {
  constexpr absl::string_view kJsonString =
      "{\n"
      "  \"value\": 10\n"
      "}";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));

  ASSERT_THAT(json.HasField("value"), IsOkAndHolds(true));
  ASSERT_THAT(json.HasField("not_existing"), IsOkAndHolds(false));
}

TEST(SerdeJsonBridge, HasFieldNotObject) {
  constexpr absl::string_view kJsonString = "10";

  ASSERT_OK_AND_ASSIGN(
      security::json::serde_json_bridge::SerdeJson json,
      security::json::serde_json_bridge::SerdeJson::Parse(kJsonString));

  ASSERT_THAT(json.HasField("value"), IsOkAndHolds(false));
}

}  // namespace
