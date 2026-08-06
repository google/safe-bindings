#include <sys/resource.h>
#include <unistd.h>

#include <cstdlib>
#include <cstring>
#include <optional>
#include <string>
#include <vector>

#include "proto_parser.h"
#include "absl/algorithm/container.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "third_party/benchmark/include/benchmark/benchmark.h"
#include "third_party/tcmalloc/malloc_extension.h"

namespace security::yaml {
namespace {

auto P90 = [](const std::vector<double>& v) -> double {
  std::vector<double> copy = v;
  absl::c_sort(copy);
  return copy[static_cast<size_t>(copy.size() * 0.90)];
};

auto P95 = [](const std::vector<double>& v) -> double {
  std::vector<double> copy = v;
  absl::c_sort(copy);
  return copy[static_cast<size_t>(copy.size() * 0.95)];
};

auto P99 = [](const std::vector<double>& v) -> double {
  std::vector<double> copy = v;
  absl::c_sort(copy);
  return copy[static_cast<size_t>(copy.size() * 0.99)];
};

constexpr absl::string_view kSampleYaml =
    "name: test_model_graph\n"
    "version: 1.0\n"
    "features: [feature_a, feature_b, feature_c, feature_d]\n"
    "sampling_plans:\n"
    "  - name: plan_1\n"
    "    strategy: uniform\n"
    "    sample_size: 100\n"
    "  - name: plan_2\n"
    "    strategy: weighted\n"
    "    sample_size: 50\n"
    "metadata:\n"
    "  author: spanner_ml_team\n"
    "  production: true\n";

size_t GetHostAllocatedBytes() {
  std::optional<size_t> allocated =
      tcmalloc::MallocExtension::GetNumericProperty(
          "generic.current_allocated_bytes");
  return allocated.value_or(0);
}

size_t GetHostPeakRssBytes() {
  struct rusage usage{};
  if (getrusage(RUSAGE_SELF, &usage) == 0) {
    return static_cast<size_t>(usage.ru_maxrss) * 1024;
  }
  return 0;
}

void RecordMemoryCounters(benchmark::State& state) {
  state.counters["host_alloc_bytes"] =
      benchmark::Counter(GetHostAllocatedBytes(), benchmark::Counter::kDefaults,
                         benchmark::Counter::kIs1024);
  state.counters["host_peak_rss_bytes"] =
      benchmark::Counter(GetHostPeakRssBytes(), benchmark::Counter::kDefaults,
                         benchmark::Counter::kIs1024);
}

std::string GenerateYamlPayload(size_t extra_entries) {
  std::string result(kSampleYaml);
  for (size_t i = 0; i < extra_entries; ++i) {
    absl::StrAppend(&result, "  - name: plan_extra_", i, "\n",
                    "    strategy: uniform\n", "    sample_size: ", i * 10,
                    "\n");
  }
  return result;
}

void BM_ConvertYamlToJson(benchmark::State& state) {
  for (auto _ : state) {
    absl::StatusOr<std::string> res = ConvertYamlToJson(kSampleYaml);
    benchmark::DoNotOptimize(res);
  }
  RecordMemoryCounters(state);
}
BENCHMARK(BM_ConvertYamlToJson)
    ->Repetitions(10)
    ->ReportAggregatesOnly(true)
    ->ComputeStatistics("p90", P90)
    ->ComputeStatistics("p95", P95)
    ->ComputeStatistics("p99", P99);

void BM_MemoryUsage(benchmark::State& state) {
  std::string yaml_text = GenerateYamlPayload(state.range(0));
  state.SetBytesProcessed(state.iterations() * yaml_text.size());
  for (auto _ : state) {
    absl::StatusOr<std::string> res = ConvertYamlToJson(yaml_text);
    benchmark::DoNotOptimize(res);
  }
  RecordMemoryCounters(state);
}
BENCHMARK(BM_MemoryUsage)
    ->Arg(0)    // Small payload (~200B)
    ->Arg(500)  // Large payload (~35KB)
    ->Repetitions(3)
    ->ReportAggregatesOnly(true)
    ->ComputeStatistics("p90", P90)
    ->ComputeStatistics("p95", P95)
    ->ComputeStatistics("p99", P99);

}  // namespace
}  // namespace security::yaml
