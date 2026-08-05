// This library provides a C++ interface for regular expression matching,
// powered by the `regex` crate in Rust. It is designed to be a safe and
// efficient alternative to other regex libraries.
//
// Refer to https://docs.rs/regex/latest/regex/#syntax for the regex syntax
// accepted by this library.
//
// -----------------------------------------------------------------------
// BASIC USAGE:
//
// The simplest way to use this library is through the FullMatch and
// PartialMatch functions:
//
//   int i;
//   std::string s;
//   if (FullMatch("ruby:1234", R"((\w+):(\d+))", &s, &i)) {
//     // ... use s and i ...
//   }
//
// FullMatch returns true if the pattern matches the entire text, while
// PartialMatch returns true if the pattern matches any substring.
//
// -----------------------------------------------------------------------
// SUBMATCH EXTRACTION:
//
// You can supply extra pointer arguments to extract capture groups.
//
//   int major, minor;
//   std::string label;
//   if (FullMatch("v1.2-alpha", R"(v(\d+)\.(\d+)(?:-(\w+))?)",
//                                  &major, &minor, &label)) {
//     // ... major=1, minor=2, label="alpha" ...
//   }
//
// You can use nullptr to skip a capture group you are not interested in:
//
//   if (FullMatch("v1.2-alpha", R"(v(\d+)\.(\d+)(?:-(\w+))?)",
//                                  &major, nullptr, &label)) {
//     // ... major=1, label="alpha" ...
//   }
//
// If a capture group is optional and doesn't match, the conversion will fail
// (causing the Match function to return false) unless you use std::optional:
//
//   std::optional<int> patch;
//   FullMatch("v1.2", R"(v(\d+)\.(\d+)(?:\.(\d+))?)",
//                              &major, &minor, &patch);
//   // major=1, minor=2, patch=std::nullopt
//
// If there are more capture groups in the pattern than arguments provided,
// the extra capture groups are ignored. If there are more arguments than
// capture groups, the extra arguments cause the match to fail.
//
// -----------------------------------------------------------------------
// PRE-COMPILED REGULAR EXPRESSIONS:
//
// For better performance when using the same pattern multiple times,
// use the Regex class to compile the pattern once:
//
//   auto re = Regex::Compile(R"(\d+)");
//   if (re.ok()) {
//     if (re->IsMatch("123")) { ... }
//   }
//
// You can also use the matching functions with a pre-compiled Regex:
//
//   PartialMatch("123", *re, &i);
//
// -----------------------------------------------------------------------
// THE MATCH AND CAPTURES OBJECTS:
//
// When using a Regex object directly, you can get detailed information about
// matches:
//
//   auto re = Regex::Compile(R"(\d+)");
//   std::optional<Match> m = re->Find("hello 123 world");
//   if (m) {
//     absl::string_view piece = m->AsStr();  // "123"
//     size_t start = m->Start();             // 6
//   }
//
// For capture groups, use FindCaptures to get a Captures object:
//
//   auto re = Regex::Compile(R"(v(\d+)\.(?P<version>\d+))");
//   std::optional<Captures> groups = re->FindCaptures("v1.2");
//   if (groups) {
//     Match whole = groups->GetMatch();      // Overall match
//     std::optional<Match> m1 = groups->Get(1); // First capture group
//     std::optional<Match> v = groups->Get("version"); // Named capture group
//   }
//
// -----------------------------------------------------------------------
// ITERATING THROUGH MATCHES:
//
// You can find all non-overlapping matches in a string:
//
//   auto re = Regex::Compile(R"(\d+)");
//   for (Match m : re->FindAll("1 2 3 4")) {
//     // Iterate through "1", "2", "3", "4"
//   }
//
// Or find all capture groups for each match:
//
//   auto re = Regex::Compile(R"((\w+)=(\d+))");
//   for (Captures c : re->FindAllCaptures("a=1 b=2")) {
//     // Iterate through {a, 1}, {b, 2}
//   }
//
// You can also split a string using a regex as a delimiter:
//
//   auto re = Regex::Compile(R"(,\s*)");
//   for (absl::string_view piece : re->Split("a, b, c")) {
//     // piece will be "a", "b", "c"
//   }
//
// Note: These ranges use an underlying Rust iterator and can only be
// iterated once.
//
// -----------------------------------------------------------------------
// REPLACEMENT AND EXPANSION:
//
// You can use a Captures object to expand a replacement template:
//
//   auto re = Regex::Compile(R"((\w+) (\w+))");
//   if (auto c = re->FindCaptures("hello world")) {
//     std::string s = c->Expand("$2 $1"); // "world hello"
//   }
//
// -----------------------------------------------------------------------
// NUMERIC PARSING:
//
// Numeric captures are automatically parsed into the provided pointer's type.
// By default, base-10 is used. You can use Hex(), Octal(), or CRadix()
// to specify a different base:
//
//   int hex_val;
//   FullMatch("0x123", "(.*)", Hex(&hex_val));
//
// -----------------------------------------------------------------------

