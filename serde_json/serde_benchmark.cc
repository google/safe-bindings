#include <functional>
#include <initializer_list>
#include <string>
#include <utility>
#include <vector>

#include "devtools/build/runtime/get_runfiles_dir.h"
#include "file/base/helpers.h"
#include "file/base/options.h"
#include "file/base/path.h"
#include "serde_json_bridge.h"
#include "testing/base/public/benchmark.h"
#include "absl/log/check.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "third_party/gloop/util/gtl/value_or_die.h"
#include "third_party/json/src/json.hpp"

namespace {

using ::security::json::serde_json_bridge::SerdeJson;

std::string LoadJson(absl::string_view file_base_name) {
  std::string dst_str;
  CHECK_OK(file::GetContents(
      devtools_build::GetDataDependencyFilepath(file::JoinPath(
          "google3/security/json/serde_json/test_data", file_base_name)),
      &dst_str, file::Defaults()));
  return dst_str;
}

void BM_Nlohmann_Parse(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  for (auto s : state) {
    benchmark::DoNotOptimize(nlohmann::json::parse(json_document));
  }
}

void BM_SerdeJson_Parse(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  for (auto s : state) {
    benchmark::DoNotOptimize(SerdeJson::Parse(json_document));
  }
}

void BM_Nlohmann_Dump(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  nlohmann::json parsed = nlohmann::json::parse(json_document);
  for (auto s : state) {
    benchmark::DoNotOptimize(parsed.dump());
  }
}

void BM_SerdeJson_Dump(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  SerdeJson parsed = gtl::ValueOrDie(SerdeJson::Parse(json_document));
  for (auto s : state) {
    benchmark::DoNotOptimize(parsed.ToString(/*sort_keys=*/false));
  }
}

// Recursively traverses the Nlohmann JSON structure.
void WalkNlohmann(const nlohmann::json& j) {
  if (j.is_object()) {
    for (auto& [key, val] : j.items()) {
      benchmark::DoNotOptimize(key);
      WalkNlohmann(val);
    }
  } else if (j.is_array()) {
    for (const nlohmann::json& val : j) {
      WalkNlohmann(val);
    }
  }
}

// Recursively traverses the SerdeJson structure.
void WalkSerde(const SerdeJson& s) {
  if (s.IsObject()) {
    std::vector<std::string> keys = gtl::ValueOrDie(s.GetKeys());
    for (const std::string& key : keys) {
      benchmark::DoNotOptimize(key);
      WalkSerde(gtl::ValueOrDie(s.GetField(key)));
    }
  } else if (s.IsArray()) {
    std::vector<SerdeJson> arr = gtl::ValueOrDie(s.GetArray());
    for (const SerdeJson& val : arr) {
      WalkSerde(val);
    }
  }
}

void BM_Nlohmann_Walk(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  nlohmann::json parsed = nlohmann::json::parse(json_document);
  for (auto s : state) {
    WalkNlohmann(parsed);
  }
}

void BM_SerdeJson_Walk(benchmark::State& state, absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  SerdeJson parsed = gtl::ValueOrDie(SerdeJson::Parse(json_document));
  for (auto s : state) {
    WalkSerde(parsed);
  }
}

void BM_Nlohmann_AccessPath(benchmark::State& state,
                            absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  nlohmann::json parsed = nlohmann::json::parse(json_document);
  for (auto s : state) {
    std::string name = parsed["dataFeedElement"][0]["name"].get<std::string>();
    benchmark::DoNotOptimize(name);
  }
}

void BM_SerdeJson_AccessPath(benchmark::State& state,
                             absl::string_view json_file) {
  std::string json_document = LoadJson(json_file);
  SerdeJson parsed = gtl::ValueOrDie(SerdeJson::Parse(json_document));
  for (auto s : state) {
    std::vector<SerdeJson> elements =
        gtl::ValueOrDie(parsed.GetFieldArray("dataFeedElement"));
    std::string name = gtl::ValueOrDie(elements[0].GetFieldString("name"));
    benchmark::DoNotOptimize(name);
  }
}

[[maybe_unused]] const bool kBenchmarkRegistration = [] {
  for (const auto& [benchmark_input_name, json_file] :
       std::initializer_list<std::pair<absl::string_view, absl::string_view>>{
           {"LargeDoc", "large.json"},
           {"SampleDoc", "sample.json"},
           {"TinyFloats", "tiny_floats.json"}}) {
    for (const auto& [benchmark_name, benchmark_function] :
         std::initializer_list<std::pair<
             absl::string_view,
             std::function<void(benchmark::State&, absl::string_view)>>>{
             {"BM_Nlohmann_Parse", BM_Nlohmann_Parse},
             {"BM_SerdeJson_Parse", BM_SerdeJson_Parse},
             {"BM_Nlohmann_Dump", BM_Nlohmann_Dump},
             {"BM_SerdeJson_Dump", BM_SerdeJson_Dump},
             {"BM_Nlohmann_Walk", BM_Nlohmann_Walk},
             {"BM_SerdeJson_Walk", BM_SerdeJson_Walk},
         }) {
      benchmark::RegisterBenchmark(
          absl::StrCat(benchmark_name, "_", benchmark_input_name),
          benchmark_function, json_file);
    }
  }
  benchmark::RegisterBenchmark("BM_Nlohmann_AccessPath_LargeDoc",
                               BM_Nlohmann_AccessPath, "large.json");
  benchmark::RegisterBenchmark("BM_SerdeJson_AccessPath_LargeDoc",
                               BM_SerdeJson_AccessPath, "large.json");
  return false;
}();

}  // namespace
