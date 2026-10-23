#include "contrib/message_to_json.h"

#include "contrib/messages.proto.h"
#include "serde_json_bridge.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "third_party/protobuf/json/json.h"

namespace security::json::serde_json_bridge::contrib {
namespace {

TEST(MessageToJsonTest, SimpleMessageToJson) {
  SimpleMessage message;
  message.set_string_field("hello");
  message.set_int32_field(123);
  message.set_bool_field(true);
  message.set_enum_field(SIMPLE_ENUM_A);

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message));

  EXPECT_EQ(
      json.ToString(),
      R"json({"boolField":true,"enumField":"SIMPLE_ENUM_A","int32Field":123,"stringField":"hello"})json");
}

TEST(MessageToJsonTest, SimpleMessageToJsonAlwaysPrintEnumsAsInts) {
  SimpleMessage message;
  message.set_enum_field(SIMPLE_ENUM_A);

  proto2::json::PrintOptions options;
  options.always_print_enums_as_ints = true;

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message, options));

  EXPECT_EQ(json.ToString(), R"json({"enumField":1})json");
}

TEST(MessageToJsonTest, SimpleMessageRoundTrip) {
  SimpleMessage message;
  message.set_string_field("hello");
  message.set_int32_field(123);
  message.set_bool_field(true);
  message.set_enum_field(SIMPLE_ENUM_B);

  proto2::json::PrintOptions options;

  options.preserve_proto_field_names = true;

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message, options));
  ASSERT_OK_AND_ASSIGN(SimpleMessage parsed,
                       JsonToMessage<SimpleMessage>(json));
  EXPECT_THAT(message, testing::EqualsProto(parsed));
}

TEST(MessageToJsonTest, SimpleMessageRoundTrip_WithOptions) {
  proto2::json::PrintOptions print_options;
  print_options.preserve_proto_field_names = true;

  SimpleMessage message;
  message.set_string_field("hello");
  message.set_int32_field(123);
  message.set_bool_field(true);
  message.set_enum_field(SIMPLE_ENUM_C);

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message, print_options));

  ASSERT_OK_AND_ASSIGN(SimpleMessage parsed,
                       JsonToMessage<SimpleMessage>(json));
  EXPECT_THAT(message, testing::EqualsProto(parsed));
}

TEST(MessageToJsonTest, ComplexMessageRoundTrip) {
  ComplexMessage message;
  message.mutable_simple()->set_string_field("nested_hello");
  message.add_repeated_string("a");
  message.add_repeated_string("b");
  message.mutable_nested()->set_value("val");
  message.add_repeated_nested()->set_value("val1");
  message.add_repeated_nested()->set_value("val2");

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message));

  ASSERT_OK_AND_ASSIGN(ComplexMessage parsed,
                       JsonToMessage<ComplexMessage>(json));
  ASSERT_THAT(message, testing::EqualsProto(parsed));
}

TEST(MessageToJsonTest, ComplexMessageRoundTrip_WithOptions) {
  proto2::json::PrintOptions print_options;
  print_options.preserve_proto_field_names = true;

  ComplexMessage message;
  message.mutable_simple()->set_string_field("nested_hello");
  message.add_repeated_string("a");
  message.add_repeated_string("b");
  message.mutable_nested()->set_value("val");
  message.add_repeated_nested()->set_value("val1");
  message.add_repeated_nested()->set_value("val2");

  ASSERT_OK_AND_ASSIGN(SerdeJson json, MessageToJson(message, print_options));

  ASSERT_OK_AND_ASSIGN(ComplexMessage parsed,
                       JsonToMessage<ComplexMessage>(json));
  ASSERT_THAT(message, testing::EqualsProto(parsed));
}

}  // namespace
}  // namespace security::json::serde_json_bridge::contrib
