#include "proto_parser.h"

#include <string>

#include <google/protobuf/struct.pb.h>
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "absl/status/status.h"

namespace security::yaml {
namespace {

using ::google::protobuf::Value;
using ::testing::EqualsProto;
using ::testing::HasSubstr;
using ::testing::status::IsOkAndHolds;
using ::testing::status::StatusIs;

TEST(ProtoParserTest, ValidYamlToJsonSuccess) {
  std::string yaml_text = "name: jetski\nversion: 1.0\nenabled: true";
  ASSERT_OK_AND_ASSIGN(std::string json, ConvertYamlToJson(yaml_text));
  EXPECT_THAT(json, HasSubstr("\"name\":\"jetski\""));
  EXPECT_THAT(json, HasSubstr("\"version\":1"));
  EXPECT_THAT(json, HasSubstr("\"enabled\":true"));
}

TEST(ProtoParserTest, PreserveProtoFieldNames) {
  std::string yaml_text = "my_field_name: jetski\nanother_field: 123";
  ASSERT_OK_AND_ASSIGN(std::string json, ConvertYamlToJson(yaml_text));
  EXPECT_THAT(json, HasSubstr("\"my_field_name\":\"jetski\""));
  EXPECT_THAT(json, HasSubstr("\"another_field\":123"));
}

TEST(ProtoParserTest, InvalidYamlFailure) {
  std::string invalid_yaml = "name: [invalid : yaml : structure";
  EXPECT_THAT(ConvertYamlToJson(invalid_yaml),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(ProtoParserTest, EmptyOrUndefinedYamlFailure) {
  EXPECT_THAT(ConvertYamlToJson(""),
              StatusIs(absl::StatusCode::kInvalidArgument));
  EXPECT_THAT(ConvertYamlToJson("   \n  "),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(ProtoParserTest, InvalidUtf8Failure) {
  std::string invalid_utf8 = "name: \xFF\xFE";
  EXPECT_THAT(ConvertYamlToJson(invalid_utf8),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(ProtoParserTest, MultipleDocumentsFailure) {
  std::string multi_doc_yaml = "doc1: 1\n---\ndoc2: 2";
  EXPECT_THAT(ConvertYamlToJson(multi_doc_yaml),
              StatusIs(absl::StatusCode::kInvalidArgument,
                       HasSubstr("Multiple YAML documents are not supported")));
}

TEST(ProtoParserTest, ParseYamlProtoSuccess) {
  std::string yaml_text = "string_value: hello\nbool_value: true";
  EXPECT_THAT(ParseYaml<Value>(yaml_text), IsOkAndHolds(EqualsProto(R"pb(
                struct_value {
                  fields {
                    key: "string_value"
                    value { string_value: "hello" }
                  }
                  fields {
                    key: "bool_value"
                    value { bool_value: true }
                  }
                }
              )pb")));
}

TEST(ProtoParserTest, NullHandling) {
  std::string yaml_text =
      "null_val1: null\nnull_val2: ~\nquoted_null: \"null\"\nquoted_tilde: "
      "\"~\"";
  ASSERT_OK_AND_ASSIGN(std::string json, ConvertYamlToJson(yaml_text));
  EXPECT_THAT(json, HasSubstr("\"null_val1\":null"));
  EXPECT_THAT(json, HasSubstr("\"null_val2\":null"));
  EXPECT_THAT(json, HasSubstr("\"quoted_null\":\"null\""));
  EXPECT_THAT(json, HasSubstr("\"quoted_tilde\":\"~\""));

  EXPECT_THAT(ParseYaml<Value>(yaml_text), IsOkAndHolds(EqualsProto(R"pb(
                struct_value {
                  fields {
                    key: "null_val1"
                    value { null_value: NULL_VALUE }
                  }
                  fields {
                    key: "null_val2"
                    value { null_value: NULL_VALUE }
                  }
                  fields {
                    key: "quoted_null"
                    value { string_value: "null" }
                  }
                  fields {
                    key: "quoted_tilde"
                    value { string_value: "~" }
                  }
                }
              )pb")));
}

TEST(ProtoParserTest, IntBoundsHandling) {
  std::string yaml_text =
      "safe_int: 9007199254740991\nunsafe_int: 9007199254740992";
  EXPECT_THAT(ParseYaml<Value>(yaml_text), IsOkAndHolds(EqualsProto(R"pb(
                struct_value {
                  fields {
                    key: "safe_int"
                    value { number_value: 9007199254740991 }
                  }
                  fields {
                    key: "unsafe_int"
                    value { string_value: "9007199254740992" }
                  }
                }
              )pb")));
}

TEST(ProtoParserTest, FloatSpecialValuesHandling) {
  std::string yaml_text =
      "pos_inf: .inf\nneg_inf: -.inf\nnan_val: .nan\nnormal_val: 3.14";
  ASSERT_OK_AND_ASSIGN(std::string json, ConvertYamlToJson(yaml_text));
  EXPECT_THAT(json, HasSubstr("\"pos_inf\":\"Infinity\""));
  EXPECT_THAT(json, HasSubstr("\"neg_inf\":\"-Infinity\""));
  EXPECT_THAT(json, HasSubstr("\"nan_val\":\"NaN\""));
  EXPECT_THAT(json, HasSubstr("\"normal_val\":3.14"));

  EXPECT_THAT(ParseYaml<Value>(yaml_text), IsOkAndHolds(EqualsProto(R"pb(
                struct_value {
                  fields {
                    key: "pos_inf"
                    value { string_value: "Infinity" }
                  }
                  fields {
                    key: "neg_inf"
                    value { string_value: "-Infinity" }
                  }
                  fields {
                    key: "nan_val"
                    value { string_value: "NaN" }
                  }
                  fields {
                    key: "normal_val"
                    value { number_value: 3.14 }
                  }
                }
              )pb")));
}

TEST(ProtoParserTest, RecursionLimit) {
  std::string yaml_text = "a: {b: {c: {d: {e: 1}}}}";
  ParseOptions options;
  options.recursion_depth_limit = 2;  // Should fail at c
  EXPECT_THAT(ParseYaml<Value>(yaml_text, options),
              StatusIs(absl::StatusCode::kInvalidArgument,
                       HasSubstr("depth limit: 2")));

  options.recursion_depth_limit = 10;
  ASSERT_OK_AND_ASSIGN(Value proto, ParseYaml<Value>(yaml_text, options));
  EXPECT_TRUE(proto.has_struct_value());
}

}  // namespace
}  // namespace security::yaml
