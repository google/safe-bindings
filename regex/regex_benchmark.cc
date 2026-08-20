#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "benchmark/benchmark.h"
#include "regex.h"
#include "absl/log/check.h"
#include "absl/random/random.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"
#include "re2/re2.h"
#include "re2/set.h"

namespace security::regex {
namespace {

// The literal prefix allows engines to quickly skip through the haystack.
inline constexpr char kLiteral[] = "google_vendor_specific_attribute_value$";

// Simulates a common URL or file path.
// Contains optional components and simple alternations, but still contains
// an easy to scan prefix.
inline constexpr char kPathBranch[] =
    "/usr/(local/)?(bin|lib|share|include)/[a-zA-Z0-9_-]+$";

// Alternation of literals with different first characters.
// Prevents single-character prefix optimizations (like memchr on 'G').
inline constexpr char kAlternation[] = "(GET|POST|PUT|DELETE) /index.html$";

// Broad repetition followed by a specific suffix.
// The leading `[^\n]*` forces a scan to the end of the line, and can cause
// trouble to backtracking implementations.
inline constexpr char kLeadingRepetition[] = "[^\\n]*_debug_flag_enabled$";

// Optional matches of a large class, ending with a mismatch.
// The large class `[ -~\p{L}]` matches all ASCII and Unicode letters. The final
// `\u03B1` is missing from our ASCII haystack, forcing a full search before
// failing.
inline constexpr char kFanout[] = "(?:[ -~\\p{L}]?){50}\u03B1";

// Exploits frequent partial matches to bypass optimizations.
// In `a[a-z]b` with haystack "axxb...", both 'a' and 'b' are frequent, but
// never separated by just one char. Forces the engine to scan fully and
// constantly trigger and fail candidate matches.
inline constexpr char kNearMisses[] = "a[a-z]b";

// A pattern with multiple capturing groups, to check capture performance.
inline constexpr char kCapturePattern[] = R"((\d{3})-(\d{3})-(\d{4}))";

// A pattern with many capturing groups, similar to RE2's PARENS.
// This stresses engines that are trying to track parentheses.
inline constexpr char kParensPattern[] =
    "([ -~])*(A)(B)(C)(D)(E)(F)(G)(H)(I)(J)(K)(L)(M)"
    "(N)(O)(P)(Q)(R)(S)(T)(U)(V)(W)(X)(Y)(Z)$";

// --- Additional patterns for compilation benchmarks ---

// Standard email address validation pattern (RFC 5322 simplified).
inline constexpr char kEmail[] =
    R"(^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$)";

// Generic URI parsing regex with scheme, host, port, and path/query.
inline constexpr char kUri[] =
    R"(^https?://([a-zA-Z0-9.-]+)(:[0-9]+)?(/[-a-zA-Z0-9_:@&?=+,.!/~%$]*)?$)";

// IPv4 dotted-decimal address with octet ranges 0-255.
inline constexpr char kIpv4[] =
    R"(^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$)";

// IPv6 8-hextet full address.
inline constexpr char kIpv6[] = R"(^([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$)";

// ISO 8601 extended date/time with optional fractional seconds and timezone.
inline constexpr char kIso8601Date[] =
    R"(^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$)";

// Canonical UUID (8-4-4-4-12 hex digits).
inline constexpr char kUuid[] =
    R"(^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$)";

// Apache / Common Log Format line parser.
inline constexpr char kHttpLog[] =
    R"raw(^(\S+) (\S+) (\S+) \[([\w:/]+\s[+\-]\d{4})\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-))raw";

// Semantic Versioning 2.0.0 official specification regex.
inline constexpr char kSemVer[] =
    R"(^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$)";

// Character classes utilizing multiple Unicode property categories.
inline constexpr char kUnicodeClasses[] = R"([\p{L}\p{N}\p{P}]+)";

// Complex character class with negated range and punctuation characters.
inline constexpr char kComplexCharClass[] =
    R"([^a-zA-Z0-9_\-.:/?#\[\]@!$&'()*+,;=]+)";

// Generates random printable ASCII text (0x20 to 0x7F) of size `nbytes`.
// This matches the assumptions of our test patterns (e.g. FANOUT expects
// no non-ASCII characters).
std::string RandomText(int64_t nbytes) {
  static const std::string* const text = []() {
    std::string* text = new std::string;
    absl::BitGen bitgen;
    // Pre-generate 1MB of random text.
    text->resize(1 << 20);
    for (int64_t i = 0; i < 1 << 20; i++) {
      // Generate printable ASCII characters in range [0x20, 0x7F] inclusive.
      // absl::Uniform is [low, high), so we use 0x20 to 0x80.
      char byte = absl::Uniform(bitgen, 0x20, 0x80);
      (*text)[i] = byte;
    }
    return text;
  }();
  CHECK_LE(nbytes, 1 << 20);
  return text->substr(0, nbytes);
}

// Generates a haystack with many "near misses" for scanning: "axxb" repeated.
// Rationale: Both 'a' and 'b' are very frequent in the haystack, but the
// pattern "a[a-z]b" never matches because there are always two 'x's between
// them. This bypasses simple literal optimizations and forces a full scan with
// frequent candidate triggers.
std::string NearMissesText(int64_t nbytes) {
  std::string s;
  s.reserve(nbytes);
  while (s.size() < nbytes - 4) {
    s += "axxb";
  }
  while (s.size() < nbytes) {
    s += 'x';
  }
  return s;
}

// Generates a haystack that guaranteed to match kCapturePattern at the end.
std::string CaptureText(int64_t nbytes) {
  if (nbytes < 12) {
    return "123-456-7890";
  }
  std::string s = RandomText(nbytes - 12);
  s += "123-456-7890";
  return s;
}

// Generates repeating dense pattern text: "item_0000:value_0000
// item_0001:value_0001 ..."
std::string DenseText(int64_t nbytes) {
  std::string s;
  s.reserve(nbytes);
  int i = 0;
  while (s.size() < static_cast<size_t>(nbytes)) {
    absl::StrAppendFormat(&s, "item_%04d:value_%04d ", i % 10000, i % 10000);
    ++i;
  }
  s.resize(nbytes);
  return s;
}

// Generates repeating capture text: "123-456-7890 987-654-3210 ..."
std::string CapturePairsText(int64_t nbytes) {
  std::string s;
  s.reserve(nbytes);
  int i = 0;
  while (s.size() < static_cast<size_t>(nbytes)) {
    absl::StrAppendFormat(&s, "%03d-%03d-%04d ", (100 + i) % 1000,
                          (200 + i) % 1000, (1000 + i) % 10000);
    ++i;
  }
  s.resize(nbytes);
  return s;
}

// Generates an alternation pattern with `n` branches: "item_0000|item_0001|..."
std::string GenerateAlternationPattern(int n) {
  std::string result;
  result.reserve(n * 10);
  for (int i = 0; i < n; ++i) {
    if (i > 0) result.push_back('|');
    absl::StrAppendFormat(&result, "item_%04d", i);
  }
  return result;
}

// Generates a literal string pattern of length `n`.
std::string GenerateLiteralPattern(int n) {
  std::string s;
  s.reserve(n);
  for (int i = 0; i < n; ++i) {
    s.push_back(static_cast<char>('a' + (i % 26)));
  }
  return s;
}

// Generates a bounded repetition pattern: "(?:ab){n}"
std::string GenerateRepetitionPattern(int n) {
  return absl::StrFormat("(?:ab){%d}", n);
}

// Generates a character class with `n` disjoint Unicode ranges.
std::string GenerateCharClassPattern(int n) {
  std::string result = "[";
  for (int i = 0; i < n; ++i) {
    absl::StrAppendFormat(&result, "\\x{%x}-\\x{%x}", 0x100 + i * 4,
                          0x100 + i * 4 + 2);
  }
  result += ']';
  return result;
}

// Generates a pattern with `n` capture groups: "(a)(b)(c)..."
std::string GenerateCaptureGroupsPattern(int n) {
  std::string result;
  result.reserve(n * 4);
  for (int i = 0; i < n; ++i) {
    absl::StrAppendFormat(&result, "(%c)", 'a' + (i % 26));
  }
  return result;
}

// Generates a deeply nested group pattern with depth `n`: "((...((a))...))"
std::string GenerateNestedPattern(int n) {
  std::string result;
  result.reserve(n * 2 + 1);
  result.append(n, '(');
  result.push_back('a');
  result.append(n, ')');
  return result;
}

// Generates `n` distinct regex patterns for RegexSet / RE2::Set benchmarks.
std::vector<std::string> GenerateSetPatterns(int n) {
  std::vector<std::string> patterns;
  patterns.reserve(n);
  for (int i = 0; i < n; ++i) {
    patterns.push_back(absl::StrFormat("pattern_[0-9]+_%04d_test$", i));
  }
  return patterns;
}

// ============================================================================
// Search Benchmark Helpers
// ============================================================================

void BM_RE2_Search(benchmark::State& state, const char* pattern) {
  std::string text = RandomText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);  // 256MB to avoid DFA out of memory
  RE2 re(pattern, options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    bool matched = RE2::PartialMatch(text, re);
    benchmark::DoNotOptimize(matched);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Search(benchmark::State& state, const char* pattern) {
  std::string text = RandomText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(pattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    bool matched = re->IsMatch(text);
    benchmark::DoNotOptimize(matched);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_NearMisses(benchmark::State& state) {
  std::string text = NearMissesText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);  // 256MB to avoid DFA out of memory
  RE2 re(kNearMisses, options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    bool matched = RE2::PartialMatch(text, re);
    benchmark::DoNotOptimize(matched);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_NearMisses(benchmark::State& state) {
  std::string text = NearMissesText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kNearMisses);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    bool matched = re->IsMatch(text);
    benchmark::DoNotOptimize(matched);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_CaptureHelper(benchmark::State& state, const char* pattern) {
  std::string text = CaptureText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);  // 256MB to avoid DFA out of memory
  RE2 re(pattern, options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    absl::string_view sub1, sub2, sub3;
    bool matched = RE2::PartialMatch(text, re, &sub1, &sub2, &sub3);
    benchmark::DoNotOptimize(matched);
    benchmark::DoNotOptimize(sub1);
    benchmark::DoNotOptimize(sub2);
    benchmark::DoNotOptimize(sub3);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_CaptureHelper(benchmark::State& state, const char* pattern) {
  std::string text = CaptureText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(pattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    absl::string_view sub1, sub2, sub3;
    bool matched = PartialMatch(text, *re, &sub1, &sub2, &sub3);
    benchmark::DoNotOptimize(matched);
    benchmark::DoNotOptimize(sub1);
    benchmark::DoNotOptimize(sub2);
    benchmark::DoNotOptimize(sub3);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

// ============================================================================
// Compilation Benchmark Helpers
// ============================================================================

void BM_RE2_Compile_Helper(benchmark::State& state, absl::string_view pattern) {
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations());
}

void BM_Regex_Compile_Helper(benchmark::State& state,
                             absl::string_view pattern) {
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations());
}

void BM_RE2_Compile_CaseInsensitive_Helper(benchmark::State& state,
                                           absl::string_view pattern) {
  RE2::Options options;
  options.set_max_mem(256 << 20);
  options.set_case_sensitive(false);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations());
}

void BM_Regex_Compile_CaseInsensitive_Helper(benchmark::State& state,
                                             absl::string_view pattern) {
  Options options;
  options.case_insensitive = true;
  CHECK_OK(Regex::Compile(pattern, options));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations());
}

void BM_Regex_Compile_RE2Compat_Helper(benchmark::State& state,
                                       absl::string_view pattern) {
  Options options;
  options.re2_compatibility = true;
  CHECK_OK(Regex::Compile(pattern, options));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations());
}

// --- Parameterized Compilation Benchmarks ---

void BM_RE2_Compile_AlternationBranches(benchmark::State& state) {
  const std::string pattern = GenerateAlternationPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_AlternationBranches(benchmark::State& state) {
  const std::string pattern = GenerateAlternationPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_LiteralLength(benchmark::State& state) {
  const std::string pattern = GenerateLiteralPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_LiteralLength(benchmark::State& state) {
  const std::string pattern = GenerateLiteralPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_BoundedRepetition(benchmark::State& state) {
  const std::string pattern = GenerateRepetitionPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_BoundedRepetition(benchmark::State& state) {
  const std::string pattern = GenerateRepetitionPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_CharClassRanges(benchmark::State& state) {
  const std::string pattern = GenerateCharClassPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_CharClassRanges(benchmark::State& state) {
  const std::string pattern = GenerateCharClassPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_CaptureGroups(benchmark::State& state) {
  const std::string pattern = GenerateCaptureGroupsPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_CaptureGroups(benchmark::State& state) {
  const std::string pattern = GenerateCaptureGroupsPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_NestedDepth(benchmark::State& state) {
  const std::string pattern = GenerateNestedPattern(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  CHECK(RE2(pattern, options).ok());
  for (auto _ : state) {
    RE2 re(pattern, options);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Compile_NestedDepth(benchmark::State& state) {
  const std::string pattern = GenerateNestedPattern(state.range(0));
  CHECK_OK(Regex::Compile(pattern));
  for (auto _ : state) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern);
    benchmark::DoNotOptimize(re.ok());
  }
  state.SetItemsProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Compile_RegexSet(benchmark::State& state) {
  const int num_patterns = state.range(0);
  const std::vector<std::string> patterns = GenerateSetPatterns(num_patterns);
  RE2::Options options;
  options.set_max_mem(256 << 20);
  for (auto _ : state) {
    RE2::Set set(options, RE2::UNANCHORED);
    for (const auto& p : patterns) {
      set.Add(p, nullptr);
    }
    bool ok = set.Compile();
    benchmark::DoNotOptimize(ok);
  }
  state.SetItemsProcessed(state.iterations() * num_patterns);
}

void BM_Regex_Compile_RegexSet(benchmark::State& state) {
  const int num_patterns = state.range(0);
  const std::vector<std::string> patterns = GenerateSetPatterns(num_patterns);
  std::vector<absl::string_view> pattern_views;
  pattern_views.reserve(patterns.size());
  for (const auto& p : patterns) {
    pattern_views.push_back(p);
  }
  for (auto _ : state) {
    absl::StatusOr<RegexSet> set = RegexSet::Compile(pattern_views);
    benchmark::DoNotOptimize(set);
  }
  state.SetItemsProcessed(state.iterations() * num_patterns);
}

// ============================================================================
// Replacement Benchmark Helpers
// ============================================================================

void BM_RE2_Replace_Capture(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re(kCapturePattern, options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    bool ok = RE2::Replace(&s, re, R"(\1.\2.\3)");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Replace_Capture(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kCapturePattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    bool ok = Replace(&s, *re, R"($1.$2.$3)");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Replace_Literal(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re("123-456-7890", options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    bool ok = RE2::Replace(&s, re, "REPLACED");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Replace_Literal(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile("123-456-7890");
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    bool ok = Replace(&s, *re, "REPLACED");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Replace_NoMatch(benchmark::State& state) {
  std::string text = RandomText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re("NON_EXISTENT_TOKEN_12345", options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    bool ok = RE2::Replace(&s, re, "REPLACED");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Replace_NoMatch(benchmark::State& state) {
  std::string text = RandomText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile("NON_EXISTENT_TOKEN_12345");
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    bool ok = Replace(&s, *re, "REPLACED");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_GlobalReplace_Dense(benchmark::State& state) {
  std::string text = DenseText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re(R"(item_(\d+))", options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    int count = RE2::GlobalReplace(&s, re, R"(entry_\1)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_GlobalReplace_Dense(benchmark::State& state) {
  std::string text = DenseText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(R"(item_(\d+))");
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    int count = GlobalReplace(&s, *re, R"(entry_$1)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_GlobalReplace_Capture(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re(kCapturePattern, options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    int count = RE2::GlobalReplace(&s, re, R"(\1.\2.\3)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_GlobalReplace_Capture(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kCapturePattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    int count = GlobalReplace(&s, *re, R"($1.$2.$3)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_GlobalReplace_Literal(benchmark::State& state) {
  std::string text = DenseText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re("item", options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    int count = RE2::GlobalReplace(&s, re, "object");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_GlobalReplace_Literal(benchmark::State& state) {
  std::string text = DenseText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile("item");
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    int count = GlobalReplace(&s, *re, "object");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_GlobalReplace_NoMatch(benchmark::State& state) {
  std::string text = RandomText(state.range(0));
  RE2::Options options;
  options.set_max_mem(256 << 20);
  RE2 re("NON_EXISTENT_TOKEN_12345", options);
  CHECK(re.ok()) << re.error();
  for (auto _ : state) {
    std::string s = text;
    int count = RE2::GlobalReplace(&s, re, "REPLACED");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_GlobalReplace_NoMatch(benchmark::State& state) {
  std::string text = RandomText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile("NON_EXISTENT_TOKEN_12345");
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string s = text;
    int count = GlobalReplace(&s, *re, "REPLACED");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_Replace_Uncompiled(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  for (auto _ : state) {
    std::string s = text;
    bool ok = RE2::Replace(&s, kCapturePattern, R"(\1.\2.\3)");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Replace_Uncompiled(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  for (auto _ : state) {
    std::string s = text;
    bool ok = Replace(&s, kCapturePattern, R"($1.$2.$3)");
    benchmark::DoNotOptimize(ok);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_RE2_GlobalReplace_Uncompiled(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  for (auto _ : state) {
    std::string s = text;
    int count = RE2::GlobalReplace(&s, kCapturePattern, R"(\1.\2.\3)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_GlobalReplace_Uncompiled(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  for (auto _ : state) {
    std::string s = text;
    int count = GlobalReplace(&s, kCapturePattern, R"($1.$2.$3)");
    benchmark::DoNotOptimize(count);
    benchmark::DoNotOptimize(s);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Member_Replace(benchmark::State& state) {
  std::string text = CaptureText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kCapturePattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string result = re->Replace(text, R"($1.$2.$3)");
    benchmark::DoNotOptimize(result);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Member_ReplaceAll(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kCapturePattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string result = re->ReplaceAll(text, R"($1.$2.$3)");
    benchmark::DoNotOptimize(result);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

void BM_Regex_Member_Replacen(benchmark::State& state) {
  std::string text = CapturePairsText(state.range(0));
  absl::StatusOr<Regex> re = Regex::Compile(kCapturePattern);
  CHECK(re.ok()) << re.status();
  for (auto _ : state) {
    std::string result = re->Replacen(text, 2, R"($1.$2.$3)");
    benchmark::DoNotOptimize(result);
  }
  state.SetBytesProcessed(state.iterations() * state.range(0));
}

// ============================================================================
// Benchmark Registration Macros
// ============================================================================

#define BENCHMARK_REGEX_PAIR(name, pattern)       \
  void BM_RE2_##name(benchmark::State& state) {   \
    BM_RE2_Search(state, pattern);                \
  }                                               \
  BENCHMARK(BM_RE2_##name)                        \
      ->RangeMultiplier(16)                       \
      ->Range(64, 1 << 20)                        \
      ->MinTime(0.05);                            \
                                                  \
  void BM_Regex_##name(benchmark::State& state) { \
    BM_Regex_Search(state, pattern);              \
  }                                               \
  BENCHMARK(BM_Regex_##name)                      \
      ->RangeMultiplier(16)                       \
      ->Range(64, 1 << 20)                        \
      ->MinTime(0.05);

// The other benchmarks start at size 8, but that's smaller than the pattern
// for the capture benchmark, so we use a larger value to guarantee all sizes
// will be able to match the pattern. Starting at 64 results in the same range
// as the other benchmarks, but skipping size 8 that doesn't fit the pattern.
#define BENCHMARK_CAPTURE_PAIR(name, pattern)     \
  void BM_RE2_##name(benchmark::State& state) {   \
    BM_RE2_CaptureHelper(state, pattern);         \
  }                                               \
  BENCHMARK(BM_RE2_##name)                        \
      ->RangeMultiplier(16)                       \
      ->Range(64, 1 << 20)                        \
      ->MinTime(0.05);                            \
                                                  \
  void BM_Regex_##name(benchmark::State& state) { \
    BM_Regex_CaptureHelper(state, pattern);       \
  }                                               \
  BENCHMARK(BM_Regex_##name)                      \
      ->RangeMultiplier(16)                       \
      ->Range(64, 1 << 20)                        \
      ->MinTime(0.05);

#define BENCHMARK_COMPILE_PAIR(name, pattern)             \
  void BM_RE2_Compile_##name(benchmark::State& state) {   \
    BM_RE2_Compile_Helper(state, pattern);                \
  }                                                       \
  BENCHMARK(BM_RE2_Compile_##name)->MinTime(0.05);        \
                                                          \
  void BM_Regex_Compile_##name(benchmark::State& state) { \
    BM_Regex_Compile_Helper(state, pattern);              \
  }                                                       \
  BENCHMARK(BM_Regex_Compile_##name)->MinTime(0.05);

#define BENCHMARK_COMPILE_CASE_INSENSITIVE_PAIR(name, pattern)            \
  void BM_RE2_Compile_CaseInsensitive_##name(benchmark::State& state) {   \
    BM_RE2_Compile_CaseInsensitive_Helper(state, pattern);                \
  }                                                                       \
  BENCHMARK(BM_RE2_Compile_CaseInsensitive_##name)->MinTime(0.05);        \
                                                                          \
  void BM_Regex_Compile_CaseInsensitive_##name(benchmark::State& state) { \
    BM_Regex_Compile_CaseInsensitive_Helper(state, pattern);              \
  }                                                                       \
  BENCHMARK(BM_Regex_Compile_CaseInsensitive_##name)->MinTime(0.05);

#define BENCHMARK_COMPILE_RE2_COMPAT(name, pattern)                 \
  void BM_Regex_Compile_RE2Compat_##name(benchmark::State& state) { \
    BM_Regex_Compile_RE2Compat_Helper(state, pattern);              \
  }                                                                 \
  BENCHMARK(BM_Regex_Compile_RE2Compat_##name)->MinTime(0.05);

// ============================================================================
// Benchmark Registrations
// ============================================================================

// --- Search Benchmarks ---
BENCHMARK_REGEX_PAIR(Literal, kLiteral)
BENCHMARK_REGEX_PAIR(PathBranch, kPathBranch)
BENCHMARK_REGEX_PAIR(Alternation, kAlternation)
BENCHMARK_REGEX_PAIR(LeadingRepetition, kLeadingRepetition)
BENCHMARK_REGEX_PAIR(Fanout, kFanout)
BENCHMARK_REGEX_PAIR(Parens, kParensPattern)
BENCHMARK_CAPTURE_PAIR(Capture, kCapturePattern)

BENCHMARK(BM_RE2_NearMisses)
    ->RangeMultiplier(16)
    ->Range(64, 1 << 20)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_NearMisses)
    ->RangeMultiplier(16)
    ->Range(64, 1 << 20)
    ->MinTime(0.05);

// --- Compilation Benchmarks: Fixed Patterns ---
BENCHMARK_COMPILE_PAIR(Literal, kLiteral)
BENCHMARK_COMPILE_PAIR(PathBranch, kPathBranch)
BENCHMARK_COMPILE_PAIR(Alternation, kAlternation)
BENCHMARK_COMPILE_PAIR(LeadingRepetition, kLeadingRepetition)
BENCHMARK_COMPILE_PAIR(Fanout, kFanout)
BENCHMARK_COMPILE_PAIR(NearMisses, kNearMisses)
BENCHMARK_COMPILE_PAIR(Capture, kCapturePattern)
BENCHMARK_COMPILE_PAIR(Parens, kParensPattern)
BENCHMARK_COMPILE_PAIR(Email, kEmail)
BENCHMARK_COMPILE_PAIR(Uri, kUri)
BENCHMARK_COMPILE_PAIR(Ipv4, kIpv4)
BENCHMARK_COMPILE_PAIR(Ipv6, kIpv6)
BENCHMARK_COMPILE_PAIR(Iso8601Date, kIso8601Date)
BENCHMARK_COMPILE_PAIR(Uuid, kUuid)
BENCHMARK_COMPILE_PAIR(HttpLog, kHttpLog)
BENCHMARK_COMPILE_PAIR(SemVer, kSemVer)
BENCHMARK_COMPILE_PAIR(UnicodeClasses, kUnicodeClasses)
BENCHMARK_COMPILE_PAIR(ComplexCharClass, kComplexCharClass)

// --- Compilation Benchmarks: Case-Insensitive Mode ---
BENCHMARK_COMPILE_CASE_INSENSITIVE_PAIR(PathBranch, kPathBranch)
BENCHMARK_COMPILE_CASE_INSENSITIVE_PAIR(Alternation, kAlternation)

// --- Compilation Benchmarks: RE2-Compatibility Rewriter Overhead ---
BENCHMARK_COMPILE_RE2_COMPAT(Capture, kCapturePattern)
BENCHMARK_COMPILE_RE2_COMPAT(Email, kEmail)
BENCHMARK_COMPILE_RE2_COMPAT(HttpLog, kHttpLog)

// --- Compilation Benchmarks: Parameterized / Scaled Patterns ---
BENCHMARK(BM_RE2_Compile_AlternationBranches)
    ->RangeMultiplier(4)
    ->Range(1, 1024)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_AlternationBranches)
    ->RangeMultiplier(4)
    ->Range(1, 1024)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_LiteralLength)
    ->RangeMultiplier(8)
    ->Range(8, 32768)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_LiteralLength)
    ->RangeMultiplier(8)
    ->Range(8, 32768)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_BoundedRepetition)
    ->RangeMultiplier(4)
    ->Range(1, 512)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_BoundedRepetition)
    ->RangeMultiplier(4)
    ->Range(1, 512)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_CharClassRanges)
    ->RangeMultiplier(4)
    ->Range(1, 256)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_CharClassRanges)
    ->RangeMultiplier(4)
    ->Range(1, 256)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_CaptureGroups)
    ->RangeMultiplier(4)
    ->Range(1, 128)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_CaptureGroups)
    ->RangeMultiplier(4)
    ->Range(1, 128)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_NestedDepth)
    ->RangeMultiplier(4)
    ->Range(1, 128)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_NestedDepth)
    ->RangeMultiplier(4)
    ->Range(1, 128)
    ->MinTime(0.05);

BENCHMARK(BM_RE2_Compile_RegexSet)
    ->RangeMultiplier(4)
    ->Range(1, 256)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Compile_RegexSet)
    ->RangeMultiplier(4)
    ->Range(1, 256)
    ->MinTime(0.05);

// --- Replacement Benchmarks ---
#define BENCHMARK_REPLACE_PAIR(name) \
  BENCHMARK(BM_RE2_##name)           \
      ->RangeMultiplier(16)          \
      ->Range(64, 1 << 20)           \
      ->MinTime(0.05);               \
  BENCHMARK(BM_Regex_##name)         \
      ->RangeMultiplier(16)          \
      ->Range(64, 1 << 20)           \
      ->MinTime(0.05);

BENCHMARK_REPLACE_PAIR(Replace_Capture)
BENCHMARK_REPLACE_PAIR(Replace_Literal)
BENCHMARK_REPLACE_PAIR(Replace_NoMatch)
BENCHMARK_REPLACE_PAIR(GlobalReplace_Dense)
BENCHMARK_REPLACE_PAIR(GlobalReplace_Capture)
BENCHMARK_REPLACE_PAIR(GlobalReplace_Literal)
BENCHMARK_REPLACE_PAIR(GlobalReplace_NoMatch)

BENCHMARK_REPLACE_PAIR(Replace_Uncompiled)
BENCHMARK_REPLACE_PAIR(GlobalReplace_Uncompiled)

BENCHMARK(BM_Regex_Member_Replace)
    ->RangeMultiplier(16)
    ->Range(64, 1 << 20)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Member_ReplaceAll)
    ->RangeMultiplier(16)
    ->Range(64, 1 << 20)
    ->MinTime(0.05);

BENCHMARK(BM_Regex_Member_Replacen)
    ->RangeMultiplier(16)
    ->Range(64, 1 << 20)
    ->MinTime(0.05);

}  // namespace
}  // namespace security::regex

BENCHMARK_MAIN();
