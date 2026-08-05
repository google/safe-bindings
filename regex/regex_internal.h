#ifndef SECURITY_REGEX_REGEX_INTERNAL_H_
#define SECURITY_REGEX_REGEX_INTERNAL_H_

#include <cstddef>
#include <iterator>
#include <optional>
#include <type_traits>
#include <utility>

#include "absl/log/check.h"

namespace security::regex {

class Match;

namespace internal {

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
