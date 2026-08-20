#ifndef SECURITY_REGEX_REGEX_INTERNAL_H_
#define SECURITY_REGEX_REGEX_INTERNAL_H_

#include <cstddef>
#include <cstdint>
#include <iterator>
#include <memory>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>

#include "support/rs_std/slice_ref.h"
#include "support/rs_std/vec.h"
#include "crubit/rust.h"
#include "absl/log/check.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::regex {

class Match;

namespace internal {

inline rs_std::SliceRef<const std::uint8_t> AsSlice(absl::string_view s) {
  return rs_std::SliceRef<const std::uint8_t>(absl::Span<const std::uint8_t>(
      reinterpret_cast<const std::uint8_t*>(s.data()), s.size()));
}

inline absl::string_view AsStringView(rs_std::SliceRef<const std::uint8_t> s) {
  if (s.data() == nullptr) return {};
  return absl::string_view(reinterpret_cast<const char*>(s.data()), s.size());
}

inline absl::string_view AsStr(const rs_std::Vec<uint8_t>& v) {
  if (v.data() == nullptr) return {};
  return absl::string_view(reinterpret_cast<const char*>(v.data()), v.size());
}

inline std::string AsString(const rs_std::Vec<uint8_t>& v) {
  if (v.data() == nullptr) return {};
  return std::string(reinterpret_cast<const char*>(v.data()), v.size());
}

template <typename Wrapper, typename Inner>
std::optional<Wrapper> MapOptional(std::optional<Inner> opt);

// C++ does not allow partial specialization of function templates. To allow
// specializing MapOptional for std::optional<T>, we delegate to this helper
// struct which can be partially specialized.
//
// Maps an optional<Inner> into an optional<Wrapper> by calling the Wrapper
// constructor on the inner object.
template <typename Wrapper, typename Inner>
struct MapOptionalHelper {
  static std::optional<Wrapper> Map(std::optional<Inner> opt) {
    if (opt.has_value()) {
      return Wrapper(std::move(opt.value()));
    } else {
      return std::nullopt;
    }
  }
};

// Partial specialization for when Wrapper is itself a std::optional.
// This is needed when mapping nested optionals where the inner conversion
// might be explicit (like Match from rust::Match).
template <typename T, typename Inner>
struct MapOptionalHelper<std::optional<T>, Inner> {
  static std::optional<std::optional<T>> Map(std::optional<Inner> opt) {
    if (opt.has_value()) {
      // Recursively call MapOptional for the inner type.
      return MapOptional<T>(std::move(opt.value()));
    } else {
      return std::nullopt;
    }
  }
};

template <>
struct MapOptionalHelper<absl::string_view,
                         rs_std::SliceRef<const std::uint8_t>> {
  static std::optional<absl::string_view> Map(
      std::optional<rs_std::SliceRef<const std::uint8_t>> opt) {
    if (opt.has_value()) {
      return AsStringView(*opt);
    } else {
      return std::nullopt;
    }
  }
};

template <>
struct MapOptionalHelper<std::string, rs_std::Vec<uint8_t>> {
  static std::optional<std::string> Map(
      std::optional<rs_std::Vec<uint8_t>> opt) {
    if (opt.has_value()) {
      return AsString(*opt);
    } else {
      return std::nullopt;
    }
  }
};

template <typename Wrapper, typename Inner>
std::optional<Wrapper> MapOptional(std::optional<Inner> opt) {
  return MapOptionalHelper<Wrapper, Inner>::Map(std::move(opt));
}

template <typename T>
struct is_optional : std::false_type {};
template <typename T>
struct is_optional<std::optional<T>> : std::true_type {};

// Represents a sequence of items produced by a Rust-style iterator, with a
// `next` method that returns `std::optional<ValueType>`. Each element is
// generated on the fly by `next()` so this range can only be iterated once.
template <typename RI, typename WrapperType>
class RustIteratorRange {
 public:
  using value_type = WrapperType;

  explicit RustIteratorRange(RI rust_it)
      : inner_it_(std::make_unique<RI>(std::move(rust_it))), consumed_(false) {}

  // Move only.
  RustIteratorRange(const RustIteratorRange&) = delete;
  RustIteratorRange(RustIteratorRange&&) = default;

  struct Iterator {
    using iterator_concept = std::input_iterator_tag;
    using difference_type = std::ptrdiff_t;
    using value_type = WrapperType;

    RI* ptr;
    std::optional<WrapperType> current;

    Iterator& operator++() {
      current = MapOptional<WrapperType>(ptr->next());
      return *this;
    }

    const value_type& operator*() const { return *current; }

    // Allow comparison with `end()`.
    bool operator==(std::default_sentinel_t) const {
      return !current.has_value();
    }
  };

  auto begin() {
    CHECK(!consumed_) << "Range can only be iterated once.";
    consumed_ = true;
    return Iterator{inner_it_.get(),
                    MapOptional<WrapperType>(inner_it_->next())};
  }
  auto end() const { return std::default_sentinel; }

 private:
  // The Iterator class keeps a pointer to inner_it_, but RustIteratorRange is
  // movable. Adding a level of indirection here keeps the pointer (and
  // iterators) valid if the range itself is moved.
  std::unique_ptr<RI> inner_it_;
  bool consumed_;
};

template <int Radix, typename T>
bool ParseNumeric(const Match& m, T* dest);

}  // namespace internal
}  // namespace security::regex

#endif  // SECURITY_REGEX_REGEX_INTERNAL_H_