#ifndef SECURITY_REGEX_REGEX_H_
#define SECURITY_REGEX_REGEX_H_

#include <cstdint>
#include <cstdlib>
#include <map>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "regex_internal.h"
#include "crubit/rust.h"
#include "absl/log/check.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::regex {

// Describes a match by a regular expression. It's used for full regex matches
// and also for substring captures.
class Match {
 public:
  // Returns the byte offset of the start of the match in the text.
  size_t Start() const { return match_.start(); }

  // Returns the byte offset of the end of the match in the text. That is, the
  // byte immediately after the last byte in the match.
  size_t End() const { return match_.end(); }

  // Returns true if and only if the match is zero-length.
  bool IsEmpty() const { return match_.is_empty(); }

  // Returns the length in bytes of this match.
  size_t Len() const { return match_.len(); }

  // Returns the substring of the text that matched. The returned string_view is
  // valid for the lifetime of the text.
  absl::string_view AsStr() const {
    return internal::AsStringView(match_.as_str());
  }

  // Access to the inner wrapper. Required by Arg to perform numeric parsing.
  const ::rust::Match& Inner() const { return match_; }

 private:
  friend class Captures;
  friend class Regex;
  template <typename Wrapper, typename Inner>
  friend struct internal::MapOptionalHelper;

  explicit Match(rust::Match inner) : match_(std::move(inner)) {}

  rust::Match match_;
};

// Represents a set of capture groups from a regular expression match.
class Captures {
 public:
  // Returns the overall match for the whole expression.
  Match GetMatch() const { return Match(captures_.get_match()); }

  // Returns the total number of capture groups.
  size_t Len() const { return captures_.len(); }

  // Returns the Match for capture group number `i`. Returns nullopt if `i`
  // doesn't correspond to a capture group, or if that group didn't participate
  // in the match. Equivalent to `get_match` for i=0.
  std::optional<Match> Get(size_t i) const {
    return internal::MapOptional<Match>(captures_.get(i));
  }

  // Returns the Match for capture group named `name`. Returns nullopt if `name`
  // doesn't correspond to a capture group, or if that group didn't participate
  // in the match.
  std::optional<Match> Get(absl::string_view name) const;

  // Returns a new string by expanding placeholders in replacement (e.g., $1,
  // $name) with the values of the capture groups in this match.
  std::string Expand(absl::string_view replacement) const {
    return internal::AsString(captures_.expand(internal::AsSlice(replacement)));
  }

  // A range over capture matches. These ranges can only be iterated once.
  using SubCaptureMatches =
      internal::RustIteratorRange<rust::SubCaptureMatches,
                                  std::optional<Match>>;

  // Returns a range over all capture groups. The first element is always the
  // overall match.
  SubCaptureMatches GetAll() const {
    return SubCaptureMatches(captures_.iter());
  }

 private:
  friend class Regex;
  template <typename Wrapper, typename Inner>
  friend struct internal::MapOptionalHelper;

  explicit Captures(rust::Captures inner)
      : captures_(std::move(inner)) {}

  rust::Captures captures_;
};

