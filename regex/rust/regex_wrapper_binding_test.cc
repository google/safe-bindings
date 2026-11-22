#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "support/rs_std/slice_ref.h"
#include "support/rs_std/str_ref.h"
#include "support/rs_std/vec.h"
#include "crubit/regex_cpp_bindings.h"

using testing::Eq;
using testing::Optional;

namespace regex_cpp_bindings {
namespace {

rs_std::SliceRef<const std::uint8_t> AsSlice(absl::string_view s) {
  return rs_std::SliceRef<const std::uint8_t>(absl::Span<const std::uint8_t>(
      reinterpret_cast<const std::uint8_t*>(s.data()), s.size()));
}

absl::string_view AsStringView(rs_std::StrRef s) {
  return absl::string_view(s);
}

absl::string_view AsStringView(rs_std::SliceRef<const std::uint8_t> s) {
  if (s.data() == nullptr) return {};
  return absl::string_view(reinterpret_cast<const char*>(s.data()), s.size());
}

absl::string_view AsStringView(const rs_std::Vec<uint8_t>& v) {
  if (v.data() == nullptr) return {};
  return absl::string_view(reinterpret_cast<const char*>(v.data()), v.size());
}

std::optional<absl::string_view> AsStringView(
    std::optional<rs_std::SliceRef<const std::uint8_t>> opt) {
  if (!opt.has_value()) return std::nullopt;
  return AsStringView(*opt);
}

std::optional<std::optional<absl::string_view>> AsStringView(
    std::optional<std::optional<rs_std::SliceRef<const std::uint8_t>>> opt) {
  if (!opt.has_value()) return std::nullopt;
  if (!opt->has_value()) return std::optional<absl::string_view>(std::nullopt);
  return std::optional<absl::string_view>(AsStringView(**opt));
}

TEST(RegexTest, New) {
  // Valid regex, should parse.
  ASSERT_TRUE(Regex::new_(AsSlice(R"(abc(.)efg)")).has_value());
  // Invalid regex, should fail.
  ASSERT_FALSE(Regex::new_(AsSlice(R"(abc[)")).has_value());
}

TEST(RegexTest, AsStr) {
  absl::string_view regex_str = R"(abc(.)efg)";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  ASSERT_THAT(AsStringView(r.as_str()), Eq(regex_str));
}

TEST(RegexTest, IsMatch) {
  absl::string_view regex_str = R"(a(bc)+)";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  ASSERT_TRUE(r.is_match(AsSlice("abcbc")));
  ASSERT_TRUE(r.is_match(AsSlice("there's an abcbcbc somewhere")));
  ASSERT_FALSE(r.is_match(AsSlice("xyz")));
}

TEST(RegexTest, FindReturnsUsableMatch) {
  absl::string_view regex_str = R"(a(bc)+)";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  std::optional<Match> m = r.find(AsSlice("hello abcbc world"));
  ASSERT_TRUE(m.has_value());
  ASSERT_THAT(m->start(), Eq(6));
  ASSERT_THAT(m->end(), Eq(11));
  ASSERT_THAT(m->is_empty(), Eq(false));
  ASSERT_THAT(m->len(), Eq(5));
  ASSERT_THAT(AsStringView(m->as_str()), Eq("abcbc"));
}

TEST(RegexTest, FindIter) {
  absl::string_view regex_str = R"(\b\w{3}\b)";  // Find 3-letter words.
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  Matches matches = r.find_iter(AsSlice("abc aa a aaaaa xyz aa 123"));
  ASSERT_THAT(AsStringView(matches.next()->as_str()), Eq("abc"));
  ASSERT_THAT(AsStringView(matches.next()->as_str()), Eq("xyz"));
  ASSERT_THAT(AsStringView(matches.next()->as_str()), Eq("123"));
  ASSERT_THAT(matches.next(), Eq(std::nullopt));
}

TEST(RegexTest, Captures) {
  absl::string_view regex_str = R"((?<first>[abc]+) (.) (?<third>[xyz]+))";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  std::optional<Captures> c = r.captures(AsSlice("aaaa a xyzzy"));
  ASSERT_TRUE(c.has_value());
  ASSERT_THAT(c->len(), Eq(4));
  ASSERT_TRUE(c->get_match().has_value());
  ASSERT_THAT(AsStringView(c->get_match()->as_str()), Eq("aaaa a xyzzy"));
  // Get by index.
  ASSERT_THAT(AsStringView(c->get(0)->as_str()), Eq("aaaa a xyzzy"));
  ASSERT_THAT(AsStringView(c->get(1)->as_str()), Eq("aaaa"));
  ASSERT_THAT(AsStringView(c->get(2)->as_str()), Eq("a"));
  ASSERT_THAT(AsStringView(c->get(3)->as_str()), Eq("xyzzy"));
  ASSERT_THAT(c->get(4), Eq(std::nullopt));
  // Get by name.
  ASSERT_THAT(AsStringView(c->name("first")->as_str()), Eq("aaaa"));
  ASSERT_THAT(AsStringView(c->name("third")->as_str()), Eq("xyzzy"));
  ASSERT_THAT(c->name("does not exist"), Eq(std::nullopt));
}

TEST(RegexWrapperBindingTest, CapturesExpand) {
  Regex regex = Regex::new_(AsSlice(R"((?P<name>\w+):(?P<num>\d+))")).value();
  std::optional<Captures> captures = regex.captures(AsSlice("johndoe:123"));
  ASSERT_TRUE(captures.has_value());
  EXPECT_EQ(AsStringView(captures->expand(AsSlice("$name=$num"))),
            "johndoe=123");
  EXPECT_EQ(AsStringView(captures->expand(AsSlice("$2:$1"))), "123:johndoe");
}

TEST(RegexTest, CapturesIter) {
  absl::string_view regex_str = R"((?<key>[a-z]+)=(?<value>[0-9]+))";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  CaptureMatches matches = r.captures_iter(AsSlice("aaa=123 bbb=456"));
  Captures first = matches.next().value();
  ASSERT_THAT(AsStringView(first.name("key")->as_str()), Eq("aaa"));
  ASSERT_THAT(AsStringView(first.name("value")->as_str()), Eq("123"));
  Captures second = matches.next().value();
  ASSERT_THAT(AsStringView(second.name("key")->as_str()), Eq("bbb"));
  ASSERT_THAT(AsStringView(second.name("value")->as_str()), Eq("456"));
  ASSERT_THAT(matches.next(), Eq(std::nullopt));
}

TEST(RegexTest, CapturesLen) {
  // captures_len always counts the implicit group for the entire match.
  ASSERT_THAT(Regex::new_(AsSlice("a")).value().captures_len(), Eq(1));
  ASSERT_THAT(Regex::new_(AsSlice("(a)")).value().captures_len(), Eq(2));
  ASSERT_THAT(Regex::new_(AsSlice("(a)(b)")).value().captures_len(), Eq(3));
}

TEST(RegexTest, CaptureNames) {
  Regex r = Regex::new_(AsSlice("(?<x>(?<y>a) (b) (?<z>c)) d")).value();
  CaptureNames names = r.capture_names();
  // The first group is always the unnamed overall match.
  ASSERT_THAT(AsStringView(names.next()), Optional(Eq(std::nullopt)));
  // group with a, b, c
  ASSERT_THAT(AsStringView(names.next()), Optional(Optional(Eq("x"))));
  // a
  ASSERT_THAT(AsStringView(names.next()), Optional(Optional(Eq("y"))));
  // b
  ASSERT_THAT(AsStringView(names.next()), Optional(Eq(std::nullopt)));
  // c
  ASSERT_THAT(AsStringView(names.next()), Optional(Optional(Eq("z"))));
  // end
  ASSERT_THAT(AsStringView(names.next()), Eq(std::nullopt));
}

TEST(RegexTest, Split) {
  Regex r = Regex::new_(AsSlice("[ ]+")).value();
  Split s = r.split(AsSlice("a b    c"));
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("a")));
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("b")));
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("c")));
  ASSERT_THAT(AsStringView(s.next()), Eq(std::nullopt));
}

