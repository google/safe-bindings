#include "regex_flag.h"

#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/flags/commandlineflag.h"
#include "absl/flags/flag.h"
#include "absl/flags/reflection.h"
#include "absl/strings/string_view.h"
#include "regex.h"

ABSL_FLAG(security::regex::RegexFlag, empty_flag,
          security::regex::RegexFlag::OrDie(""),
          "a flag whose value is the empty string");
ABSL_FLAG(security::regex::RegexFlag, nonempty_flag,
          security::regex::RegexFlag::OrDie("a+"),
          "a flag whose value is not the empty string");
ABSL_FLAG(security::regex::RegexFlag, uninitialized_flag, {},
          "a flag whose value is uninitialized");
ABSL_FLAG(std::optional<security::regex::RegexFlag>, optional_flag,
          std::nullopt, "a flag whose value is optional");

ABSL_FLAG(security::regex::RegexListFlag, empty_list_flag, {},
          "empty regexp list");
ABSL_FLAG(security::regex::RegexListFlag, list_flag,
          security::regex::RegexListFlag::OrDie({"a+", "b+"}),
          "list with 2 regexps");

namespace {
using testing::ElementsAre;
using testing::IsEmpty;
using testing::Property;

template <typename T>
concept CanDereference = requires(T f) { *std::forward<T>(f); };
template <typename T>
concept CanArrow = requires(T f) {
  std::forward<T>(f).operator->();
  std::forward<T>(f)->IsMatch("");
};
template <typename T>
concept CanCallGet = requires(T f) { std::forward<T>(f).get(); };
template <typename T>
concept CanCallPattern = requires(T f) { std::forward<T>(f).pattern(); };
template <typename T>
concept CanCallBegin = requires(T f) { std::forward<T>(f).begin(); };
template <typename T>
concept CanCallEnd = requires(T f) { std::forward<T>(f).end(); };

// Unlike Regex::AsStr(), the RegexFlag and RegexListFlag accessors deliberately
// keep their rvalue overloads so that temporary copies returned by
// absl::GetFlag() can be used within a single full-expression.
static_assert(CanDereference<const security::regex::RegexFlag&>);
static_assert(CanDereference<security::regex::RegexFlag&>);
static_assert(CanDereference<security::regex::RegexFlag&&>);
static_assert(CanDereference<const security::regex::RegexFlag&&>);

static_assert(CanArrow<const security::regex::RegexFlag&>);
static_assert(CanArrow<security::regex::RegexFlag&>);
static_assert(CanArrow<security::regex::RegexFlag&&>);
static_assert(CanArrow<const security::regex::RegexFlag&&>);

static_assert(CanCallGet<const security::regex::RegexFlag&>);
static_assert(CanCallGet<security::regex::RegexFlag&>);
static_assert(CanCallGet<security::regex::RegexFlag&&>);
static_assert(CanCallGet<const security::regex::RegexFlag&&>);

static_assert(CanCallPattern<const security::regex::RegexFlag&>);
static_assert(CanCallPattern<security::regex::RegexFlag&>);
static_assert(CanCallPattern<security::regex::RegexFlag&&>);
static_assert(CanCallPattern<const security::regex::RegexFlag&&>);

static_assert(CanCallBegin<const security::regex::RegexListFlag&>);
static_assert(CanCallBegin<security::regex::RegexListFlag&>);
static_assert(CanCallBegin<security::regex::RegexListFlag&&>);
static_assert(CanCallBegin<const security::regex::RegexListFlag&&>);

static_assert(CanCallEnd<const security::regex::RegexListFlag&>);
static_assert(CanCallEnd<security::regex::RegexListFlag&>);
static_assert(CanCallEnd<security::regex::RegexListFlag&&>);
static_assert(CanCallEnd<const security::regex::RegexListFlag&&>);

bool SetFlag(absl::string_view name, absl::string_view value) {
  absl::CommandLineFlag* flag = absl::FindCommandLineFlag(name);
  if (flag == nullptr) {
    return false;
  }
  std::string error;
  return flag->ParseFrom(value, &error);
}

bool GetFlagValue(absl::string_view name, std::string* value) {
  absl::CommandLineFlag* flag = absl::FindCommandLineFlag(name);
  if (flag == nullptr) {
    return false;
  }
  *value = flag->CurrentValue();
  return true;
}

testing::Matcher<security::regex::RegexFlag> HasPattern(
    absl::string_view pattern) {
  return Property(&security::regex::RegexFlag::pattern, pattern);
}

TEST(RegexFlagTest, OperatorsAndGet) {
  auto empty_flag = absl::GetFlag(FLAGS_empty_flag);
  EXPECT_EQ("", empty_flag.pattern());

  auto nonempty_flag = absl::GetFlag(FLAGS_nonempty_flag);
  EXPECT_EQ("a+", nonempty_flag.pattern());
  EXPECT_EQ((*nonempty_flag).AsStr(), "a+");
  EXPECT_TRUE((*nonempty_flag).IsMatch("aaa"));
  EXPECT_TRUE(nonempty_flag->IsMatch("aaa"));
  EXPECT_FALSE(nonempty_flag->IsMatch("bbb"));

  // Consuming a temporary RegexFlag from absl::GetFlag() within the
  // full-expression remains valid via ->, *, .get(), and .pattern().
  EXPECT_TRUE(absl::GetFlag(FLAGS_nonempty_flag)->IsMatch("aaa"));
  EXPECT_EQ(std::string(absl::GetFlag(FLAGS_nonempty_flag)->AsStr()), "a+");
  EXPECT_TRUE((*absl::GetFlag(FLAGS_nonempty_flag)).IsMatch("aaa"));
  EXPECT_EQ(std::string((*absl::GetFlag(FLAGS_nonempty_flag)).AsStr()), "a+");
  EXPECT_TRUE(absl::GetFlag(FLAGS_nonempty_flag).get()->IsMatch("aaa"));
  EXPECT_EQ(std::string(absl::GetFlag(FLAGS_nonempty_flag).pattern()), "a+");

  auto empty_copy = absl::GetFlag(FLAGS_empty_flag);
  EXPECT_EQ(empty_flag.get(), empty_copy.get());

  auto nonempty_copy = absl::GetFlag(FLAGS_nonempty_flag);
  EXPECT_EQ(nonempty_flag.get(), nonempty_copy.get());

  auto uninitialized_flag = absl::GetFlag(FLAGS_uninitialized_flag);
  EXPECT_EQ(uninitialized_flag.get(), nullptr);
  EXPECT_EQ(uninitialized_flag.pattern(), "");
  EXPECT_EQ(std::string(absl::GetFlag(FLAGS_uninitialized_flag).pattern()), "");
  EXPECT_DEATH(*uninitialized_flag,
               "Dereferencing a RegexFlagWithOptions that has not been "
               "initialized");
  EXPECT_DEATH(uninitialized_flag->IsMatch("a"),
               "Dereferencing a RegexFlagWithOptions that has not been "
               "initialized");
  EXPECT_DEATH(*absl::GetFlag(FLAGS_uninitialized_flag),
               "Dereferencing a RegexFlagWithOptions that has not been "
               "initialized");
  EXPECT_DEATH(absl::GetFlag(FLAGS_uninitialized_flag)->IsMatch("a"),
               "Dereferencing a RegexFlagWithOptions that has not been "
               "initialized");

  auto optional_flag = absl::GetFlag(FLAGS_optional_flag);
  EXPECT_EQ(optional_flag, std::nullopt);
}

TEST(RegexFlagTest, OrDie_InvalidRegexp) {
  EXPECT_DEATH(security::regex::RegexFlag::OrDie("(a+"),
               "Check failed: AbslParseFlag\\(");
}

TEST(RegexFlagTest, ParseFlag_ValidRegexp) {
  absl::FlagSaver flag_saver;
  EXPECT_TRUE(SetFlag("nonempty_flag", "b+"));
  EXPECT_THAT(absl::GetFlag(FLAGS_nonempty_flag), HasPattern("b+"));
}

TEST(RegexFlagTest, ParseFlag_InvalidRegexp) {
  absl::FlagSaver flag_saver;
  EXPECT_FALSE(SetFlag("nonempty_flag", "(b+"));
  EXPECT_THAT(absl::GetFlag(FLAGS_nonempty_flag), HasPattern("a+"));
}

TEST(RegexFlagTest, UnparseFlag) {
  std::string pattern;
  EXPECT_TRUE(GetFlagValue("nonempty_flag", &pattern));
  EXPECT_EQ("a+", pattern);
  EXPECT_TRUE(GetFlagValue("uninitialized_flag", &pattern));
  EXPECT_EQ("", pattern);
}

TEST(RegexListFlagTest, GetFlag) {
  EXPECT_THAT(absl::GetFlag(FLAGS_empty_list_flag), ElementsAre());
  EXPECT_THAT(absl::GetFlag(FLAGS_list_flag),
              ElementsAre(HasPattern("a+"), HasPattern("b+")));

  std::vector<std::string> empty_patterns;
  for (const auto& re : absl::GetFlag(FLAGS_empty_list_flag)) {
    empty_patterns.push_back(std::string(re.pattern()));
  }
  EXPECT_THAT(empty_patterns, IsEmpty());

  std::vector<std::string> patterns;
  for (const auto& re : absl::GetFlag(FLAGS_list_flag)) {
    EXPECT_EQ((*re).AsStr(), re.pattern());
    EXPECT_TRUE(re->IsMatch(re.pattern()));
    patterns.push_back(std::string(re.pattern()));
  }
  EXPECT_THAT(patterns, ElementsAre("a+", "b+"));
}

TEST(RegexListFlagTest, OrDie_Comma) {
  EXPECT_DEATH(security::regex::RegexListFlag::OrDie({"a,b"}),
               "commas in the patterns");
}

TEST(RegexListFlagTest, OrDie_InvalidRegexp) {
  EXPECT_DEATH(security::regex::RegexListFlag::OrDie({"(a+"}),
               "Check failed: AbslParseFlag\\(");
}

TEST(RegexListFlagTest, ParseFlag_Empty) {
  absl::FlagSaver flag_saver;
  EXPECT_TRUE(SetFlag("list_flag", ""));
  EXPECT_THAT(absl::GetFlag(FLAGS_list_flag), IsEmpty());
}

TEST(RegexListFlagTest, ParseFlag_Comma) {
  absl::FlagSaver flag_saver;
  EXPECT_TRUE(SetFlag("list_flag", "a+,b+"));
  EXPECT_THAT(absl::GetFlag(FLAGS_list_flag),
              ElementsAre(HasPattern("a+"), HasPattern("b+")));
}

TEST(RegexListFlagTest, ParseFlag_EscapedComma) {
  absl::FlagSaver flag_saver;
  EXPECT_TRUE(SetFlag("list_flag", R"(a+\x2cb+)"));
  EXPECT_THAT(absl::GetFlag(FLAGS_list_flag),
              ElementsAre(HasPattern(R"(a+\x2cb+)")));
}

TEST(RegexListFlagTest, ParseFlag_InvalidRegexp) {
  absl::FlagSaver flag_saver;
  EXPECT_FALSE(SetFlag("list_flag", "a+,(b+,c+"));
  EXPECT_THAT(absl::GetFlag(FLAGS_list_flag),
              ElementsAre(HasPattern("a+"), HasPattern("b+")));
}

TEST(RegexListFlagTest, UnparseFlag) {
  std::string pattern_list;
  EXPECT_TRUE(GetFlagValue("empty_list_flag", &pattern_list));
  EXPECT_EQ(pattern_list, "");
  EXPECT_TRUE(GetFlagValue("list_flag", &pattern_list));
  EXPECT_EQ(pattern_list, "a+,b+");
}

}  // namespace

struct CaseInsensitiveOptions {
  security::regex::Options operator()() const {
    security::regex::Options options;
    options.case_insensitive = true;
    return options;
  }
};

ABSL_FLAG(
    security::regex::RegexFlagWithOptions<CaseInsensitiveOptions>,
    case_insensitive_flag,
    security::regex::RegexFlagWithOptions<CaseInsensitiveOptions>::OrDie("a+"),
    "a flag with case insensitive option");

namespace {

TEST(RegexFlagWithOptionsTest, CaseInsensitive) {
  auto flag = absl::GetFlag(FLAGS_case_insensitive_flag);
  EXPECT_TRUE(flag->IsMatch("A"));
  EXPECT_TRUE(flag->IsMatch("a"));
}

}  // namespace
