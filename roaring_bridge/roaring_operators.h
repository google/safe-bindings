#ifndef THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_
#define THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_

// Free-function operators for RoaringBitmap32 and RoaringBitmap64.
//
// These enable natural syntax like `a & b` in C++ without requiring
// std::move on either operand. The lhs is taken by value (copied),
// modified in-place, and returned.
//
// Usage:
//   #include "roaring_operators.h"
//
//   roaring_bridge::RoaringBitmap32 a, b;
//   roaring_bridge::RoaringBitmap32 result = a & b;

#include "crubit/roaring_bridge.h"

namespace roaring_bridge {

// RoaringBitmap32 operators

inline RoaringBitmap32 operator&(RoaringBitmap32 lhs,
                                 const RoaringBitmap32& rhs) {
  lhs &= rhs;
  return lhs;
}

inline RoaringBitmap32 operator|(RoaringBitmap32 lhs,
                                 const RoaringBitmap32& rhs) {
  lhs |= rhs;
  return lhs;
}

inline RoaringBitmap32 operator^(RoaringBitmap32 lhs,
                                 const RoaringBitmap32& rhs) {
  lhs ^= rhs;
  return lhs;
}

inline RoaringBitmap32 operator-(RoaringBitmap32 lhs,
                                 const RoaringBitmap32& rhs) {
  lhs -= rhs;
  return lhs;
}

// RoaringBitmap64

inline RoaringBitmap64 operator&(RoaringBitmap64 lhs,
                                 const RoaringBitmap64& rhs) {
  lhs &= rhs;
  return lhs;
}

inline RoaringBitmap64 operator|(RoaringBitmap64 lhs,
                                 const RoaringBitmap64& rhs) {
  lhs |= rhs;
  return lhs;
}

inline RoaringBitmap64 operator^(RoaringBitmap64 lhs,
                                 const RoaringBitmap64& rhs) {
  lhs ^= rhs;
  return lhs;
}

inline RoaringBitmap64 operator-(RoaringBitmap64 lhs,
                                 const RoaringBitmap64& rhs) {
  lhs -= rhs;
  return lhs;
}

}  // namespace roaring_bridge

#endif  // THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_