TEST(RegexTest, SplitN) {
  Regex r = Regex::new_(AsSlice("[ ]+")).value();
  SplitN s = r.splitn(AsSlice("a b    c d e f"), 3);
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("a")));
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("b")));
  ASSERT_THAT(AsStringView(s.next()), Optional(Eq("c d e f")));
  ASSERT_THAT(AsStringView(s.next()), Eq(std::nullopt));
}

TEST(RegexWrapperBindingTest, Replace) {
  auto regex = Regex::new_(AsSlice("a")).value();
  EXPECT_EQ(AsStringView(regex.replace(AsSlice("bbb"), AsSlice("_"))), "bbb");
  EXPECT_EQ(AsStringView(regex.replace(AsSlice("bab"), AsSlice("_"))), "b_b");
}

TEST(RegexWrapperBindingTest, ReplaceAll) {
  auto regex = Regex::new_(AsSlice("a")).value();
  EXPECT_EQ(AsStringView(regex.replace_all(AsSlice("bbb"), AsSlice("_"))),
            "bbb");
  EXPECT_EQ(AsStringView(regex.replace_all(AsSlice("bab"), AsSlice("_"))),
            "b_b");
  EXPECT_EQ(AsStringView(regex.replace_all(AsSlice("aaa"), AsSlice("_"))),
            "___");
}

