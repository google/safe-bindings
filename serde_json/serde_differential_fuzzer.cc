#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "fast_serde_json_bridge.h"
#include "serde_json_bridge.h"
#include "testing/base/public/gmock.h"
#include "testing/base/public/gunit.h"
#include "testing/fuzzing/fuzztest.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::json {
namespace {

using ::security::json::fast_serde_json_bridge::FastSerdeJson;
using ::security::json::serde_json_bridge::SerdeJson;

void WalkAndCompare(const SerdeJson& s_doc, FastSerdeJson& f_doc,
                    FastSerdeJson::NodeHandle handle = {}) {
  EXPECT_EQ(s_doc.IsNull(), f_doc.IsNull(handle));
  EXPECT_EQ(s_doc.IsObject(), f_doc.IsObject(handle));
  EXPECT_EQ(s_doc.IsArray(), f_doc.IsArray(handle));
  EXPECT_EQ(s_doc.IsString(), f_doc.IsString(handle));
  EXPECT_EQ(s_doc.IsNumber(), f_doc.IsNumber(handle));
  EXPECT_EQ(s_doc.IsInt(), f_doc.IsInt(handle));
  EXPECT_EQ(s_doc.IsDouble(), f_doc.IsDouble(handle));
  EXPECT_EQ(s_doc.IsBool(), f_doc.IsBool(handle));

  ASSERT_OK_AND_ASSIGN(bool f_empty, f_doc.IsEmpty(handle));
  EXPECT_EQ(s_doc.IsEmpty(), f_empty);

  if (s_doc.IsInt()) {
    absl::StatusOr<int64_t> s_val = s_doc.GetInt();
    absl::StatusOr<int64_t> f_val = f_doc.GetInt(handle);
    EXPECT_EQ(s_val.ok(), f_val.ok());
    if (s_val.ok() && f_val.ok()) {
      EXPECT_EQ(*s_val, *f_val);
    }
  }
  if (s_doc.IsDouble()) {
    absl::StatusOr<double> s_val = s_doc.GetDouble();
    absl::StatusOr<double> f_val = f_doc.GetDouble(handle);
    EXPECT_EQ(s_val.ok(), f_val.ok());
    if (s_val.ok() && f_val.ok()) {
      EXPECT_DOUBLE_EQ(*s_val, *f_val);
    }
  }
  if (s_doc.IsBool()) {
    absl::StatusOr<bool> s_val = s_doc.GetBool();
    absl::StatusOr<bool> f_val = f_doc.GetBool(handle);
    EXPECT_EQ(s_val.ok(), f_val.ok());
    if (s_val.ok() && f_val.ok()) {
      EXPECT_EQ(*s_val, *f_val);
    }
  }
  if (s_doc.IsString()) {
    absl::StatusOr<std::string> s_val = s_doc.GetString();
    absl::StatusOr<std::string> f_val = f_doc.GetString(handle);
    EXPECT_EQ(s_val.ok(), f_val.ok());
    if (s_val.ok() && f_val.ok()) {
      EXPECT_EQ(*s_val, *f_val);
    }
  }

  EXPECT_EQ(s_doc.ToString(/*sort_keys=*/true),
            f_doc.ToString(/*sort_keys=*/true, handle));
  EXPECT_EQ(s_doc.ToString(/*sort_keys=*/false),
            f_doc.ToString(/*sort_keys=*/false, handle));

  if (s_doc.IsObject()) {
    absl::StatusOr<std::vector<std::string>> s_keys = s_doc.GetKeys();
    absl::StatusOr<std::vector<std::string>> f_keys = f_doc.GetKeys(handle);
    ASSERT_EQ(s_keys.ok(), f_keys.ok());
    if (!s_keys.ok() || !f_keys.ok()) return;

    EXPECT_EQ(*s_keys, *f_keys);

    for (const std::string& key : *s_keys) {
      absl::StatusOr<SerdeJson> s_child = s_doc.GetField(key);
      absl::StatusOr<FastSerdeJson::NodeHandle> f_child_handle =
          f_doc.GetField(key, handle);
      ASSERT_EQ(s_child.ok(), f_child_handle.ok());
      if (s_child.ok() && f_child_handle.ok()) {
        WalkAndCompare(*s_child, f_doc, *f_child_handle);
      }

      absl::StatusOr<bool> s_has = s_doc.HasField(key);
      absl::StatusOr<bool> f_has = f_doc.HasField(key, handle);
      EXPECT_EQ(s_has.ok(), f_has.ok());
      if (s_has.ok() && f_has.ok()) {
        EXPECT_EQ(*s_has, *f_has);
      }
    }
  } else if (s_doc.IsArray()) {
    absl::StatusOr<std::vector<SerdeJson>> s_arr = s_doc.GetArray();
    absl::StatusOr<std::vector<FastSerdeJson::NodeHandle>> f_arr =
        f_doc.GetArray(handle);
    ASSERT_EQ(s_arr.ok(), f_arr.ok());
    if (!s_arr.ok() || !f_arr.ok()) return;
    ASSERT_EQ(s_arr->size(), f_arr->size());

    for (size_t i = 0; i < s_arr->size(); ++i) {
      WalkAndCompare((*s_arr)[i], f_doc, (*f_arr)[i]);

      absl::StatusOr<SerdeJson> s_elem = s_doc.GetArrayElement(i);
      absl::StatusOr<FastSerdeJson::NodeHandle> f_elem =
          f_doc.GetArrayElement(i, handle);
      EXPECT_EQ(s_elem.ok(), f_elem.ok());
      if (s_elem.ok() && f_elem.ok()) {
        WalkAndCompare(*s_elem, f_doc, *f_elem);
      }
    }
  }
}

void CompareSerdeJsonAndFastSerdeJson(absl::string_view json_str) {
  absl::StatusOr<SerdeJson> s_doc = SerdeJson::Parse(json_str);
  absl::StatusOr<FastSerdeJson> f_doc = FastSerdeJson::Parse(json_str);

  ASSERT_EQ(s_doc.ok(), f_doc.ok());
  if (!s_doc.ok()) {
    return;
  }

  WalkAndCompare(*s_doc, *f_doc);
}
FUZZ_TEST(SerdeDifferentialFuzzer, CompareSerdeJsonAndFastSerdeJson);

}  // namespace
}  // namespace security::json