// The encoding of the input text. Note that patterns must always be valid
// UTF-8, but you can still match invalid sequences in the text by using
// escape sequences like "\xFF".
enum class Encoding {
  // The input text is UTF-8, but it can contain invalid UTF-8 sequences.
  // Character classes are Unicode-aware (e.g. \d matches Unicode digits like
  // 𝟙𝟚𝟛𝟜).
  //
  // This matches the behavior of `regex::bytes::Regex` in Unicode mode.
  kUtf8,
  // The input text is treated as a sequence of bytes. Useful for matching
  // single-byte encodings like Latin-1.
  kBytes,
};

// See https://docs.rs/regex/latest/regex/bytes/struct.RegexBuilder.html
// for more details about the meaning of the options.
//
// Many of these options can also be configured via flags in the pattern
// string (e.g. `(?i)` for case-insensitive matching).
struct Options {
  Encoding encoding = Encoding::kUtf8;
  // Enables case-insensitive matching.
  bool case_insensitive = false;
  // Enables multi-line mode. In this mode, `^` matches the start of the
  // string and the start of each line, and `$` matches the end of the string
  // and the end of each line.
  bool multi_line = false;
  // Enables verbose mode for the pattern. In this mode, whitespace is ignored
  // and comments starting with `#` are allowed.
  bool verbose = false;
  // Enables dot to match newlines.
  bool dot_matches_new_line = false;
  // Treat both '\r' and '\n' as line terminators.
  bool crlf = false;
  // Sets the line terminator.
  char line_terminator = '\n';
  // Swap the meaning of greedy and non-greedy quantifiers.
  bool swap_greed = false;
  // Enable octal literals in the regex.
  bool octal = false;
  // Size limit for the compiled regex. The default value matches RE2's
  // kDefaultMaxMem, which is supposed to be "something close to Code Search".
  std::optional<size_t> size_limit = 8 << 20;
  // Approximate capacity, in bytes, of the cache of transitions used by the
  // lazy DFA. Setting it to std::nullopt will use the default value from the
  // Rust regex crate.
  std::optional<size_t> dfa_size_limit = std::nullopt;
  // How deep the abstract syntax tree is allowed to be. Setting it to
  // std::nullopt will use the default value from the Rust regex crate.
  std::optional<size_t> nest_limit = std::nullopt;
  // If true, the regular expression will be modified to add begin and end
  // anchors. You can always add the anchors yourself, but this is meant to
  // support use cases where you want to apply FullMatch semantics to a
  // user-provided pattern (e.g. using RegexFlag).
  bool add_begin_and_end_anchors = false;
  // If true, the pattern is rewritten to emulate RE2 behavior as closely as
  // possible (e.g., rewriting Perl character classes like \s and \d to their
  // RE2 equivalents).
  bool re2_compatibility = false;
};

// Represents a compiled regular expression.
class Regex {
 public:
  // Compiles a regular expression. Returns a `Regex` if successful.
  static absl::StatusOr<Regex> Compile(absl::string_view pattern,
                                       Options options = {});

  // Returns true if the regex matches any substring of `text`.
  bool IsMatch(absl::string_view text) const;

  // Returns the compiled pattern string. This might be different from the
  // input pattern due to rewrites for compatibility with RE2 (see
  // `regex_rewrite.rs`).
  absl::string_view AsStr() const { return regex_.as_str(); }

  // Searches for the first match of the regex in `text`, returns an object
  // containing the matches of all captures specified in the regex pattern.
  std::optional<Captures> FindCaptures(absl::string_view text) const;

  // Returns the number of capturing groups. Includes the unnamed overall match.
  size_t NumCaptures() const { return regex_.captures_len(); }

  // A range over capture group names. These ranges can only be iterated once.
  using CaptureNamesResult =
      internal::RustIteratorRange<rust::CaptureNames,
                                  std::optional<absl::string_view>>;

  // Returns a range over the names of all the capture groups. The unnamed
  // overall match is always the first element. Unnamed groups are represented
  // by std::nullopt. The string_view values are valid until the Regex is
  // deleted.
  CaptureNamesResult CaptureNames() const;

  // Returns a map from names to capture indices. The map records the index of
  // the leftmost group with the given name.
  std::map<std::string, int> NamedCapturingGroups() const;

