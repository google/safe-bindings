#include <string>

#include <google/protobuf/struct.pb.h>
#include "proto_parser.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "testing/fuzzing/fuzztest.h"
#include "absl/status/statusor.h"

namespace security::yaml {
namespace {

using ::google::protobuf::Value;
using ::testing::EqualsProto;

void ParseArbitraryYamlDoesNotCrash(const std::string& yaml_input) {
  absl::StatusOr<Value> result1 = ParseYaml<Value>(yaml_input);
  absl::StatusOr<Value> result2 = ParseYaml<Value>(yaml_input);

  EXPECT_EQ(result1.status(), result2.status());
  if (result1.ok() && result2.ok()) {
    EXPECT_THAT(*result1, EqualsProto(*result2));
  }
}

FUZZ_TEST(ProtoParserFuzzTest, ParseArbitraryYamlDoesNotCrash)
    .WithDomains(fuzztest::Arbitrary<std::string>());

}  // namespace
}  // namespace security::yaml
