#ifndef THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_
#define THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_

// Free-function operators for RoaringBitmap32 and RoaringBitmap64.
//
// These enable natural syntax like `a & b` in C++ without requiring
// std::move on either operand.
//
// Usage:
//   #include "roaring_operators.h"
//
//   roaring_bridge::RoaringBitmap32 a, b;
//   roaring_bridge::RoaringBitmap32 result = a & b;

#include "crubit/roaring_bridge.h"

namespace roaring_bridge {

// RoaringBitmap32 operators

inline RoaringBitmap32 operator&(const RoaringBitmap32& lhs,
                                 const RoaringBitmap32& rhs) {
  return lhs.CrubitInternalIntersect(rhs);
}

inline RoaringBitmap32 operator|(const RoaringBitmap32& lhs,
                                 const RoaringBitmap32& rhs) {
  return lhs.CrubitInternalUnion(rhs);
}

inline RoaringBitmap32 operator^(const RoaringBitmap32& lhs,
                                 const RoaringBitmap32& rhs) {
  return lhs.CrubitInternalSymDiff(rhs);
}

inline RoaringBitmap32 operator-(const RoaringBitmap32& lhs,
                                 const RoaringBitmap32& rhs) {
  return lhs.CrubitInternalDiff(rhs);
}

// RoaringBitmap64 operators

inline RoaringBitmap64 operator&(const RoaringBitmap64& lhs,
                                 const RoaringBitmap64& rhs) {
  return lhs.CrubitInternalIntersect(rhs);
}

inline RoaringBitmap64 operator|(const RoaringBitmap64& lhs,
                                 const RoaringBitmap64& rhs) {
  return lhs.CrubitInternalUnion(rhs);
}

inline RoaringBitmap64 operator^(const RoaringBitmap64& lhs,
                                 const RoaringBitmap64& rhs) {
  return lhs.CrubitInternalSymDiff(rhs);
}

inline RoaringBitmap64 operator-(const RoaringBitmap64& lhs,
                                 const RoaringBitmap64& rhs) {
  return lhs.CrubitInternalDiff(rhs);
}

}  // namespace roaring_bridge

#endif  // THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_OPERATORS_H_
