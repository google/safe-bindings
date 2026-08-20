#include "regex.h"

#include <cstddef>
#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "support/rs_std/slice_ref.h"
#include "support/rs_std/str_ref.h"
#include "regex_internal.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::regex {

namespace {

// The default nest limit used by the Rust regex-syntax crate's ParserBuilder.
constexpr size_t kDefaultNestLimit = 250;

// Escapes non-ASCII bytes into hex. We use this because the Rust regex library,
// even when matching arbitrary bytes, needs the pattern to be a UTF-8 string.
// Note that we don't want to escape any other "special" ASCII characters like
// backslashes because they have meaning to the regex parser, so we can't use
// something like absl::CHexEscape.
std::string EscapeNonAsciiBytes(std::string s) {
  std::string result;
  result.reserve(s.size() * 2);  // rough estimate.
  for (unsigned char c : s) {
    if (c > 127) {
      absl::StrAppendFormat(&result, "\\x%02x", static_cast<unsigned int>(c));
    } else {
      result.push_back(c);
    }
  }
  return result;
}

// Rewrites pattern to implement some of the options in `options`.
//
// While most options are passed directly to the underlying RegexBuilder, some
// of them are implemented via pattern rewriting to compensate for differences
// in supported features between Rust `regex` crate and RE2.
//
// Returns an InvalidArgumentError if the pattern fails to parse, and
// InternalError if the rewritten pattern fails to print, which should be
// rare (e.g. OOM).
absl::StatusOr<std::string> RewriteWithOptions(absl::string_view pattern,
                                               Options options) {
  std::string result(pattern);

  // Even when using regex::bytes, the Rust library expects the pattern to be
  // a `str`, which must be UTF-8 encoded. To avoid invalid byte sequences in
  // the pattern, convert them to escape sequences.
  if (options.encoding == Encoding::kBytes) {
    result = EscapeNonAsciiBytes(result);
  }

  if (options.add_begin_and_end_anchors || options.re2_compatibility) {
    auto rewriter_result = rust::regex_rewrite::Rewriter::new_(
        internal::AsSlice(result), options.verbose, options.octal,
        options.nest_limit.value_or(kDefaultNestLimit));
    if (!rewriter_result.has_value()) {
      return absl::InvalidArgumentError(
          internal::AsStr(rewriter_result.err().message()));
    }
    rust::regex_rewrite::Rewriter rewriter =
        std::move(rewriter_result).value();

    if (options.add_begin_and_end_anchors) {
      rewriter.add_begin_and_end_anchors();
    }
    if (options.re2_compatibility) {
      rewriter.rewrite_for_re2_compat(options.encoding == Encoding::kUtf8);
    }

    auto rewrite_result = std::move(rewriter).finish();
    if (!rewrite_result.has_value()) {
      const rust::VecU8& err = rewrite_result.err();
      return absl::InternalError(internal::AsStr(err));
    }
    const rust::VecU8& val = rewrite_result.value();
    result = internal::AsString(val);
  }
  return result;
}

}  // namespace

std::optional<Match> Captures::Get(absl::string_view name) const {
  // The regex crate takes the capture name as a `str`, even when using
  // regex::bytes (as we do). Therefore we must validate `name` as UTF-8.
  std::optional<rs_std::StrRef> name_ref = rs_std::StrRef::FromUtf8(name);
  if (!name_ref.has_value()) {
    return std::nullopt;
  }
  return internal::MapOptional<Match>(captures_.name(*name_ref));
}

Regex::Regex(rust::Regex inner) : regex_(std::move(inner)) {}

absl::StatusOr<Regex> Regex::Compile(absl::string_view pattern,
                                     Options options) {
  absl::StatusOr<std::string> final_pattern =
      RewriteWithOptions(pattern, options);
  if (!final_pattern.ok()) {
    return final_pattern.status();
  }

  rust::RegexBuilder builder =
      rust::RegexBuilder::new_(internal::AsSlice(*final_pattern));
  builder.unicode(options.encoding == Encoding::kUtf8);
  builder.case_insensitive(options.case_insensitive);
  builder.multi_line(options.multi_line);
  builder.ignore_whitespace(options.verbose);
  builder.dot_matches_new_line(options.dot_matches_new_line);
  builder.crlf(options.crlf);
  builder.line_terminator(options.line_terminator);
  builder.swap_greed(options.swap_greed);
  builder.octal(options.octal);
  if (options.size_limit.has_value()) {
    builder.size_limit(*options.size_limit);
  }
  if (options.dfa_size_limit.has_value()) {
    builder.dfa_size_limit(*options.dfa_size_limit);
  }
  if (options.nest_limit.has_value()) {
    builder.nest_limit(*options.nest_limit);
  }
  auto result = std::move(builder).build();
  if (!result.has_value()) {
    const rust::VecU8& err = result.err();
    return absl::InvalidArgumentError(internal::AsStr(err));
  }
  return Regex(std::move(result).value());
}

