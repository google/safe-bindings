#ifndef SECURITY_LEVELDB_RUST_CPP_CMP_H_
#define SECURITY_LEVELDB_RUST_CPP_CMP_H_

#include <cstdint>
#include <string>

#include "absl/types/span.h"
#include "third_party/crubit/support/annotations.h"

namespace leveldb_rs {

// Abstract class for a comparator.
//
// SAFETY: Any implementation of this class MUST be thread-safe
class CRUBIT_UNSAFE_IMPL("Send", "Sync") Comparator {
 public:
  virtual ~Comparator() = default;

  virtual int Compare(absl::Span<const uint8_t> a,
                      absl::Span<const uint8_t> b) const = 0;

  virtual std::string FindShortestSeparator(
      absl::Span<const uint8_t> start,
      absl::Span<const uint8_t> limit) const = 0;

  virtual std::string FindShortSuccessor(
      absl::Span<const uint8_t> key) const = 0;
};

}  // namespace leveldb_rs

#endif  // SECURITY_LEVELDB_RUST_CPP_CMP_H_
