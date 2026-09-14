#ifndef SECURITY_LEVELDB_RUST_CPP_LOGGER_H_
#define SECURITY_LEVELDB_RUST_CPP_LOGGER_H_

#include "absl/strings/string_view.h"
#include "third_party/crubit/support/annotations.h"

namespace leveldb_rs {

// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") Logger {
 public:
  virtual ~Logger() = default;

  virtual void Log(absl::string_view message) = 0;
};

}  // namespace leveldb_rs

#endif  // SECURITY_LEVELDB_RUST_CPP_LOGGER_H_
