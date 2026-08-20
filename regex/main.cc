#include <iostream>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "regex.h"
#include "absl/status/statusor.h"

int main() {
  std::cout << "=== Security Regex Library Showcase ===\n\n";

  // 1. Basic Matching
  std::cout << "--- 1. Basic Matching ---\n";
  security::regex::Regex email_regex = *security::regex::Regex::Compile(
      R"(\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b)");

  std::string_view sample_email = "user@example.com";
  std::cout << "Checking '" << sample_email << "': "
            << (email_regex.IsMatch(sample_email) ? "MATCHED" : "NO MATCH")
            << "\n\n";

  // 2. Finding Matches & Extracting Substrings
  std::cout << "--- 2. Finding Substring Matches ---\n";
  std::string_view text =
      "Contact us at support@example.com or sales@example.org for info.";
  std::optional<security::regex::Match> first_match = email_regex.Find(text);
  if (first_match.has_value()) {
    std::cout << "First match found: '" << first_match->AsStr()
              << "' at position [" << first_match->Start() << ", "
              << first_match->End() << ")\n";
  }

  // 3. Iterating All Matches
  std::cout << "\n--- 3. Iterating All Matches ---\n";
  for (const security::regex::Match& match : email_regex.FindAll(text)) {
    std::cout << " - Found email: '" << match.AsStr() << "'\n";
  }

  // 4. Named Capture Groups & Expansion
  security::regex::Regex kv_regex = *security::regex::Regex::Compile(
      R"raw((?P<key>[a-z_]+)\s*=\s*"(?P<value>[^"]*)")raw");
  std::string_view config = R"(setting_name = "production_mode")";
  std::optional<security::regex::Captures> captures =
      kv_regex.FindCaptures(config);
  if (captures.has_value()) {
    std::cout << "Full match: " << captures->GetMatch()->AsStr() << "\n";
    if (std::optional<security::regex::Match> key = captures->Get("key")) {
      std::cout << "Key: " << key->AsStr() << "\n";
    }
    if (std::optional<security::regex::Match> val = captures->Get("value")) {
      std::cout << "Value: " << val->AsStr() << "\n";
    }
    std::string expanded = captures->Expand("JSON: {\"$key\": \"$value\"}");
    std::cout << "Expanded: " << expanded << "\n";
  }

  // 5. Multi-pattern Matching with RegexSet
  std::cout << "\n--- 5. Multi-pattern Matching with RegexSet ---\n";
  std::vector<std::string_view> patterns = {"ERR_[0-9]+", "WARN_[0-9]+",
                                            "INFO_[0-9]+"};
  security::regex::RegexSet set = *security::regex::RegexSet::Compile(patterns);
  std::string_view log_line =
      "System status: ERR_404 occurred alongside WARN_12";
  security::regex::SetMatches matches = set.Matches(log_line);
  std::cout << "Log line matches " << matches.MatchedIndices().size()
            << " pattern(s):\n";
  for (size_t idx : matches.MatchedIndices()) {
    std::cout << " - Matched pattern index " << idx << " (" << patterns[idx]
              << ")\n";
  }

  // 6. Using Options (Case Insensitive Matching)
  std::cout << "\n--- 6. Options (Case Insensitive) ---\n";
  security::regex::Options options;
  options.case_insensitive = true;
  security::regex::Regex ci_regex =
      *security::regex::Regex::Compile("hello world", options);
  std::cout << "Case insensitive match 'HeLLo WoRLd': "
            << (ci_regex.IsMatch("HeLLo WoRLd") ? "MATCHED" : "NO MATCH")
            << "\n";

  return 0;
}
