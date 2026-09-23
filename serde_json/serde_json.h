#ifndef SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_
#define SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_

// Single-include header for serde_json C++ bindings.
//
// Includes the Crubit-generated header plus additional C++ templates for
// structured binding support of .items() and .items_mut().

#include <cstddef>
#include <tuple>
#include <type_traits>

#include "crubit/rust.h"  // IWYU pragma: export
#include "support/rs_std/str_ref.h"

namespace std {
template <>
struct tuple_size<::rust::Item>
    : std::integral_constant<std::size_t, 2> {};

template <>
struct tuple_element<0, ::rust::Item> {
  using type = rs_std::StrRef;
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
  using type = rs_std::StrRef;
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
}  // namespace rust

#endif  // SECURITY_JSON_SERDE_JSON_SERDE_JSON_H_
