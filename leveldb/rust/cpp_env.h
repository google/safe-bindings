#ifndef SECURITY_LEVELDB_RUST_CPP_ENV_H_
#define SECURITY_LEVELDB_RUST_CPP_ENV_H_

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "rust/cpp_logger.h"
#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/time/clock.h"
#include "absl/time/time.h"
#include "absl/types/span.h"
#include "third_party/crubit/support/annotations.h"

namespace leveldb_rs {

class ChildNameList final {
 public:
  explicit ChildNameList(std::vector<std::string> names)
      : names_(std::move(names)) {}
  static int get_count(const ChildNameList* list) {
    CHECK(list != nullptr);
    return static_cast<int>(list->names_.size());
  }
  static std::string get_item(const ChildNameList* list, int i) {
    CHECK(list != nullptr);
    CHECK_GE(i, 0);

    CHECK_LT(i, get_count(list));
    return list->names_[i];
  }

 private:
  std::vector<std::string> names_;
};

// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") SequentialFile {
 public:
  virtual ~SequentialFile() = default;
  virtual absl::StatusOr<size_t> Read(absl::Span<uint8_t> dst) = 0;
  virtual absl::Status Skip(uint64_t n) = 0;
};

// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") RandomAccessFile {
 public:
  virtual ~RandomAccessFile() = default;
  virtual absl::StatusOr<size_t> Read(uint64_t offset,
                                      absl::Span<uint8_t> dst) const = 0;
};

// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") WritableFile {
 public:
  virtual ~WritableFile() = default;
  virtual absl::Status Append(absl::Span<const uint8_t> data) = 0;
  virtual absl::Status Close() = 0;
  virtual absl::Status Flush() = 0;
  virtual absl::Status Sync() = 0;
};

// Interface for C++ environment operations (like file system access) used
// by the Rust LevelDB implementation. Methods return UnimplementedError by
// default, signaling the Rust implementation to use its native fallback.
// SAFETY: Any implementation of this class MUST be thread-safe.
class CRUBIT_UNSAFE_IMPL("Send", "Sync") Env {
 public:
  virtual ~Env() = default;

  virtual absl::StatusOr<std::unique_ptr<SequentialFile>> NewSequentialFile(
      absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::StatusOr<std::unique_ptr<RandomAccessFile>> NewRandomAccessFile(
      absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::StatusOr<std::unique_ptr<WritableFile>> NewWritableFile(
      absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::StatusOr<std::unique_ptr<WritableFile>> NewAppendableFile(
      absl::string_view path) const {
    return absl::UnimplementedError("");
  }

  virtual absl::StatusOr<bool> FileExists(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::StatusOr<std::vector<std::string>> GetChildren(
      absl::string_view path) const {
    return absl::UnimplementedError("");
  }

  absl::StatusOr<std::unique_ptr<ChildNameList>> ListChildren(
      absl::string_view path) const {
    auto res = GetChildren(path);
    if (!res.ok()) return res.status();
    return std::make_unique<ChildNameList>(std::move(res).value());
  }

  virtual absl::StatusOr<size_t> GetFileSize(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::Status DeleteFile(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::Status CreateDir(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::Status DeleteDir(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::Status RenameFile(absl::string_view old_path,
                                  absl::string_view new_path) const {
    return absl::UnimplementedError("");
  }
  virtual uint64_t NowMicros() const { return absl::ToUnixMicros(absl::Now()); }
  virtual void SleepForMicroseconds(uint32_t micros) const {
    absl::SleepFor(absl::Microseconds(micros));
  }
  virtual absl::StatusOr<std::string> LockFile(absl::string_view path) const {
    return absl::UnimplementedError("");
  }
  virtual absl::Status UnlockFile(absl::string_view lock_id) const {
    return absl::UnimplementedError("");
  }

  virtual absl::StatusOr<std::unique_ptr<Logger>> NewLogger(
      absl::string_view fname) const {
    return absl::UnimplementedError("");
  }
};

}  // namespace leveldb_rs

#endif  // SECURITY_LEVELDB_RUST_CPP_ENV_H_