bool Regex::IsMatch(absl::string_view text) const {
  return regex_.is_match(internal::AsSlice(text));
}

std::optional<Captures> Regex::FindCaptures(absl::string_view text) const {
  return internal::MapOptional<Captures>(
      regex_.captures(internal::AsSlice(text)));
}

Regex::CaptureNamesResult Regex::CaptureNames() const {
  return CaptureNamesResult(regex_.capture_names());
}

std::optional<Match> Regex::Find(absl::string_view text) const {
  return internal::MapOptional<Match>(regex_.find(internal::AsSlice(text)));
}

Regex::FindAllResult Regex::FindAll(absl::string_view text) const {
  return FindAllResult(regex_.find_iter(internal::AsSlice(text)));
}

Regex::FindAllCapturesResult Regex::FindAllCaptures(
    absl::string_view text) const {
  return FindAllCapturesResult(regex_.captures_iter(internal::AsSlice(text)));
}

Regex::SplitResult Regex::Split(absl::string_view text) const {
  return SplitResult(regex_.split(internal::AsSlice(text)));
}

Regex::SplitNResult Regex::Split(absl::string_view text, size_t limit) const {
  return SplitNResult(regex_.splitn(internal::AsSlice(text), limit));
}

std::map<std::string, int> Regex::NamedCapturingGroups() const {
  std::map<std::string, int> result;
  int i = 0;
  for (const auto& name : CaptureNames()) {
    if (name.has_value()) {
      result.try_emplace(std::string(*name), i);
    }
    ++i;
  }
  return result;
}

std::map<int, std::string> Regex::CapturingGroupNames() const {
  std::map<int, std::string> result;
  int i = 0;
  for (const auto& name : CaptureNames()) {
    if (name.has_value()) {
      result.try_emplace(i, std::string(*name));
    }
    ++i;
  }
  return result;
}

absl::StatusOr<RegexSet> RegexSet::Compile(
    absl::Span<const absl::string_view> patterns, Options options) {
  // Rewrite all patterns as needed.
  std::vector<std::string> rewritten_strings;
  rewritten_strings.reserve(patterns.size());
  for (absl::string_view pattern : patterns) {
    absl::StatusOr<std::string> rewritten =
        RewriteWithOptions(pattern, options);
    if (!rewritten.ok()) {
      return rewritten.status();
    }
    rewritten_strings.push_back(*rewritten);
  }

  // Create a vector of SliceRefs to match what RegexSetBuilder expects.
  std::vector<rs_std::SliceRef<const std::uint8_t>> final_patterns;
  final_patterns.reserve(patterns.size());
  for (const std::string& s : rewritten_strings) {
    final_patterns.push_back(internal::AsSlice(s));
  }

  rust::RegexSetBuilder builder =
      rust::RegexSetBuilder::new_(final_patterns);
  builder.unicode(options.encoding == Encoding::kUtf8);
  builder.case_insensitive(options.case_insensitive);
  builder.multi_line(options.multi_line);
  builder.ignore_whitespace(options.verbose);
  builder.dot_matches_new_line(options.dot_matches_new_line);
  builder.crlf(options.crlf);
  builder.line_terminator(options.line_terminator);
  builder.swap_greed(options.swap_greed);
  builder.octal(options.octal);
  if (options.size_limit.has_value()) {
    builder.size_limit(*options.size_limit);
  }
  if (options.dfa_size_limit.has_value()) {
    builder.dfa_size_limit(*options.dfa_size_limit);
  }
  if (options.nest_limit.has_value()) {
    builder.nest_limit(*options.nest_limit);
  }
  auto result = std::move(builder).build();
  if (!result.has_value()) {
    const rust::VecU8& err = result.err();
    return absl::InvalidArgumentError(internal::AsStr(err));
  }
  return RegexSet(std::move(result).value());
}

}  // namespace security::regex