  // Returns a map from capture indices to group names. Unnamed groups don't
  // have entries in the map.
  std::map<int, std::string> CapturingGroupNames() const;

  // Searches for the first match of the regex in `text` and returns a `Match`.
  // If you only want to check for the existence of a match it's potentially
  // faster to use `IsMatch(text)` instead of `Find(text).has_value()`.
  std::optional<Match> Find(absl::string_view text) const;

  // A range over all matches of a regex. These ranges can only be iterated
  // once.
  using FindAllResult =
      internal::RustIteratorRange<rust::Matches, Match>;

  // Returns a range with all matches of the regex in `text`.
  FindAllResult FindAll(absl::string_view text) const;

  // A range over all capture matches of a regex. These ranges can only be
  // iterated once.
  using FindAllCapturesResult =
      internal::RustIteratorRange<rust::CaptureMatches, Captures>;

  // Returns a range with all matches of the regex in `text`.
  FindAllCapturesResult FindAllCaptures(absl::string_view text) const;

  // A range over substrings separated by regex matches. These ranges can only
  // be iterated once.
  using SplitResult =
      internal::RustIteratorRange<rust::Split, absl::string_view>;

  // Splits `text` using the regex as a delimiter, and returns a range of
  // string_views with the parts that DON'T match the regex.
  SplitResult Split(absl::string_view text) const;

  // A range over at most N substrings separated by regex matches. These ranges
  // can only be iterated once.
  using SplitNResult =
      internal::RustIteratorRange<rust::SplitN, absl::string_view>;

  // Splits `text` using the regex as a delimiter, and returns a range of
  // string_views with the parts that DON'T match the regex. It always
  // returns at most `limit` pieces.
  SplitNResult Split(absl::string_view text, size_t limit) const;

 private:
  explicit Regex(rust::Regex inner);

  rust::Regex regex_;
};

// Represents the set of matches returned by RegexSet::Matches.
class SetMatches {
 public:
  // Returns true if the regex at `index` in the set matched.
  bool Matched(size_t index) const { return matches_.matched(index); }

  // Returns the total number of regexes in the set.
  size_t size() const { return matches_.len(); }

  // Returns a vector with the indices of all regexes that matched.
  std::vector<size_t> MatchedIndices() const {
    std::vector<size_t> result;
    for (size_t i = 0; i < size(); ++i) {
      if (Matched(i)) {
        result.push_back(i);
      }
    }
    return result;
  }

 private:
  friend class RegexSet;
  explicit SetMatches(rust::SetMatches inner)
      : matches_(std::move(inner)) {}

  rust::SetMatches matches_;
};

// Represents a compiled set of regular expressions.
class RegexSet {
 public:
  // Compiles a set of regular expressions. Returns a `RegexSet` if successful.
  static absl::StatusOr<RegexSet> Compile(
      absl::Span<const absl::string_view> patterns, Options options = {});

  // Returns true if any of the regular expressions in the set match `text`.
  bool IsMatch(absl::string_view text) const {
    return regex_set_.is_match(internal::AsSlice(text));
  }

  // Returns a `SetMatches` object indicating which regular expressions in the
  // set matched `text`.
  SetMatches Matches(absl::string_view text) const {
    return SetMatches(regex_set_.matches(internal::AsSlice(text)));
  }

  // Returns the number of regular expressions in the set.
  size_t size() const { return regex_set_.len(); }

  // RE2::Set compatibility method.
  // Returns true if text matches at least one of the regexps in the set.
  // If `v` is not null, it clears `v` and fills it with the indices of the
  // matching regexps.
  bool Match(absl::string_view text, std::vector<int>* v) const {
    if (!v) return IsMatch(text);
    SetMatches matches = Matches(text);
    v->clear();
    bool any_match = false;
    for (size_t i = 0; i < matches.size(); ++i) {
      if (matches.Matched(i)) {
        any_match = true;
        v->push_back(static_cast<int>(i));
      }
    }
    return any_match;
  }

 private:
  explicit RegexSet(rust::RegexSet inner)
      : regex_set_(std::move(inner)) {}

  rust::RegexSet regex_set_;
};

