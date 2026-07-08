#ifndef SECURITY_LEVELDB_RUST_CPP_FILTER_H_
#define SECURITY_LEVELDB_RUST_CPP_FILTER_H_

#include <cstdint>
#include <string>

#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "third_party/crubit/support/annotations.h"

namespace leveldb_rs {

// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") FilterPolicy {
 public:
  virtual ~FilterPolicy() = default;

  // Returns the name of this policy.
  // The returned string_view must point to a static string.
  virtual absl::string_view Name() const = 0;

  virtual std::string CreateFilter(
      absl::Span<const uint8_t> keys,
      absl::Span<const uint64_t> offsets) const = 0;

  virtual bool KeyMayMatch(absl::Span<const uint8_t> key,
                           absl::Span<const uint8_t> filter) const = 0;
};

}  // namespace leveldb_rs

#endif  // SECURITY_LEVELDB_RUST_CPP_FILTER_H_
