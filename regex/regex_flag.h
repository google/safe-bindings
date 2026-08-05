// This file provides a set of helpers to define flags whose value is a Regex.
// It is heavily based on google3/util/regexp/re2/regexp_flag.h, but it uses
// the //security/regex package rather than RE2. It is otherwise almost
// identical.
//
// RegexFlag
// ---------
//
// The RegexFlag class allows you to define a flag whose value is a Regex
// without having to compile the regular expression every time you use it.
// It uses the default flags for the Regex::Compile function. If you need
// to use different flags, use RegexFlagWithOptions instead.
//
// RegexFlag provides operator*(), operator->() and get() methods that return a
// reference to the compiled Regex, like a smart pointer. Therefore, it must
// outlive the Regex object it points to.
//
// The default value must be set using RegexFlag::OrDie so that an invalid
// pattern cannot go unnoticed.
//
// Example usage:
//
//   ABSL_FLAG(RegexFlag, result_filter_regex,
//             RegexFlag::OrDie("(foo|bar)"),
//             "Only include results that match this regex.");
//
//   bool ShouldIncludeResult(absl::string_view result) {
//     return result_filter_regex->IsMatch(result);
//   }
//
// Remember that flag values can be changed dynamically, so you should never
// keep a reference to the underlying Regex object. In other words:
//
//   const auto& regex = *absl::GetFlag(result_filter_regex);  // BAD!
//
// will immediately leave you with a dangling reference.
//
// A RegexFlag defined in one file can be declared and used in another file.
// You might do this during process initialisation or in test code. For example:
//
//   ABSL_DECLARE_FLAG(RegexFlag, my_flag);
//   ...
//   absl::SetFlag(&my_flag, RegexFlag::OrDie("foo"));
//
// RegexFlagWithOptions
// --------------------
//
// The RegexFlagWithOptions class is similar to RegexFlag but it allows you to
// specify custom options for the Regex::Compile function.
//
// Example usage:
//
//   struct CaseInsensitiveOptions {
//     security::regex::Options operator()() const {
//       return {.case_insensitive = true};
//     }
//   };
//
//   ABSL_FLAG(RegexFlagWithOptions<CaseInsensitiveOptions>,
//             result_filter_regex,
//             RegexFlagWithOptions<CaseInsensitiveOptions>::OrDie("(foo|bar)"),
//             "Only include results that match this regex.");
//
// RegexFlag is a specialization of RegexFlagWithOptions, so the guidance above
// applies here as well.
//
// RegexListFlag
// -------------
//
// The RegexListFlag class is similar to RegexFlag but it holds a list of
// Regex objects, created from a comma-separated list of regular expressions.
//
// The regular expressions in the list can't contain commas. Use the escape
// sequence \x2C to match a comma instead.
//
// The semantics are aligned with the semantics of `std::vector<std::string>`:
//   - an empty flag corresponds to an empty list of regexes, not a list of a
//     single empty regex;
//   - empty regexes are allowed ("a,,b" yields three regexes).
//
// RegexListFlagWithOptions
// ------------------------
//
// Similar to RegexFlagWithOptions, but for a list of regexes. It accepts an
// options template argument and it's used exactly like RegexListFlag.

#ifndef SECURITY_REGEX_REGEX_FLAG_H_
#define SECURITY_REGEX_REGEX_FLAG_H_

#include <cstddef>
#include <initializer_list>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "regex.h"
#include "absl/log/check.h"
#include "absl/strings/match.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/str_join.h"
#include "absl/strings/str_split.h"
#include "absl/strings/string_view.h"

namespace security::regex {

template <typename Options>
class RegexFlagWithOptions {
 public:
  const Regex& operator*() const {
    CHECK(re_ != nullptr)
        << "Dereferencing a RegexFlagWithOptions that has not been "
           "initialized. Ensure the flag has a default value set using "
           "RegexFlagWithOptions::OrDie.";
    return *get();
  }
  const Regex* operator->() const {
    CHECK(re_ != nullptr)
        << "Dereferencing a RegexFlagWithOptions that has not been "
           "initialized. Ensure the flag has a default value set using "
           "RegexFlagWithOptions::OrDie.";
    return get();
  }
  const Regex* get() const { return re_.get(); }
  absl::string_view pattern() const {
    if (re_ == nullptr) return "";
    return re_->AsStr();
  }

  static RegexFlagWithOptions OrDie(absl::string_view pattern) {
    RegexFlagWithOptions flag;
    std::string error;
    CHECK(AbslParseFlag(pattern, &flag, &error)) << error;
    return flag;
  }

 private:
  friend bool AbslParseFlag(absl::string_view pattern,
                            RegexFlagWithOptions* flag, std::string* error) {
    absl::StatusOr<Regex> re = Regex::Compile(pattern, Options()());
    if (!re.ok()) {
      *error = std::string(re.status().message());
      return false;
    }
    flag->re_ = std::make_shared<const Regex>(std::move(*re));
    return true;
  }

  friend std::string AbslUnparseFlag(const RegexFlagWithOptions& flag) {
    if (flag.re_ == nullptr) return "";
    return std::string(flag.re_->AsStr());
  }

  std::shared_ptr<const Regex> re_;
};

template <typename Options>
class RegexListFlagWithOptions {
 public:
  using value_type = RegexFlagWithOptions<Options>;

  static RegexListFlagWithOptions OrDie(
      std::initializer_list<absl::string_view> patterns) {
    RegexListFlagWithOptions flag;
    flag.regexes_.reserve(patterns.size());
    // Enforce the invariant that regexes can't contain commas. Otherwise
    // unparsing the flag would return an incorrect result.
    for (auto pattern : patterns) {
      CHECK(!absl::StrContains(pattern, ','))
          << "RegexListFlag does not support commas in the patterns, escape "
             "the commas using \\x2c: "
          << pattern;
      flag.regexes_.push_back(value_type::OrDie(pattern));
    }
    return flag;
  }

  auto begin() const { return regexes_.begin(); }
  auto end() const { return regexes_.end(); }
  size_t size() const { return regexes_.size(); }
  bool empty() const { return regexes_.empty(); }

 private:
  friend bool AbslParseFlag(absl::string_view text,
                            RegexListFlagWithOptions* flag,
                            std::string* error) {
    std::vector<value_type> regexes;
    if (!text.empty()) {
      for (const auto part : absl::StrSplit(text, ',', absl::AllowEmpty())) {
        if (!AbslParseFlag(part, &regexes.emplace_back(), error)) {
          return false;
        }
      }
    }
    flag->regexes_ = std::move(regexes);
    return true;
  }

  friend std::string AbslUnparseFlag(const RegexListFlagWithOptions& flag) {
    return absl::StrJoin(flag.regexes_, ",",
                         [](std::string* out, const auto& re) {
                           absl::StrAppend(out, re.pattern());
                         });
  }

 private:
  std::vector<value_type> regexes_;
};

namespace regex_flag_internal {

struct DefaultOptions {
  Options operator()() const {
    Options options;
    return options;
  }
};

}  // namespace regex_flag_internal

using RegexFlag = RegexFlagWithOptions<regex_flag_internal::DefaultOptions>;
using RegexListFlag =
    RegexListFlagWithOptions<regex_flag_internal::DefaultOptions>;

}  // namespace security::regex

#endif  // SECURITY_REGEX_REGEX_FLAG_H_