// Represents a pointer to a value that will be populated by a regex capture.
// Similar to RE2::Arg, it uses a type-erased parser function. This is necessary
// because the MatchN function supports passing an array of Args computed at
// runtime (for example, if the regular expression is generated on the fly).
// So we can't do everything in a type-safe way using variadic templates.
class Arg {
 public:
  // Function pointer type for parsing a match into a value.
  using Parser = bool (*)(void* ptr, const std::optional<Match>& m);

  Arg() : ptr_(nullptr), parser_(nullptr) {}
  explicit Arg(std::nullptr_t) : ptr_(nullptr), parser_(nullptr) {}

  // Explicit constructor for a pointer and a custom parser.
  Arg(void* ptr, Parser parser) : ptr_(ptr), parser_(parser) {}

  // Template constructor for raw pointers of supported types.
  template <typename T>
  explicit Arg(T* ptr) : ptr_(ptr), parser_(InternalParse<T, 10>) {}

  // Parses the match `m` and stores the result in the pointed-to object.
  // Returns true on success.
  bool Parse(const std::optional<Match>& m) const {
    if (parser_ == nullptr) return true;
    return parser_(ptr_, m);
  }

 private:
  template <typename T, int Radix>
  static bool InternalParse(void* ptr, const std::optional<Match>& match) {
    if constexpr (internal::is_optional<T>::value) {
      T* opt_ptr = static_cast<T*>(ptr);
      if (!match.has_value()) {
        opt_ptr->reset();
        return true;
      }
      // Parse into the inner type of the optional.
      using U = typename T::value_type;
      U val;
      if (InternalParse<U, Radix>(&val, match)) {
        *opt_ptr = std::move(val);
        return true;
      }
      return false;
    }

    T* dest = static_cast<T*>(ptr);

    if (!match.has_value()) {
      // RE2 treats non-matched string captures in a special way: "an optional
      // sub-pattern that does not exist in the matched string is assigned the
      // null string" (http://google3/third_party/re2/re2.h;l=454;rcl=899034878)
      if constexpr (std::is_same_v<T, std::string>) {
        dest->clear();
        return true;
      } else if constexpr (std::is_same_v<T, absl::string_view>) {
        *dest = absl::string_view();
        return true;
      } else {
        return false;
      }
    }

    if constexpr (std::is_same_v<T, std::string>) {
      dest->assign(match->AsStr());
      return true;
    } else if constexpr (std::is_same_v<T, absl::string_view>) {
      *dest = match->AsStr();
      return true;
    } else {
      // For numeric types, we use the internal::ParseNumeric template.
      return internal::ParseNumeric<Radix, T>(*match, dest);
    }
  }

  void* ptr_;
  Parser parser_;

  // Friends for the Hex/Octal/CRadix helpers to access InternalParse.
  template <typename T>
  friend Arg Hex(T* ptr);
  template <typename T>
  friend Arg Octal(T* ptr);
  template <typename T>
  friend Arg CRadix(T* ptr);
};

// Helper function to create an Arg for a pointer to T.
template <typename T>
Arg MakeArg(T* ptr) {
  return Arg(ptr);
}

// Pass-through for already-created Arg objects.
inline Arg MakeArg(Arg arg) { return arg; }

// Helper for nullptr arguments.
inline Arg MakeArg(std::nullptr_t) { return Arg(nullptr); }

// Interprets a numeric capture as a hexadecimal number.
template <typename T>
Arg Hex(T* ptr) {
  if constexpr (std::is_integral_v<T>) {
    return Arg(ptr, Arg::InternalParse<T, 16>);
  } else {
    return Arg(nullptr);
  }
}

// Interprets a numeric capture as a C-style number (0x for hex, 0 for octal).
template <typename T>
Arg CRadix(T* ptr) {
  if constexpr (std::is_integral_v<T>) {
    return Arg(ptr, Arg::InternalParse<T, 0>);
  } else {
    return Arg(nullptr);
  }
}

// Interprets a numeric capture as an octal number.
template <typename T>
Arg Octal(T* ptr) {
  if constexpr (std::is_integral_v<T>) {
    return Arg(ptr, Arg::InternalParse<T, 8>);
  } else {
    return Arg(nullptr);
  }
}

