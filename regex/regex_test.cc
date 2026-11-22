#include "regex.h"

#include <cstddef>
#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/status/status_matchers.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

using absl_testing::IsOk;
using absl_testing::StatusIs;
using testing::ElementsAre;
using testing::Eq;
using testing::FieldsAre;
using testing::Optional;

namespace security::regex {
namespace {

template <typename T>
concept CanCallMatchAsStr = requires(T m) { std::forward<T>(m).AsStr(); };
template <typename T>
concept CanCallInner = requires(T m) { std::forward<T>(m).Inner(); };
template <typename T>
concept CanCallGetAll = requires(T c) { std::forward<T>(c).GetAll(); };
template <typename T>
concept CanCallAsStr = requires(T r) { std::forward<T>(r).AsStr(); };
template <typename T>
concept CanCallCaptureNames =
    requires(T r) { std::forward<T>(r).CaptureNames(); };
template <typename T>
concept CanCallFindAll =
    requires(T r, absl::string_view text) { std::forward<T>(r).FindAll(text); };
template <typename T>
concept CanCallFindAllCaptures = requires(T r, absl::string_view text) {
  std::forward<T>(r).FindAllCaptures(text);
};
template <typename T>
concept CanCallSplit =
    requires(T r, absl::string_view text) { std::forward<T>(r).Split(text); };
template <typename T>
concept CanCallSplitN = requires(T r, absl::string_view text, size_t limit) {
  std::forward<T>(r).Split(text, limit);
};

// Match::AsStr() borrows the input text rather than the Match itself, so it
// remains callable on both lvalues and rvalues.
static_assert(CanCallMatchAsStr<const Match&>);
static_assert(CanCallMatchAsStr<Match&>);
static_assert(CanCallMatchAsStr<Match&&>);
static_assert(CanCallMatchAsStr<const Match&&>);

static_assert(CanCallInner<const Match&>);
static_assert(CanCallInner<Match&>);
static_assert(!CanCallInner<Match&&>);
static_assert(!CanCallInner<const Match&&>);

static_assert(CanCallGetAll<const Captures&>);
static_assert(CanCallGetAll<Captures&>);
static_assert(!CanCallGetAll<Captures&&>);
static_assert(!CanCallGetAll<const Captures&&>);

static_assert(CanCallAsStr<const Regex&>);
static_assert(CanCallAsStr<Regex&>);
static_assert(!CanCallAsStr<Regex&&>);
static_assert(!CanCallAsStr<const Regex&&>);

static_assert(CanCallCaptureNames<const Regex&>);
static_assert(CanCallCaptureNames<Regex&>);
static_assert(!CanCallCaptureNames<Regex&&>);
static_assert(!CanCallCaptureNames<const Regex&&>);

static_assert(CanCallFindAll<const Regex&>);
static_assert(CanCallFindAll<Regex&>);
static_assert(!CanCallFindAll<Regex&&>);
static_assert(!CanCallFindAll<const Regex&&>);

static_assert(CanCallFindAllCaptures<const Regex&>);
static_assert(CanCallFindAllCaptures<Regex&>);
static_assert(!CanCallFindAllCaptures<Regex&&>);
static_assert(!CanCallFindAllCaptures<const Regex&&>);

static_assert(CanCallSplit<const Regex&>);
static_assert(CanCallSplit<Regex&>);
static_assert(!CanCallSplit<Regex&&>);
static_assert(!CanCallSplit<const Regex&&>);

static_assert(CanCallSplitN<const Regex&>);
static_assert(CanCallSplitN<Regex&>);
static_assert(!CanCallSplitN<Regex&&>);
static_assert(!CanCallSplitN<const Regex&&>);

template <typename T>
absl::string_view AsStringView(const T& s) {
  if (s.data() == nullptr) return {};
  return absl::string_view(reinterpret_cast<const char*>(s.data()), s.size());
}

TEST(RegexTest, Compile) {
  ASSERT_THAT(Regex::Compile("abc"), IsOk());
  ASSERT_THAT(Regex::Compile("ab("),
              StatusIs(absl::StatusCode::kInvalidArgument));
  // Test that the parser that rewrites character classes also rejects invalid
  // regexes in RE2 compatibility mode.
  ASSERT_THAT(Regex::Compile("ab(", {.encoding = Encoding::kUtf8,
                                     .re2_compatibility = true}),
              StatusIs(absl::StatusCode::kInvalidArgument));
  // Test that invalid UTF-8 pattern is rejected in RE2 compatibility mode.
  ASSERT_THAT(Regex::Compile("\xFF", {.encoding = Encoding::kUtf8,
                                      .re2_compatibility = true}),
              StatusIs(absl::StatusCode::kInvalidArgument));

  // Test that invalid UTF-8 pattern is accepted in kBytes mode.
  ASSERT_THAT(Regex::Compile("\xFF", {.encoding = Encoding::kBytes}), IsOk());
}

TEST(RegexTest, Clone) {
  absl::StatusOr<Regex> r = Regex::Compile("abc");
  ASSERT_THAT(r, IsOk());
  // The underlying Regex object implements `Clone`, so our wrapper objects
  // should be copyable as well.
  Regex clone = *r;
  ASSERT_TRUE(clone.IsMatch("xyz abc 123"));
}

TEST(RegexTest, AsStr) {
  absl::StatusOr<Regex> r = Regex::Compile("abc");
  ASSERT_THAT(r, IsOk());
  EXPECT_THAT(r->AsStr(), Eq("abc"));

  absl::StatusOr<Regex> empty_r = Regex::Compile("");
  ASSERT_THAT(empty_r, IsOk());
  EXPECT_THAT(empty_r->AsStr(), Eq(""));

  absl::StatusOr<Regex> anchored_r =
      Regex::Compile("abc", {.add_begin_and_end_anchors = true});
  ASSERT_THAT(anchored_r, IsOk());
  EXPECT_THAT(anchored_r->AsStr(), Eq(R"(\A(?:abc)\z)"));

  // Consuming the view inside the full-expression via StatusOr::operator-> is
  // valid because the temporary StatusOr<Regex> lives until the semicolon.
  EXPECT_THAT(std::string(Regex::Compile("abc")->AsStr()), Eq("abc"));
}

TEST(RegexTest, Find) {
  absl::StatusOr<Regex> r = Regex::Compile(R"([a-z]{3})");
  ASSERT_THAT(r, IsOk());
  std::optional<Match> m = r->Find("123 456 abc 789 xyz");
  // Check that we found the first match.
  ASSERT_TRUE(m.has_value());
  ASSERT_THAT(m->AsStr(), Eq("abc"));
  EXPECT_THAT(m->Start(), Eq(8));
  EXPECT_THAT(m->End(), Eq(11));
  EXPECT_THAT(m->Len(), Eq(3));
  EXPECT_FALSE(m->IsEmpty());
  EXPECT_THAT(AsStringView(m->Inner().as_str()), Eq("abc"));
  EXPECT_THAT(m->Inner().start(), Eq(8));
  EXPECT_THAT(m->Inner().end(), Eq(11));
  EXPECT_THAT(m->Inner().len(), Eq(3));
  EXPECT_FALSE(m->Inner().is_empty());

  // Match::AsStr() is also callable on a temporary Match.
  EXPECT_THAT((*r->Find("123 456 abc 789 xyz")).AsStr(), Eq("abc"));

  // Zero-length match boundary case.
  absl::StatusOr<Regex> empty_r = Regex::Compile("a*");
  ASSERT_THAT(empty_r, IsOk());
  std::optional<Match> empty_m = empty_r->Find("bbb");
  ASSERT_TRUE(empty_m.has_value());
  EXPECT_THAT(empty_m->AsStr(), Eq(""));
  EXPECT_THAT(empty_m->Start(), Eq(0));
  EXPECT_THAT(empty_m->End(), Eq(0));
  EXPECT_THAT(empty_m->Len(), Eq(0));
  EXPECT_TRUE(empty_m->IsEmpty());
  EXPECT_THAT(AsStringView(empty_m->Inner().as_str()), Eq(""));
  EXPECT_THAT(empty_m->Inner().start(), Eq(0));
  EXPECT_THAT(empty_m->Inner().end(), Eq(0));
  EXPECT_THAT(empty_m->Inner().len(), Eq(0));
  EXPECT_TRUE(empty_m->Inner().is_empty());
}

// Check that we can find an invalid UTF-8 pattern in "bytes" mode.
TEST(RegexTest, FindInvalidUtf8) {
  // Here the pattern contains an actual 0xFF byte. This is not valid
  // UTF-8, so it has to be replaced with a "\xff" before reaching the Rust
  // regex parser.
  absl::StatusOr<Regex> r2 =
      Regex::Compile("[a-z\xff]{3}", {.encoding = Encoding::kBytes});
  ASSERT_THAT(r2, IsOk());
  std::optional<Match> m2 = r2->Find("123 456 ab\xff 789 xyz");
  // Check that we found the first match.
  ASSERT_TRUE(m2.has_value());
  ASSERT_THAT(m2->AsStr(), Eq("ab\xff"));

  // Here \xff is escaped, because we're passing it in a raw string. So this
  // string is already valid UTF-8. Check that our pre-processing doesn't mangle
  // the backslash.
  absl::StatusOr<Regex> r =
      Regex::Compile(R"([a-z\xff]{3})", {.encoding = Encoding::kBytes});
  ASSERT_THAT(r, IsOk());
  std::optional<Match> m = r->Find("123 456 ab\xff 789 xyz");
  // Check that we found the first match.
  ASSERT_TRUE(m.has_value());
  ASSERT_THAT(m->AsStr(), Eq("ab\xff"));
}

TEST(RegexTest, Captures) {
  // We'll leave the optional fourth capture unmatched for testing.
  absl::string_view pattern =
      R"((?<first>\d+)\s+(?<second>\d+)\s+(?<third>\d+)(?<fourth> \d+)?)";
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  ASSERT_THAT(r, IsOk());
  std::optional<Captures> c = r->FindCaptures("xxx 123 456 789 yyy");
  ASSERT_TRUE(c.has_value());
  // Get whole match.
  ASSERT_TRUE(c->GetMatch().has_value());
  ASSERT_THAT(c->GetMatch()->AsStr(), Eq("123 456 789"));
  // Get captures by number.
  ASSERT_THAT(c->Get(0)->AsStr(), Eq("123 456 789"));
  ASSERT_THAT(c->Get(1)->AsStr(), Eq("123"));
  ASSERT_THAT(c->Get(2)->AsStr(), Eq("456"));
  ASSERT_THAT(c->Get(3)->AsStr(), Eq("789"));
  ASSERT_THAT(c->Get(4), Eq(std::nullopt));  // Not matched.
  ASSERT_THAT(c->Get(5), Eq(std::nullopt));  // Not a valid index.
  // Get captures by name.
  ASSERT_THAT(c->Get("first")->AsStr(), Eq("123"));
  ASSERT_THAT(c->Get("second")->AsStr(), Eq("456"));
  ASSERT_THAT(c->Get("third")->AsStr(), Eq("789"));
  ASSERT_THAT(c->Get("fourth"), Eq(std::nullopt));   // Not matched.
  ASSERT_THAT(c->Get("none"), Eq(std::nullopt));     // Not a valid name.
  ASSERT_THAT(c->Get("aaa\xff"), Eq(std::nullopt));  // Not valid UTF-8.
  // Iterate matches for all capture groups.
  std::vector<std::optional<std::string>> matches;
  for (auto&& m : c->GetAll()) {
    if (m.has_value()) {
      matches.push_back(std::string(m->AsStr()));
    } else {
      matches.push_back(std::nullopt);
    }
  }
  ASSERT_THAT(matches, ElementsAre(Eq("123 456 789"), Eq("123"), Eq("456"),
                                   Eq("789"), Eq(std::nullopt)));

  std::string expand_result = c->Expand("$second aaa $first");
  ASSERT_THAT(expand_result, Eq("456 aaa 123"));
}

TEST(RegexTest, CaptureNames) {
  absl::string_view pattern = "(?<first>a)(?<second>b)(c)(?<fourth>d)";
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  ASSERT_THAT(r, IsOk());
  std::vector<std::optional<absl::string_view>> v;
  for (auto&& name : r->CaptureNames()) {
    v.push_back(name);
  }
  ASSERT_THAT(v, ElementsAre(Eq(std::nullopt), Optional(Eq("first")),
                             Optional(Eq("second")), Eq(std::nullopt),
                             Optional(Eq("fourth"))));

  // Consuming CaptureNames() within the full-expression via
  // StatusOr::operator-> is valid.
  auto collect_names = [](Regex::CaptureNamesResult range) {
    std::vector<std::optional<std::string>> names;
    for (auto&& name : range) {
      names.push_back(name.has_value() ? std::optional<std::string>(*name)
                                       : std::nullopt);
    }
    return names;
  };
  EXPECT_THAT(collect_names(Regex::Compile(pattern)->CaptureNames()),
              ElementsAre(Eq(std::nullopt), Optional(Eq("first")),
                          Optional(Eq("second")), Eq(std::nullopt),
                          Optional(Eq("fourth"))));
}

TEST(RegexTest, NamedCapturingGroups) {
  absl::string_view pattern = "(?<first>a)(?<second>b)(c)(?<fourth>d)";
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  ASSERT_THAT(r, IsOk());
  std::map<std::string, int> names = r->NamedCapturingGroups();
  std::map<std::string, int> expected{
      {"first", 1},
      {"second", 2},
      {"fourth", 4},
  };
  ASSERT_THAT(names, Eq(expected));
}

TEST(RegexTest, CapturingGroupNames) {
  absl::string_view pattern = "(?<first>a)(?<second>b)(c)(?<fourth>d)";
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  ASSERT_THAT(r, IsOk());
  std::map<int, std::string> names = r->CapturingGroupNames();
  std::map<int, std::string> expected{
      {1, "first"},
      {2, "second"},
      {4, "fourth"},
  };
  ASSERT_THAT(names, Eq(expected));
}

TEST(RegexTest, FindAll) {
  absl::StatusOr<Regex> r = Regex::Compile(R"(\d{3})");
  ASSERT_THAT(r, IsOk());
  std::vector<std::string> matches;
  for (auto&& match : r->FindAll("123 456 abc 789")) {
    matches.push_back(std::string(match.AsStr()));
  }
  ASSERT_THAT(matches, ElementsAre("123", "456", "789"));

  auto collect_matches = [](Regex::FindAllResult range) {
    std::vector<std::string> result;
    for (auto&& match : range) {
      result.push_back(std::string(match.AsStr()));
    }
    return result;
  };
  EXPECT_THAT(
      collect_matches(Regex::Compile(R"(\d{3})")->FindAll("123 456 abc 789")),
      ElementsAre("123", "456", "789"));
}

TEST(RegexTest, FindAllCaptures) {
  absl::StatusOr<Regex> r = Regex::Compile(R"((?<key>\w+)=(?<value>\d+))");
  ASSERT_THAT(r, IsOk());
  struct Entry {
    std::string key;
    int value;
  };
  std::vector<Entry> entries;
  for (auto&& c : r->FindAllCaptures("first=123 and then second=456")) {
    entries.push_back({std::string(c.Get("key")->AsStr()),
                       std::stoi(std::string(c.Get("value")->AsStr()))});
  }
  ASSERT_THAT(entries,
              ElementsAre(FieldsAre("first", 123), FieldsAre("second", 456)));

  auto collect_entries = [](Regex::FindAllCapturesResult range) {
    std::vector<Entry> result;
    for (auto&& c : range) {
      result.push_back({std::string(c.Get("key")->AsStr()),
                        std::stoi(std::string(c.Get("value")->AsStr()))});
    }
    return result;
  };
  EXPECT_THAT(
      collect_entries(Regex::Compile(R"((?<key>\w+)=(?<value>\d+))")
                          ->FindAllCaptures("first=123 and then second=456")),
      ElementsAre(FieldsAre("first", 123), FieldsAre("second", 456)));
}

TEST(RegexTest, Split) {
  absl::StatusOr<Regex> r = Regex::Compile(R"(\s+)");
  ASSERT_THAT(r, IsOk());
  // Test Split.
  std::vector<absl::string_view> v;
  for (auto item : r->Split("abc xyz  123   456")) {
    v.push_back(std::move(item));
  }
  ASSERT_THAT(v, ElementsAre(Eq("abc"), Eq("xyz"), Eq("123"), Eq("456")));
  // Test Split with limit.
  v.clear();
  for (auto item : r->Split("abc xyz  123   456", 3)) {
    v.push_back(std::move(item));
  }
  ASSERT_THAT(v, ElementsAre(Eq("abc"), Eq("xyz"), Eq("123   456")));

  auto collect_splits = [](Regex::SplitResult range) {
    std::vector<std::string> result;
    for (absl::string_view item : range) {
      result.push_back(std::string(item));
    }
    return result;
  };
  auto collect_split_n = [](Regex::SplitNResult range) {
    std::vector<std::string> result;
    for (absl::string_view item : range) {
      result.push_back(std::string(item));
    }
    return result;
  };
  EXPECT_THAT(
      collect_splits(Regex::Compile(R"(\s+)")->Split("abc xyz  123   456")),
      ElementsAre(Eq("abc"), Eq("xyz"), Eq("123"), Eq("456")));
  EXPECT_THAT(
      collect_split_n(Regex::Compile(R"(\s+)")->Split("abc xyz  123   456", 3)),
      ElementsAre(Eq("abc"), Eq("xyz"), Eq("123   456")));
}

// Tests the PartialMatch function, which is similar to RE2::PartialMatch and
// allows the user to pass their own variables to receive captures, and does
// parsing of numeric values.
TEST(RegexTest, PartialMatch) {
  // Last capture catches all non-whitespace characters so it can match the 0x
  // prefix to test parsing of hexadecimal values.
  absl::string_view pattern = R"(([-0-9]+)\s+([-0-9]+)\s+(\S+))";
  int a = 0;
  int b = 0;
  int c = 0;
  // Test match without captures.
  ASSERT_TRUE(PartialMatch("1234    5678 0x100", pattern));
  // Test match with captures.
  ASSERT_TRUE(PartialMatch("prefix 1234    -5678 0x100 suffix", pattern, &a, &b,
                           Hex(&c)));
  ASSERT_THAT(a, Eq(1234));
  ASSERT_THAT(b, Eq(-5678));
  ASSERT_THAT(c, Eq(0x100));

  // Test using ^ and $ to force a full string match.
  ASSERT_FALSE(PartialMatch("prefix 1234    5678 0x100 suffix",
                            R"(^(\d+)\s+(\d+)\s+(\S+)$)"));

  // Test various numeric types and bases.
  float f = 0;
  double d = 0;
  int8_t i8 = 0;
  uint8_t u8 = 0;
  int64_t i64 = 0;
  int oct = 0;

  ASSERT_TRUE(PartialMatch("1.5 2.5e-10 -127 255 123456789012345 0123",
                           R"((\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+))",
                           &f, &d, &i8, &u8, &i64, CRadix(&oct)));
  EXPECT_THAT(f, Eq(1.5f));
  EXPECT_THAT(d, Eq(2.5e-10));
  EXPECT_THAT(i8, Eq(-127));
  EXPECT_THAT(u8, Eq(255));
  EXPECT_THAT(i64, Eq(123456789012345LL));
  EXPECT_THAT(oct, Eq(0123));  // Octal 123 = 83 decimal
}

TEST(RegexTest, FullMatchCaptureNumbering) {
  int a = 0;
  int b = 0;
  // The non-capturing group in FullMatch should not affect capture numbering.
  // Patterns: (12) (34)
  // If the outer (?:...) used in FullMatch were a capturing group, (12) would
  // have index 2 instead of 1.
  ASSERT_TRUE(FullMatch("1234", R"((\d{2})(\d{2}))", &a, &b));
  EXPECT_THAT(a, Eq(12));
  EXPECT_THAT(b, Eq(34));
}

// Check if our approach consisting in adding anchors to simulate a full match
// still works when the user passes a pattern that already contains anchors.
TEST(RegexTest, FullMatchAlreadyAnchored) {
  // Anchored on one end.
  EXPECT_TRUE(FullMatch("1234", R"(^\d{4})"));
  EXPECT_TRUE(FullMatch("1234", R"(\d{4}$)"));
  // Anchored on both ends.
  EXPECT_TRUE(FullMatch("1234", R"(^\d{4}$)"));
}

TEST(RegexTest, NumericParsingErrors) {
  int i;
  uint32_t u32;
  float f;

  // Non-numeric
  EXPECT_FALSE(FullMatch("abc", R"((\w+))", &i));

  // Overflow/Underflow
  EXPECT_FALSE(FullMatch("2147483648", R"((\d+))", &i));
  EXPECT_FALSE(FullMatch("-2147483649", R"((-?\d+))", &i));
  EXPECT_FALSE(FullMatch("4294967296", R"((\d+))", &u32));

  // Empty match
  EXPECT_FALSE(FullMatch("", R"((\d*))", &i));

  // Garbage after number within the match
  EXPECT_FALSE(FullMatch("123x", R"((\d+.))", &i));

  // Whitespace for integers
  EXPECT_FALSE(FullMatch(" 123", R"((.*))", &i));
  EXPECT_FALSE(FullMatch("123 ", R"((.*))", &i));

  // Whitespace for floats (leading allowed, trailing NOT allowed in RE2)
  EXPECT_TRUE(FullMatch(" 1.5", R"((.*))", &f));
  EXPECT_THAT(f, Eq(1.5f));
  EXPECT_FALSE(FullMatch("1.5 ", R"((.*))", &f));

  // Mismatched bases
  EXPECT_FALSE(FullMatch("0xG", R"((.*))", Hex(&i)));
  EXPECT_FALSE(FullMatch("09", R"((.*))", Octal(&i)));
  EXPECT_FALSE(FullMatch("0x123", R"((.*))", Octal(&i)));
}

TEST(RegexTest, OptionalCaptures) {
  std::string s;
  std::optional<int> oi = 42;
  // Second group won't match.
  ASSERT_TRUE(FullMatch("ruby", R"((\w+)(?::(\d+))?)", &s, &oi));
  EXPECT_THAT(s, Eq("ruby"));
  EXPECT_THAT(oi, Eq(std::nullopt));

  // Check unmatched optional string. It doesn't fail and the string capture
  // argument gets cleared.
  s = "this string shouldn't be preserved";
  ASSERT_TRUE(FullMatch("aaa", R"(\w+(:\w+)?)", &s));
  EXPECT_THAT(s, Eq(""));

  // Same with string_view.
  absl::string_view sv = "this string shouldn't be preserved either";
  ASSERT_TRUE(FullMatch("aaa", R"(\w+(:\w+)?)", &sv));
  EXPECT_THAT(sv, Eq(""));

  sv = "this string should be replaced by a captured value";
  ASSERT_TRUE(FullMatch("aaa:bbb", R"(\w+(:\w+)?)", &sv));
  EXPECT_THAT(sv, Eq(":bbb"));

  // Using std::optional<std::string> works.
  // It becomes a nullopt when not matched.
  std::optional<std::string> os = "not preserved";
  ASSERT_TRUE(FullMatch("ruby", R"((\w+)(:(\w+))?)", &s, &os));
  EXPECT_THAT(os, Eq(std::nullopt));

  // And it takes the captured value when matched.
  ASSERT_TRUE(FullMatch("ruby:test", R"((\w+)(:(\w+))?)", &s, &os));
  EXPECT_THAT(os, Optional(Eq(":test")));

  // Check unmatched optional number.
  int i;
  ASSERT_FALSE(FullMatch("a", R"(a(\d+)?)", &i));

  ASSERT_TRUE(FullMatch("a", R"(a(\d+)?)", &oi));
  ASSERT_THAT(oi, Eq(std::nullopt));

  // Check matched optional number.
  ASSERT_TRUE(FullMatch("a123", R"(a(\d+)?)", &oi));
  ASSERT_THAT(oi, Optional(Eq(123)));
}

TEST(RegexTest, FullMatchAlternation) {
  // Pattern a|abc with input abc. Leftmost-first matching would find 'a',
  // but FullMatch should find 'abc'.
  ASSERT_TRUE(FullMatch("abc", "a|abc"));
}

TEST(RegexTest, Consume) {
  absl::string_view input = "123 456 789";
  int a = 0;
  ASSERT_TRUE(Consume(&input, R"((\d+)\s+)", &a));
  EXPECT_THAT(a, Eq(123));
  EXPECT_THAT(input, Eq("456 789"));

  ASSERT_TRUE(Consume(&input, R"((\d+)\s+)", &a));
  EXPECT_THAT(a, Eq(456));
  EXPECT_THAT(input, Eq("789"));

  // No match at start.
  input = "prefix 123";
  EXPECT_FALSE(Consume(&input, R"(\d+)", &a));
  EXPECT_THAT(input, Eq("prefix 123"));

  // Match with Regex object.
  input = "123 456";
  absl::StatusOr<Regex> r = Regex::Compile(R"((\d+))");
  ASSERT_THAT(r, IsOk());
  ASSERT_TRUE(Consume(&input, *r, &a));
  EXPECT_THAT(a, Eq(123));
  EXPECT_THAT(input, Eq(" 456"));
}

TEST(RegexTest, ConsumeAlternation) {
  // Unlike FullMatch, Consume (and RE2::Consume) picks the first matching
  // alternation because it is leftmost-first. For input "abc" and pattern
  // "(a|abc)", it will match "a" and leave "bc".
  absl::string_view input = "abc";
  std::string match;
  ASSERT_TRUE(Consume(&input, "(a|abc)", &match));
  EXPECT_THAT(match, Eq("a"));
  EXPECT_THAT(input, Eq("bc"));

  // If the longer alternation is first, it will be picked.
  input = "abc";
  ASSERT_TRUE(Consume(&input, "(abc|a)", &match));
  EXPECT_THAT(match, Eq("abc"));
  EXPECT_THAT(input, Eq(""));
}

TEST(RegexTest, FindAndConsume) {
  absl::string_view input = "prefix 123 suffix";
  int a = 0;
  ASSERT_TRUE(FindAndConsume(&input, R"((\d+))", &a));
  EXPECT_THAT(a, Eq(123));
  EXPECT_THAT(input, Eq(" suffix"));

  ASSERT_FALSE(FindAndConsume(&input, R"((\d+))", &a));
  EXPECT_THAT(input, Eq(" suffix"));

  // Match with Regex object.
  input = "abc 456 def";
  absl::StatusOr<Regex> r = Regex::Compile(R"((\d+))");
  ASSERT_THAT(r, IsOk());
  ASSERT_TRUE(FindAndConsume(&input, *r, &a));
  EXPECT_THAT(a, Eq(456));
  EXPECT_THAT(input, Eq(" def"));
}

TEST(RegexTest, ConsumeN) {
  absl::string_view input = "123 456";
  int a = 0, b = 0;
  Arg arg1(&a);
  Arg arg2(&b);
  const Arg* const args[] = {&arg1, &arg2};
  absl::StatusOr<Regex> r = Regex::Compile(R"((\d+)\s+(\d+))");
  ASSERT_THAT(r, IsOk());
  ASSERT_TRUE(ConsumeN(&input, *r, args, 2));
  EXPECT_THAT(a, Eq(123));
  EXPECT_THAT(b, Eq(456));
  EXPECT_THAT(input, Eq(""));
}

TEST(RegexTest, FindAndConsumeN) {
  absl::string_view input = "prefix 123 456 suffix";
  int a = 0, b = 0;
  Arg arg1(&a);
  Arg arg2(&b);
  const Arg* const args[] = {&arg1, &arg2};
  absl::StatusOr<Regex> r = Regex::Compile(R"((\d+)\s+(\d+))");
  ASSERT_THAT(r, IsOk());
  ASSERT_TRUE(FindAndConsumeN(&input, *r, args, 2));
  EXPECT_THAT(a, Eq(123));
  EXPECT_THAT(b, Eq(456));
  EXPECT_THAT(input, Eq(" suffix"));
}

TEST(RegexTest, BytesEncoding) {
  // In bytes mode we can match arbitrary bytes that are not valid UTF-8.
  // Note that we use a raw string here to make sure that the pattern itself
  // is valid UTF-8 (but it contains an escape sequence to match what would be
  // an invalid UTF-8 byte in the haystack).
  absl::StatusOr<Regex> bytes_regex =
      Regex::Compile(R"(ab\xff)", {.encoding = Encoding::kBytes});
  ASSERT_THAT(bytes_regex, IsOk());
  EXPECT_TRUE(bytes_regex->IsMatch("ab\xff"));

  // In UTF-8 mode, the library never matches invalid UTF-8 bytes.
  absl::StatusOr<Regex> unicode_regex =
      Regex::Compile(R"(ab\xff)", {.encoding = Encoding::kUtf8});
  ASSERT_THAT(unicode_regex, IsOk());
  EXPECT_FALSE(unicode_regex->IsMatch("ab\xff"));
}

TEST(RegexTest, CaseInsensitiveOption) {
  absl::StatusOr<Regex> r = Regex::Compile("abc", {.case_insensitive = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("ABC"));
}

TEST(RegexTest, MultiLineOption) {
  absl::StatusOr<Regex> r = Regex::Compile("^abc$", {.multi_line = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("\nabc"));
}

TEST(RegexTest, VerboseOption) {
  absl::StatusOr<Regex> r = Regex::Compile("a b c", {.verbose = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("abc"));
}

TEST(RegexTest, DotMatchesNewLineOption) {
  absl::StatusOr<Regex> r = Regex::Compile(".", {.dot_matches_new_line = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("\n"));
}

TEST(RegexTest, CrlfOption) {
  absl::StatusOr<Regex> r =
      Regex::Compile("^abc$", {.multi_line = true, .crlf = true});
  ASSERT_THAT(r, IsOk());
  // This only matches in CRLF mode because otherwise there's a `\r` between the
  // "abc" and the end of the line.
  EXPECT_TRUE(r->IsMatch("abc\r\n"));
}

TEST(RegexTest, LineTerminatorOption) {
  absl::StatusOr<Regex> r =
      Regex::Compile("^abc$", {.multi_line = true, .line_terminator = 'z'});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("zabc"));
  EXPECT_TRUE(r->IsMatch("abcz"));
  EXPECT_FALSE(r->IsMatch("\nabc"));

  absl::StatusOr<Regex> dot_r = Regex::Compile(".", {.line_terminator = 'z'});
  ASSERT_THAT(dot_r, IsOk());
  EXPECT_TRUE(dot_r->IsMatch("\n"));
  EXPECT_FALSE(dot_r->IsMatch("z"));
}

TEST(RegexTest, SwapGreedOption) {
  absl::StatusOr<Regex> r = Regex::Compile("a*", {.swap_greed = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_THAT(r->Find("aaa")->AsStr(), Eq(""));
}

TEST(RegexTest, OctalOption) {
  absl::StatusOr<Regex> r = Regex::Compile(R"(\101)", {.octal = true});
  ASSERT_THAT(r, IsOk());
  EXPECT_TRUE(r->IsMatch("A"));
}

TEST(RegexTest, SizeLimitOption) {
  // \w is Unicode aware, this generates a regex that can match approximately
  // 140,000 distinct codepoints. See
  // https://docs.rs/regex/latest/regex/bytes/struct.RegexBuilder.html#method.size_limit
  EXPECT_THAT(Regex::Compile(
                  R"(\w)", {.encoding = Encoding::kUtf8, .size_limit = 45000}),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(RegexTest, NestLimitOption) {
  // According to the regex docs, "ab" requires an explicit concatenation,
  // which results in a nest depth of 1. See
  // https://docs.rs/regex/latest/regex/bytes/struct.RegexBuilder.html#method.nest_limit
  EXPECT_THAT(Regex::Compile("a", {.nest_limit = 0}), IsOk());
  EXPECT_THAT(Regex::Compile("ab", {.nest_limit = 0}),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(RegexTest, AddAnchorsOption) {
  absl::StatusOr<Regex> r =
      Regex::Compile("abc", {.add_begin_and_end_anchors = true});
  ASSERT_THAT(r, IsOk());
  // After adding begin and end anchors, only the exact string should match.
  EXPECT_TRUE(r->IsMatch("abc"));
  EXPECT_FALSE(r->IsMatch("abc "));
  EXPECT_FALSE(r->IsMatch(" abc"));
  EXPECT_FALSE(r->IsMatch(" abc "));

  // Already anchored with ^ and $
  absl::StatusOr<Regex> r2 =
      Regex::Compile("^abc$", {.add_begin_and_end_anchors = true});
  ASSERT_THAT(r2, IsOk());
  EXPECT_TRUE(r2->IsMatch("abc"));
  EXPECT_FALSE(r2->IsMatch("abc "));
  EXPECT_FALSE(r2->IsMatch(" abc"));

  // Already anchored with \A and \z
  absl::StatusOr<Regex> r3 =
      Regex::Compile(R"(\Aabc\z)", {.add_begin_and_end_anchors = true});
  ASSERT_THAT(r3, IsOk());
  EXPECT_TRUE(r3->IsMatch("abc"));
  EXPECT_FALSE(r3->IsMatch("abc "));
  EXPECT_FALSE(r3->IsMatch(" abc"));
}

TEST(RegexTest, VerboseAnchorWithComment) {
  Options options;
  options.verbose = true;
  options.add_begin_and_end_anchors = true;
  absl::StatusOr<Regex> re = Regex::Compile("foo # comment", options);
  ASSERT_THAT(re, IsOk());
  EXPECT_TRUE(re->IsMatch("foo"));
  EXPECT_FALSE(re->IsMatch("foo "));
}

TEST(RegexTest, Utf8LikeRE2CharClasses) {
  // Arabic-Indic digit 3 (\u0663)
  std::string unicode_digit = "\u0663";

  // In standard kUtf8 mode, \d should match Unicode digits.
  absl::StatusOr<Regex> r_utf8 =
      Regex::Compile(R"(\d)", {.encoding = Encoding::kUtf8});
  ASSERT_THAT(r_utf8, IsOk());
  EXPECT_TRUE(r_utf8->IsMatch(unicode_digit));

  // In RE2 compatibility mode, \d should ONLY match ASCII digits.
  absl::StatusOr<Regex> r_re2 = Regex::Compile(
      R"(\d)", {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(r_re2, IsOk());
  EXPECT_FALSE(r_re2->IsMatch(unicode_digit));
  EXPECT_TRUE(r_re2->IsMatch("3"));
}

// Same as the previous test, but with the "add anchors" flag enabled. This way
// we cover all rewriting combinations: re2_compat(on, off) x anchors(on, off).
TEST(RegexTest, Utf8LikeRE2CharClassesPlusAnchors) {
  // Arabic-Indic digit 3 (\u0663)
  std::string unicode_digit = "\u0663";

  // In standard kUtf8 mode, \d should match Unicode digits.
  absl::StatusOr<Regex> r_utf8 = Regex::Compile(
      R"(\d)",
      {.encoding = Encoding::kUtf8, .add_begin_and_end_anchors = true});
  ASSERT_THAT(r_utf8, IsOk());
  EXPECT_TRUE(r_utf8->IsMatch(unicode_digit));
  EXPECT_FALSE(r_utf8->IsMatch(unicode_digit + " "));

  // In RE2 compatibility mode, \d should ONLY match ASCII digits.
  absl::StatusOr<Regex> r_re2 =
      Regex::Compile(R"(\d)", {.encoding = Encoding::kUtf8,
                               .add_begin_and_end_anchors = true,
                               .re2_compatibility = true});
  ASSERT_THAT(r_re2, IsOk());
  EXPECT_FALSE(r_re2->IsMatch(unicode_digit));
  EXPECT_TRUE(r_re2->IsMatch("3"));
  EXPECT_FALSE(r_re2->IsMatch("3 "));
}

TEST(RegexTest, WordBoundaryEncoding) {
  // Greek letter alpha (\u03b1) is a Unicode word character, but not an ASCII
  // word character.
  std::string text = "\u03b1";
  std::string pattern_b = R"(\b\u03b1\b)";
  std::string pattern_B = R"(\B\u03b1\B)";

  // In standard kUtf8 mode, \b uses Unicode word boundaries.
  absl::StatusOr<Regex> r_b_utf8 =
      Regex::Compile(pattern_b, {.encoding = Encoding::kUtf8});
  ASSERT_THAT(r_b_utf8, IsOk());
  EXPECT_TRUE(r_b_utf8->IsMatch(text));
  absl::StatusOr<Regex> r_B_utf8 =
      Regex::Compile(pattern_B, {.encoding = Encoding::kUtf8});
  ASSERT_THAT(r_B_utf8, IsOk());
  EXPECT_FALSE(r_B_utf8->IsMatch(text));

  // In RE2 compatibility mode, word boundaries are ASCII-only. Since \u03b1 is
  // not an ASCII word character, \B matches at its boundaries instead of \b.
  absl::StatusOr<Regex> r_b_re2 = Regex::Compile(
      pattern_b, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(r_b_re2, IsOk());
  EXPECT_FALSE(r_b_re2->IsMatch(text));
  absl::StatusOr<Regex> r_B_re2 = Regex::Compile(
      pattern_B, {.encoding = Encoding::kUtf8, .re2_compatibility = true});
  ASSERT_THAT(r_B_re2, IsOk());
  EXPECT_TRUE(r_B_re2->IsMatch(text));

  // In kBytes mode, word boundaries are also ASCII-only.
  absl::StatusOr<Regex> r_b_bytes =
      Regex::Compile(pattern_b, {.encoding = Encoding::kBytes});
  ASSERT_THAT(r_b_bytes, IsOk());
  EXPECT_FALSE(r_b_bytes->IsMatch(text));
  absl::StatusOr<Regex> r_B_bytes =
      Regex::Compile(pattern_B, {.encoding = Encoding::kBytes});
  ASSERT_THAT(r_B_bytes, IsOk());
  EXPECT_TRUE(r_B_bytes->IsMatch(text));
}

TEST(RegexTest, VerticalTabIsSpace) {
  // By default, a vertical tab is always a space.
  absl::StatusOr<Regex> perl = Regex::Compile("\\s");
  ASSERT_THAT(perl, IsOk());
  absl::StatusOr<Regex> posix = Regex::Compile("[[:space:]]");
  ASSERT_THAT(posix, IsOk());
  EXPECT_TRUE(perl->IsMatch("\v"));
  EXPECT_TRUE(posix->IsMatch("\v"));

  // In compat mode, `[:space:]` includes the vertical tab, but `\s` doesn't.
  absl::StatusOr<Regex> perl_compat =
      Regex::Compile("\\s", {.re2_compatibility = true});
  ASSERT_THAT(perl_compat, IsOk());
  absl::StatusOr<Regex> posix_compat =
      Regex::Compile("[[:space:]]", {.re2_compatibility = true});
  ASSERT_THAT(posix_compat, IsOk());
  EXPECT_FALSE(perl_compat->IsMatch("\v"));
  EXPECT_TRUE(posix_compat->IsMatch("\v"));
}

// RE2 simplifies the non-ASCII dot (.) rune range into byte sequences like
// `[\xe0-\xef][\x80-\xbf]{2}` without validating overlong encodings. Thus, RE2
// matches a 3-byte overlong encoding like `\xe0\x80\x80` as a single dot (.).
// Native Rust regex (`kUtf8`) strictly enforces valid Unicode scalar values and
// rejects overlong sequences.
TEST(Re2DifferencesTest, Utf8OverlongEncodings) {
  absl::string_view pattern = ".";
  absl::string_view text_2b = "\xc0\x80";  // 2-byte overlong encoding of U+0000
  absl::string_view text_3b =
      "\xe0\x80\x80";  // 3-byte overlong encoding of U+0000
  absl::string_view text_4b =
      "\xf0\x80\x80\x80";  // 4-byte overlong encoding of U+0000

  // Native Rust regex strictly validates UTF-8 across all lengths, so it fails
  // to match.
  absl::StatusOr<Regex> regex_native = Regex::Compile(
      pattern,
      {.encoding = Encoding::kUtf8, .add_begin_and_end_anchors = true});
  ASSERT_THAT(regex_native, IsOk());
  EXPECT_FALSE(regex_native->IsMatch(text_2b));
  EXPECT_FALSE(regex_native->IsMatch(text_3b));
  EXPECT_FALSE(regex_native->IsMatch(text_4b));

  // RE2 compatibility mode explicitly rewrites dot (.) to match RE2 exactly
  // across all lengths.
  absl::StatusOr<Regex> regex_re2_like =
      Regex::Compile(pattern, {.encoding = Encoding::kUtf8,
                               .add_begin_and_end_anchors = true,
                               .re2_compatibility = true});
  ASSERT_THAT(regex_re2_like, IsOk());
  // RE2's non-ASCII dot expansion for 2-byte sequences is
  // `[\xc2-\xdf][\x80-\xbf]`. Since `\xc0` is not in `[\xc2-\xdf]`, RE2 treats
  // `\xc0\x80` as two separate invalid bytes. Thus, FullMatch against a single
  // dot (.) returns false.
  EXPECT_FALSE(regex_re2_like->IsMatch(text_2b));
  // RE2's 3-byte and 4-byte expansions (`[\xe0-\xef][\x80-\xbf]{2}` and
  // `[\xf0-\xf4][\x80-\xbf]{3}`) match overlong sequences without validation,
  // so FullMatch returns true.
  EXPECT_TRUE(regex_re2_like->IsMatch(text_3b));
  EXPECT_TRUE(regex_re2_like->IsMatch(text_4b));
}

TEST(RegexSetTest, CompileAndMatch) {
  std::vector<absl::string_view> patterns = {"a", "b", "c"};
  absl::StatusOr<RegexSet> set = RegexSet::Compile(patterns);
  ASSERT_THAT(set, IsOk());
  EXPECT_EQ(set->size(), 3);
  EXPECT_TRUE(set->IsMatch("a"));
  EXPECT_TRUE(set->IsMatch("b"));
  EXPECT_FALSE(set->IsMatch("d"));

  SetMatches matches = set->Matches("a c");
  EXPECT_EQ(matches.size(), 3);
  EXPECT_TRUE(matches.Matched(0));
  EXPECT_FALSE(matches.Matched(1));
  EXPECT_TRUE(matches.Matched(2));

  EXPECT_THAT(matches.MatchedIndices(), ElementsAre(0, 2));
}

TEST(RegexSetTest, Options) {
  std::vector<absl::string_view> patterns = {"a", "b"};
  absl::StatusOr<RegexSet> set =
      RegexSet::Compile(patterns, {.case_insensitive = true});
  ASSERT_THAT(set, IsOk());
  EXPECT_TRUE(set->IsMatch("A"));
  EXPECT_TRUE(set->IsMatch("B"));
}

TEST(RegexSetTest, CompileError) {
  std::vector<absl::string_view> patterns = {"a", "b["};
  EXPECT_THAT(RegexSet::Compile(patterns),
              StatusIs(absl::StatusCode::kInvalidArgument));
}

TEST(RegexSetTest, RE2Match) {
  std::vector<absl::string_view> patterns = {"a", "b", "c"};
  absl::StatusOr<RegexSet> set = RegexSet::Compile(patterns);
  ASSERT_THAT(set, IsOk());

  std::vector<int> v;
  EXPECT_TRUE(set->Match("a c", &v));
  EXPECT_THAT(v, ElementsAre(0, 2));

  EXPECT_FALSE(set->Match("d", &v));
  EXPECT_TRUE(v.empty());

  EXPECT_TRUE(set->Match("b", nullptr));
  EXPECT_FALSE(set->Match("d", nullptr));
}

TEST(RegexTest, Replace) {
  std::string str = "yabba dabba doo";
  EXPECT_TRUE(Replace(&str, "b+", "d"));
  EXPECT_EQ(str, "yada dabba doo");

  // Take a pre-compiled Regex
  absl::StatusOr<Regex> compiled_regex = Regex::Compile("b+");
  ASSERT_THAT(compiled_regex, IsOk());
  EXPECT_TRUE(Replace(&str, *compiled_regex, "d"));
  EXPECT_EQ(str, "yada dada doo");

  // With capture groups
  std::string name_swap = "hello world";
  EXPECT_TRUE(Replace(&name_swap, R"((\w+)\s+(\w+))", "$2 $1"));
  EXPECT_EQ(name_swap, "world hello");

  // With named capture groups
  std::string named_swap = "hello world";
  EXPECT_TRUE(Replace(&named_swap, R"((?P<first>\w+)\s+(?P<second>\w+))",
                      "$second $first"));
  EXPECT_EQ(named_swap, "world hello");

  // No match leaves string unmodified
  std::string no_match = "yabba dabba doo";
  EXPECT_FALSE(Replace(&no_match, "xyz", "foo"));
  EXPECT_EQ(no_match, "yabba dabba doo");

  // Invalid regex returns false and leaves string unmodified
  std::string invalid = "yabba dabba doo";
  EXPECT_FALSE(Replace(&invalid, "b(", "foo"));
  EXPECT_EQ(invalid, "yabba dabba doo");

  // Nullptr returns false
  EXPECT_FALSE(Replace(nullptr, "b+", "foo"));
  EXPECT_FALSE(Replace(nullptr, *compiled_regex, "foo"));

  // Replacing with identical text still returns true
  std::string same = "a";
  EXPECT_TRUE(Replace(&same, "a", "a"));
  EXPECT_EQ(same, "a");

  // Anchors
  std::string start_anchored = "foo";
  EXPECT_TRUE(Replace(&start_anchored, "^", "(START)"));
  EXPECT_EQ(start_anchored, "(START)foo");
}

TEST(RegexTest, GlobalReplace) {
  std::string str = "yabba dabba doo";
  EXPECT_EQ(GlobalReplace(&str, "b+", "d"), 2);
  EXPECT_EQ(str, "yada dada doo");

  // Pre-compiled Regex
  absl::StatusOr<Regex> compiled_regex = Regex::Compile("b");
  ASSERT_THAT(compiled_regex, IsOk());
  std::string repeated_chars_str = "bbbbbb";
  EXPECT_EQ(GlobalReplace(&repeated_chars_str, *compiled_regex, "bb"), 6);
  EXPECT_EQ(repeated_chars_str, "bbbbbbbbbbbb");

  // Non-overlapping replacements
  std::string banana = "banana";
  EXPECT_EQ(GlobalReplace(&banana, "ana", "x"), 1);
  EXPECT_EQ(banana, "bxna");

  // Capture groups
  std::string pairs = "a:1 b:2 c:3";
  EXPECT_EQ(GlobalReplace(&pairs, R"((\w+):(\d+))", "$1=$2"), 3);
  EXPECT_EQ(pairs, "a=1 b=2 c=3");

  // No match
  std::string no_match = "yabba dabba doo";
  EXPECT_EQ(GlobalReplace(&no_match, "xyz", "foo"), 0);
  EXPECT_EQ(no_match, "yabba dabba doo");

  // Invalid regex returns 0 and leaves string unmodified
  std::string invalid = "yabba dabba doo";
  EXPECT_EQ(GlobalReplace(&invalid, "b(", "foo"), 0);
  EXPECT_EQ(invalid, "yabba dabba doo");

  // Nullptr returns 0
  EXPECT_EQ(GlobalReplace(nullptr, "b+", "foo"), 0);
  EXPECT_EQ(GlobalReplace(nullptr, *compiled_regex, "foo"), 0);
}

TEST(RegexTest, RegexMemberReplace) {
  absl::StatusOr<Regex> compiled_regex = Regex::Compile("b+");
  ASSERT_THAT(compiled_regex, IsOk());
  EXPECT_EQ(compiled_regex->Replace("yabba dabba doo", "d"), "yada dabba doo");
  EXPECT_EQ(compiled_regex->ReplaceAll("yabba dabba doo", "d"),
            "yada dada doo");

  absl::StatusOr<Regex> single_regex = Regex::Compile("a");
  ASSERT_THAT(single_regex, IsOk());
  EXPECT_EQ(single_regex->Replacen("aaa", 2, "b"), "bba");
  EXPECT_EQ(single_regex->Replacen("aaa", 0, "b"), "bbb");
}

TEST(RegexTest, BracedHexAndOctalInBytesMode) {
  absl::StatusOr<Regex> r_hex = Regex::Compile(
      R"(\x{99})", {.encoding = Encoding::kBytes, .re2_compatibility = true});
  ASSERT_THAT(r_hex, IsOk());
  EXPECT_TRUE(r_hex->IsMatch("\x99\x99"));

  absl::StatusOr<Regex> r_octal = Regex::Compile(
      R"(\231)",
      {.encoding = Encoding::kBytes, .octal = true, .re2_compatibility = true});
  ASSERT_THAT(r_octal, IsOk());
  EXPECT_TRUE(r_octal->IsMatch("\x99\x99"));

  absl::StatusOr<Regex> r_class =
      Regex::Compile(R"([\x{80}-\x{ff}])",
                     {.encoding = Encoding::kBytes, .re2_compatibility = true});
  ASSERT_THAT(r_class, IsOk());
  EXPECT_TRUE(r_class->IsMatch("\x99"));
}

}  // namespace
}  // namespace security::regex
