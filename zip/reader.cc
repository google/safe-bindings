#include "reader.h"

#include <utility>

#include "absl/log/check.h"
#include "absl/status/status.h"

namespace security::zip::internal {

namespace {

// Function-local rather than a namespace-scope `thread_local`, so that the
// status is constructed on first use by a thread that actually reads an archive
// instead of on every thread in a binary that merely links this in.
absl::Status& ThreadReaderStatus() {
  static thread_local absl::Status status;
  return status;
}

}  // namespace

void SetReaderStatus(absl::Status status) {
  DCHECK(!status.ok()) << "Only a failure is worth recording";
  ThreadReaderStatus() = std::move(status);
}

void ClearReaderStatus() { ThreadReaderStatus() = absl::OkStatus(); }

absl::Status TakeReaderStatus(absl::Status fallback) {
  absl::Status& recorded = ThreadReaderStatus();
  if (recorded.ok()) {
    return fallback;
  }
  absl::Status status = std::move(recorded);
  recorded = absl::OkStatus();
  return status;
}

}  // namespace security::zip::internal