// Matches `r` against `text` and stores captures in the given Args.
inline bool MatchN(absl::string_view text, const Regex& r,
                   const Arg* const args[], int n) {
  if (n == 0) {
    return r.IsMatch(text);
  }
  std::optional<Captures> c = r.FindCaptures(text);
  if (!c.has_value()) {
    return false;
  }
  if (static_cast<size_t>(n) > c->Len() - 1) {
    return false;
  }
  for (int i = 0; i < n; ++i) {
    if (!args[i]->Parse(c->Get(i + 1))) {
      return false;
    }
  }
  return true;
}

// Matches the pattern to any substring in the text using the array interface.
inline bool PartialMatchN(absl::string_view text, const Regex& r,
                          const Arg* const args[], int n) {
  return MatchN(text, r, args, n);
}

// Matches the pattern to any substring in the text (similar to
// RE2::PartialMatch).
// Returns true on a successful partial match, false otherwise.
template <typename... Args>
bool PartialMatch(absl::string_view text, absl::string_view pattern,
                  Args&&... args) {
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  if (!r.ok()) {
    return false;
  }
  return PartialMatch(text, *r, std::forward<Args>(args)...);
}

// Like PartialMatch, but takes a pre-compiled Regex.
template <typename... Args>
bool PartialMatch(absl::string_view text, const Regex& r, Args&&... args) {
  if constexpr (sizeof...(args) == 0) {
    return MatchN(text, r, nullptr, 0);
  } else {
    Arg temp_args[] = {MakeArg(std::forward<Args>(args))...};
    // We need an array of pointers to Arg.
    return []<size_t... Is>(absl::string_view text, const Regex& r,
                            Arg* args_array, std::index_sequence<Is...>) {
      const Arg* const ptrs[] = {&args_array[Is]...};
      return MatchN(text, r, ptrs, sizeof...(Is));
    }(text, r, temp_args, std::index_sequence_for<Args...>{});
  }
}

// Matches the pattern to the entire text (similar to RE2::FullMatch).
// Returns true on a successful full match, false otherwise.
//
// This is a convenience function that only works with patterns passed as
// strings. If you want full match on a pre-compiled regex, use PartialMatch on
// a Regex with anchors (e.g., "\A(?:...)\z").
//
// The reason we don't allow passing a pre-compiled regex is that the underlying
// Rust crate only supports partial search. Adding anchors under the hood would
// require an expensive recompilation step, which defeats the point of passing a
// pre-compiled regex in the first place.
template <typename... Args>
bool FullMatch(absl::string_view text, absl::string_view pattern,
               Args&&... args) {
  // Use \A and \z to anchor to beginning and end of string even in multiline
  // mode, with an non-capturing group to make sure the anchors work properly
  // with alternation (e.g. turning 'abc|xyz' into '\Aabc|xyz\z' is wrong).
  std::string anchored = absl::StrCat("\\A(?:", pattern, ")\\z");
  return PartialMatch(text, anchored, std::forward<Args>(args)...);
}

// Matches the pattern to a prefix of the input string and advances the input
// string view past the match using the array interface.
inline bool ConsumeN(absl::string_view* input, const Regex& r,
                     const Arg* const args[], int n) {
  std::optional<Captures> c = r.FindCaptures(*input);
  // For Consume, we require the match to be at the beginning of the string.
  if (c && c->GetMatch().Start() == 0) {
    if (static_cast<size_t>(n) > c->Len() - 1) return false;
    for (int i = 0; i < n; ++i) {
      if (!args[i]->Parse(c->Get(i + 1))) return false;
    }
    input->remove_prefix(c->GetMatch().End());
    return true;
  }
  return false;
}

// Searches for the first match anywhere in the input string, populates
// arguments, and advances the input string view past the match using the array
// interface.
inline bool FindAndConsumeN(absl::string_view* input, const Regex& r,
                            const Arg* const args[], int n) {
  std::optional<Captures> c = r.FindCaptures(*input);
  if (c) {
    if (static_cast<size_t>(n) > c->Len() - 1) return false;
    for (int i = 0; i < n; ++i) {
      if (!args[i]->Parse(c->Get(i + 1))) return false;
    }
    input->remove_prefix(c->GetMatch().End());
    return true;
  }
  return false;
}

