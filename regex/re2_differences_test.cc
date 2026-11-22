// This test documents currently known behavior differences between RE2 and Rust
// regex, that are NOT addressed by the re2_compatibility flag.
//
// These differences are all potential migration blockers, depending on how each
// particular project is using RE2. For hardcoded patterns, you'll most likely
// be able to rewrite the pattern to preserve the desired behavior. If you're
// dealing with user-provided patterns, you might be able to migrate users if
// they are internal. And if a project deals with patterns provided by external
// users, migrating will be a breaking change unless all these behavioral
// differences are somehow resolved.
//
// Some (maybe all?) of these could possibly be fixed by adding the necessary
// pattern rewriting logic in re2_compatibility mode, at least in theory.
//
// For example, RE2 will parse `[a[b]]` as the character class "[a[b]" (that is,
// one of {'a', '[', 'b'}) followed by a literal ']'. However, the Rust parser
// will interpret that input as a nested character class that accepts either 'a'
// or 'b'. So instead of doing AST rewrites we'd need to write our own parser
// for character classes and do the right escaping even before the Rust parser
// gets to see the pattern.

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "re2/re2.h"
#include "regex.h"

using absl_testing::IsOk;
using absl_testing::StatusIs;

namespace security::regex {
namespace {

// RE2 does not support nested character classes. Inside a character class,
// an unescaped `[` is treated as a literal `[`. The first `]` encountered
// closes the character class.
// Rust regex supports set operations and nested character classes.
TEST(Re2DifferencesTest, NestedCharacterClasses) {
  absl::string_view pattern = "[a[b]]";
  absl::string_view text = "b";

  // RE2 parses "[a[b]]" as "[a[b]" ('a' or '[' or 'b') followed by "]".
  // "b" does not match because RE2 expects a ']' at the end.
  RE2 re2(pattern);
  ASSERT_TRUE(re2.ok());
  EXPECT_FALSE(RE2::PartialMatch(text, re2));

  // Rust regex parses `[a[b]]` as a character class containing 'a' and
  // the nested class `[b]`. Thus, it matches "b".
  absl::StatusOr<Regex> regex = Regex::Compile(
      pattern, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(regex, IsOk());
  EXPECT_TRUE(regex->IsMatch(text));
}

// RE2 interprets `<` and `>` after a backslash as literal `<` and `>`
// characters. Rust regex interprets `\<` and `\>` as start-of-word and
// end-of-word boundaries.
TEST(Re2DifferencesTest, WordBoundaries) {
  absl::string_view pattern = R"(\<foo\>)";
  absl::string_view text = "foo";

  // RE2 expects literal "<foo>".
  RE2 re2(pattern);
  ASSERT_TRUE(re2.ok());
  EXPECT_FALSE(RE2::PartialMatch(text, re2));

  // Rust regex recognizes start/end of word boundaries and matches "foo".
  absl::StatusOr<Regex> regex = Regex::Compile(
      pattern, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(regex, IsOk());
  EXPECT_TRUE(regex->IsMatch(text));
}

// RE2 supports \C to match any single byte (even in UTF-8 mode).
// Rust regex does not support \C.
TEST(Re2DifferencesTest, ArbitraryByteEscapeC) {
  absl::string_view pattern = R"(\C)";

  RE2 re2(pattern);
  EXPECT_TRUE(re2.ok());

  EXPECT_THAT(Regex::Compile(pattern, {.encoding = Encoding::kUtf8,
                                       .re2_compatibility = true}),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

// RE2 doesn't allow whitespace inside repetition braces (e.g. "x{0}"). If
// whitespace is encountered (e.g. {0 }), RE2 aborts repetition parsing and
// treats `{` as a literal brace.
// Rust regex skips whitespace inside braces, successfully parsing `{0 }` as
// `{0}`.
TEST(Re2DifferencesTest, RepetitionWhitespace) {
  absl::string_view pattern = "a{0 }";
  absl::string_view text = "";

  // RE2 treats pattern as literal "a{0 }".
  RE2 re2(pattern);
  ASSERT_TRUE(re2.ok());
  EXPECT_FALSE(RE2::PartialMatch(text, re2));

  // Rust regex parses pattern as "a{0}" (matching the empty string).
  absl::StatusOr<Regex> regex = Regex::Compile(
      pattern, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(regex, IsOk());
  EXPECT_TRUE(regex->IsMatch(text));

  // Note: RE2 does not support verbose mode or the `(?x)` flag at all, so we
  // can't use that mode to sneak in whitespace inside repetition braces either.
  // Attempting to use `(?x)` in RE2 causes a compilation failure.
  RE2 re2_verbose("(?x)a{0 }");
  EXPECT_FALSE(re2_verbose.ok());

  // Rust regex fully supports verbose mode (either via Options or (?x)) and
  // continues to match correctly.
  absl::StatusOr<Regex> regex_verbose =
      Regex::Compile(pattern, {.encoding = Encoding::kUtf8,
                               .verbose = true,
                               .re2_compatibility = true});
  ASSERT_THAT(regex_verbose, IsOk());
  EXPECT_TRUE(regex_verbose->IsMatch(text));
}

// RE2 also disallows leading zeros in decimal quantifier counts. When
// encountered, RE2 aborts repetition parsing and treats braces as literals.
// Rust regex accepts leading zeros, parsing `{00}` as `{0}`.
TEST(Re2DifferencesTest, RepetitionLeadingZeros) {
  absl::string_view pattern = "a{00}";
  absl::string_view text = "";

  // RE2 treats pattern as literal "a{00}".
  RE2 re2(pattern);
  ASSERT_TRUE(re2.ok());
  EXPECT_FALSE(RE2::PartialMatch(text, re2));

  // Rust regex parses pattern as "a{0}" (matching empty string).
  absl::StatusOr<Regex> regex = Regex::Compile(
      pattern, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(regex, IsOk());
  EXPECT_TRUE(regex->IsMatch(text));
}

// RE2 uses an older case-folding table (`unicode_casefold.cc`) that lacks case
// mappings for some Cyrillic Extended-C characters added in Unicode 16.0.
// Rust regex supports modern Unicode case folding for these characters.
TEST(Re2DifferencesTest, CyrillicCaseFolding) {
  absl::string_view pattern =
      "\xe1\xb2\x8a";  // U+1C8A (CYRILLIC SMALL LETTER TJE)
  absl::string_view text =
      "\xe1\xb2\x89";  // U+1C89 (CYRILLIC CAPITAL LETTER TJE)

  // RE2 lacks case folding for U+1C8A, so it fails to match.
  RE2::Options options;
  options.set_case_sensitive(false);
  RE2 re2(pattern, options);
  ASSERT_TRUE(re2.ok());
  EXPECT_FALSE(RE2::PartialMatch(text, re2));

  // Rust regex case folding successfully matches.
  absl::StatusOr<Regex> regex =
      Regex::Compile(pattern, {.encoding = Encoding::kUtf8,
                               .case_insensitive = true,
                               .re2_compatibility = true});
  ASSERT_THAT(regex, IsOk());
  EXPECT_TRUE(regex->IsMatch(text));
}

}  // namespace
}  // namespace security::regex