TEST(RegexWrapperBindingTest, ReplaceN) {
  auto regex = Regex::new_(AsSlice("a")).value();
  auto result_none = regex.replacen(AsSlice("bbb"), 2, AsSlice("_"));
  EXPECT_EQ(result_none.count(), 0);
  EXPECT_EQ(AsStringView(result_none.result()), "bbb");

  auto result_one = regex.replacen(AsSlice("bab"), 2, AsSlice("_"));
  EXPECT_EQ(result_one.count(), 1);
  EXPECT_EQ(AsStringView(result_one.result()), "b_b");

  auto result_two = regex.replacen(AsSlice("aaa"), 2, AsSlice("_"));
  EXPECT_EQ(result_two.count(), 2);
  EXPECT_EQ(AsStringView(result_two.result()), "__a");

  // 0 means replace all.
  auto result_all = regex.replacen(AsSlice("aaa"), 0, AsSlice("_"));
  EXPECT_EQ(result_all.count(), 3);
  EXPECT_EQ(AsStringView(result_all.result()), "___");
}

TEST(RegexWrapperBindingTest, ReplaceWithGroups) {
  auto regex = Regex::new_(AsSlice("(a)(b)")).value();
  EXPECT_EQ(AsStringView(regex.replace(AsSlice("ab"), AsSlice("$2$1"))), "ba");
  EXPECT_EQ(AsStringView(regex.replace_all(AsSlice("ab ab"), AsSlice("$2$1"))),
            "ba ba");
}

TEST(RegexTest, ParseNumbers) {
  absl::string_view regex_str = R"((.*) (.*))";
  Regex r = Regex::new_(AsSlice(regex_str)).value();
  std::optional<Captures> c = r.captures(AsSlice("123 456.789"));
  ASSERT_TRUE(c.has_value());
  // Get by index.
  ASSERT_THAT(c->get(1)->parse_as_u8(0).value(), Eq(123));
  ASSERT_THAT(c->get(2)->parse_as_f32().value(), Eq(456.789f));
}

TEST(RegexWrapperBindingTest, RegexBuilder) {
  auto builder = RegexBuilder::new_(AsSlice("a . b"));
  builder.case_insensitive(true);
  builder.ignore_whitespace(true);
  builder.dot_matches_new_line(true);
  auto r = std::move(builder).build().value();
  ASSERT_TRUE(r.is_match(AsSlice("A\nB")));
}