// Matches the pattern to a prefix of the input string and advances the input
// string view past the match (similar to RE2::Consume).
template <typename... Args>
bool Consume(absl::string_view* input, absl::string_view pattern,
             Args&&... args) {
  // Use \A to anchor the match at the beginning of the string. This is an
  // optimization that allows the engine to fail immediately if the first
  // character doesn't match, instead of scanning the rest of the string.
  // Note that if multiple alternations could match at the beginning, the
  // first matching alternation will be picked (leftmost-first matching).
  std::string anchored = absl::StrCat("\\A(?:", pattern, ")");
  absl::StatusOr<Regex> r = Regex::Compile(anchored);
  if (!r.ok()) return false;
  return Consume(input, *r, std::forward<Args>(args)...);
}

// Like Consume, but takes a pre-compiled Regex.
template <typename... Args>
bool Consume(absl::string_view* input, const Regex& r, Args&&... args) {
  Arg temp_args[] = {MakeArg(std::forward<Args>(args))...};
  if constexpr (sizeof...(args) == 0) {
    return ConsumeN(input, r, nullptr, 0);
  } else {
    return []<size_t... Is>(absl::string_view* input, const Regex& r,
                            Arg* args_array, std::index_sequence<Is...>) {
      const Arg* const ptrs[] = {&args_array[Is]...};
      return ConsumeN(input, r, ptrs, sizeof...(Is));
    }(input, r, temp_args, std::index_sequence_for<Args...>{});
  }
}

// Searches for the first match anywhere in the input string, populates
// arguments, and advances the input string view past the match (similar to
// RE2::FindAndConsume).
template <typename... Args>
bool FindAndConsume(absl::string_view* input, absl::string_view pattern,
                    Args&&... args) {
  absl::StatusOr<Regex> r = Regex::Compile(pattern);
  if (!r.ok()) return false;
  return FindAndConsume(input, *r, std::forward<Args>(args)...);
}

// Like FindAndConsume, but takes a pre-compiled Regex.
template <typename... Args>
bool FindAndConsume(absl::string_view* input, const Regex& r, Args&&... args) {
  Arg temp_args[] = {MakeArg(std::forward<Args>(args))...};
  if constexpr (sizeof...(args) == 0) {
    return FindAndConsumeN(input, r, nullptr, 0);
  } else {
    return []<size_t... Is>(absl::string_view* input, const Regex& r,
                            Arg* args_array, std::index_sequence<Is...>) {
      const Arg* const ptrs[] = {&args_array[Is]...};
      return FindAndConsumeN(input, r, ptrs, sizeof...(Is));
    }(input, r, temp_args, std::index_sequence_for<Args...>{});
  }
}

namespace internal {

// Implementation of ParseNumeric. It must be after Match is fully defined.
template <int Radix, typename T>
bool ParseNumeric(const Match& match, T* dest) {
  if constexpr (std::is_same_v<T, int8_t>) {
    auto res = match.Inner().parse_as_i8(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, uint8_t>) {
    auto res = match.Inner().parse_as_u8(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, int16_t>) {
    auto res = match.Inner().parse_as_i16(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, uint16_t>) {
    auto res = match.Inner().parse_as_u16(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, int32_t>) {
    auto res = match.Inner().parse_as_i32(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, uint32_t>) {
    auto res = match.Inner().parse_as_u32(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, int64_t>) {
    auto res = match.Inner().parse_as_i64(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, uint64_t>) {
    auto res = match.Inner().parse_as_u64(Radix);
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, float>) {
    auto res = match.Inner().parse_as_f32();
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  } else if constexpr (std::is_same_v<T, double>) {
    auto res = match.Inner().parse_as_f64();
    if (res.has_value()) {
      *dest = std::move(res).value();
      return true;
    }
  }
  return false;
}

}  // namespace internal

}  // namespace security::regex

#endif  // SECURITY_REGEX_REGEX_H_
