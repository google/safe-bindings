#ifndef THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_H_
#define THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_H_

// Single-include header for roaring-rs C++ bindings.
//
// Includes the Crubit-generated bindings and free-function operators
// that enable natural syntax: `a & b`, `a | b`, `a - b`, `a ^ b`.
//
// Usage:
//   #include "roaring.h"
//
//   roaring_bridge::RoaringBitmap32 a, b;
//   roaring_bridge::RoaringBitmap32 result = a & b;

// IWYU pragma: begin_exports
#include "crubit/roaring_bridge.h"
#include "roaring_operators.h"
// IWYU pragma: end_exports

#endif  // THIRD_PARTY_SAFE_BINDINGS_ROARING_BRIDGE_ROARING_H_
