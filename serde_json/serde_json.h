#ifndef SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_
#define SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_

// Single-include header for serde_json C++ bindings.
//
// Includes the Crubit-generated header plus additional C++ templates and
// helpers for structured binding support of .items() and .items_mut(), ArrayOf,
// and safe nested path lookups.

#include <cstddef>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>

#include "crubit/rust.h"  // IWYU pragma: export
#include "support/annotations.h"
#include "support/rs_std/str_ref.h"

namespace std {
template <>
struct tuple_size<::rust::Item>
    : std::integral_constant<std::size_t, 2> {};

template <>
struct tuple_element<0, ::rust::Item> {
  using type = rs::StrRef;
};

template <>
struct tuple_element<1, ::rust::Item> {
  using type = const ::rust::Value*;
};

template <>
struct tuple_size<::rust::ItemMut>
    : std::integral_constant<std::size_t, 2> {};

template <>
struct tuple_element<0, ::rust::ItemMut> {
  using type = rs::StrRef;
};

template <>
struct tuple_element<1, ::rust::ItemMut> {
  using type = ::rust::Value*;
};
}  // namespace std

namespace rust {
template <std::size_t I>
auto get(const Item& item) {
  if constexpr (I == 0)
    return item.key;
  else if constexpr (I == 1)
    return item.value;
}

template <std::size_t I>
auto get(const ItemMut& item) {
  if constexpr (I == 0)
    return item.key;
  else if constexpr (I == 1)
    return item.value;
}

// Returns a JSON array containing `Value(item)` for each element in `range`.
template <typename Range>
Value ArrayOf(const Range& range) {
  Value array = Value::array();
  for (const auto& item : range) {
    // `push_back` only errors if `array` is not an array or null.
    static_cast<void>(array.push_back(Value(item)));
  }
  return array;
}

namespace internal {

template <typename T, typename... Candidates>
inline constexpr bool kIsOneOf = (std::is_same_v<T, Candidates> || ...);

// NOLINTBEGIN(google-runtime-int)
template <typename T>
inline constexpr bool kIsPlainInteger =
    kIsOneOf<std::remove_cvref_t<T>, signed char, int, unsigned char,
             unsigned int,
             short,                // NOLINT(runtime/int)
             long,                 // NOLINT(runtime/int)
             long long,            // NOLINT(runtime/int)
             unsigned short,       // NOLINT(runtime/int)
             unsigned long,        // NOLINT(runtime/int)
             unsigned long long>;  // NOLINT(runtime/int)
// NOLINTEND(google-runtime-int)

template <typename T>
inline constexpr bool kIsStringKey =
    !std::is_same_v<std::remove_cvref_t<T>, std::nullptr_t> &&
    std::is_convertible_v<const T&, std::string_view>;

template <typename T>
inline constexpr bool kIsLookupStep = kIsPlainInteger<T> || kIsStringKey<T>;

template <typename Step>
inline const Value* LookupStep(const Value& current, const Step& step) {
  using PlainStep = std::remove_cvref_t<Step>;
  if constexpr (kIsPlainInteger<PlainStep>) {
    if (!current.is_array() || !std::in_range<std::size_t>(step)) {
      return nullptr;
    }
    const std::size_t index = static_cast<std::size_t>(step);
    if (index >= current.size()) {
      return nullptr;
    }
    return &current[index];
  } else {
    if constexpr (std::is_pointer_v<PlainStep>) {
      if (step == nullptr) {
        return nullptr;
      }
    }
    const std::string_view key = step;
    // NOTE(b/581343769): Switch `find_ptr` to `find` once `find` returns a raw
    // pointer.
    return current.find_ptr(key);
  }
}

}  // namespace internal

// Traverses nested JSON objects (string keys) and arrays (plain integral
// indices) starting from `value`.
//
// Returns a pointer to the target `Value`, or `nullptr` if `value` is
// `nullptr`, any key or index is missing, an index is negative or out of
// bounds, a runtime `const char*` / `char*` key is `nullptr`, or any step
// encounters an incompatible JSON container type.
//
// The returned pointer borrows from `value` and is invalidated if `value` is
// mutated, moved, or destroyed.
template <typename... Steps>
  requires(sizeof...(Steps) > 0 && (internal::kIsLookupStep<Steps> && ...))
[[nodiscard]] inline const Value* LookupPath(
    const Value* value CRUBIT_LIFETIME_BOUND, const Steps&... steps) {
  if (value == nullptr) {
    return nullptr;
  }
  const Value* current = value;
  static_cast<void>(
      (((current = internal::LookupStep(*current, steps)) != nullptr) && ...));
  return current;
}

// Convenience overload of `LookupPath` starting from a `const Value&` root.
template <typename... Steps>
  requires(sizeof...(Steps) > 0 && (internal::kIsLookupStep<Steps> && ...))
[[nodiscard]] inline const Value* LookupPath(
    const Value& value CRUBIT_LIFETIME_BOUND, const Steps&... steps) {
  return LookupPath(&value, steps...);
}

}  // namespace rust

#endif  // SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_
