#ifndef SECURITY_ISE_MEMORY_SAFETY_CRUBIT_HELPERS_STRING_CONVERSIONS_H_
#define SECURITY_ISE_MEMORY_SAFETY_CRUBIT_HELPERS_STRING_CONVERSIONS_H_

#include <concepts>
#include <cstdint>

#include "absl/base/attributes.h"
#include "absl/strings/string_view.h"

namespace security::crubit_helpers {

namespace internal {
template <typename T>
concept HasAsPtrReturningUint8Ptr = requires(T t) {
  { t.as_ptr() } -> std::same_as<const uint8_t*>;
};
}  // namespace internal

template <typename T>
  requires internal::HasAsPtrReturningUint8Ptr<T>
absl::string_view StringViewFromVecU8(
    const T& vec ABSL_ATTRIBUTE_LIFETIME_BOUND) {
  static_assert(sizeof(const char) == sizeof(const uint8_t),
                "Alignment check failed");
  static_assert(alignof(const char) <= alignof(const uint8_t),
                "We need to keep the pointer to `vec`'s data aligned after the "
                "conversion to char.");
  return absl::string_view(reinterpret_cast<const char*>(vec.as_ptr()),
                           vec.len());
}

}  // namespace security::crubit_helpers

#endif  // SECURITY_ISE_MEMORY_SAFETY_CRUBIT_HELPERS_STRING_CONVERSIONS_H_