TEST(RegexWrapperBindingTest, RegexSet) {
  std::vector<rs_std::SliceRef<const std::uint8_t>> patterns = {
      AsSlice("a"), AsSlice("b"), AsSlice("c")};
  rs_std::Result<RegexSet, rs_std::Vec<std::uint8_t>> set_result =
      RegexSet::new_(patterns);
  ASSERT_TRUE(set_result.has_value());
  RegexSet set = std::move(set_result).value();
  EXPECT_EQ(set.len(), 3);
  EXPECT_TRUE(set.is_match(AsSlice("a")));
  EXPECT_TRUE(set.is_match(AsSlice("b")));
  EXPECT_FALSE(set.is_match(AsSlice("d")));

  auto matches = set.matches(AsSlice("a c"));
  EXPECT_EQ(matches.len(), 3);       // total regex count
  EXPECT_TRUE(matches.matched(0));   // "a"
  EXPECT_FALSE(matches.matched(1));  // "b"
  EXPECT_TRUE(matches.matched(2));   // "c"
}

TEST(RegexWrapperBindingTest, RegexSetBuilder) {
  std::vector<rs_std::SliceRef<const std::uint8_t>> patterns = {AsSlice("a"),
                                                                AsSlice("b")};
  auto builder = RegexSetBuilder::new_(patterns);
  builder.case_insensitive(true);
  rs_std::Result<RegexSet, rs_std::Vec<std::uint8_t>> set_result =
      std::move(builder).build();
  RegexSet set = std::move(set_result).value();
  EXPECT_TRUE(set.is_match(AsSlice("A")));
  EXPECT_TRUE(set.is_match(AsSlice("B")));
}

TEST(RegexWrapperBindingTest, RegexBuilderLineTerminator) {
  auto builder = RegexBuilder::new_(AsSlice("."));
  builder.line_terminator('z');
  auto regex = std::move(builder).build().value();
  EXPECT_TRUE(regex.is_match(AsSlice("\n")));
  EXPECT_FALSE(regex.is_match(AsSlice("z")));

  auto builder_multiline = RegexBuilder::new_(AsSlice("^abc$"));
  builder_multiline.multi_line(true);
  builder_multiline.line_terminator('z');
  auto r_multiline = std::move(builder_multiline).build().value();
  EXPECT_TRUE(r_multiline.is_match(AsSlice("zabc")));
  EXPECT_TRUE(r_multiline.is_match(AsSlice("abcz")));
  EXPECT_FALSE(r_multiline.is_match(AsSlice("\nabc")));
}

TEST(RegexWrapperBindingTest, RegexSetBuilderLineTerminator) {
  std::vector<rs_std::SliceRef<const std::uint8_t>> patterns = {AsSlice(".")};
  auto builder = RegexSetBuilder::new_(patterns);
  builder.line_terminator('z');
  rs_std::Result<RegexSet, rs_std::Vec<std::uint8_t>> set_result =
      std::move(builder).build();
  RegexSet set = std::move(set_result).value();
  EXPECT_TRUE(set.is_match(AsSlice("\n")));
  EXPECT_FALSE(set.is_match(AsSlice("z")));

  std::vector<rs_std::SliceRef<const std::uint8_t>> multiline_patterns = {
      AsSlice("^abc$")};
  auto builder_multiline = RegexSetBuilder::new_(multiline_patterns);
  builder_multiline.multi_line(true);
  builder_multiline.line_terminator('z');
  rs_std::Result<RegexSet, rs_std::Vec<std::uint8_t>> set_multiline_result =
      std::move(builder_multiline).build();
  RegexSet set_multiline = std::move(set_multiline_result).value();
  EXPECT_TRUE(set_multiline.is_match(AsSlice("zabc")));
  EXPECT_TRUE(set_multiline.is_match(AsSlice("abcz")));
  EXPECT_FALSE(set_multiline.is_match(AsSlice("\nabc")));
}

}  // namespace
}  // namespace regex_cpp_bindings
