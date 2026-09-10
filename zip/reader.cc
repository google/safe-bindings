#include "reader.h"

#include <utility>

#include "absl/log/check.h"
#include "absl/status/status.h"
#include "absl/strings/cord.h"
#include "absl/strings/string_view.h"

namespace security::zip::internal {

namespace {

// Function-local rather than a namespace-scope `thread_local`, so that the
// status is constructed on first use by a thread that actually reads an archive
// instead of on every thread in a binary that merely links this in.
absl::Status& ThreadReaderStatus() {
  static thread_local absl::Status status;
  return status;
}

// Payload type URL that marks a status as the direct result of a failed reader
// callback. The payload carries no data; its presence is the marker.
constexpr absl::string_view kReaderCallbackFailureUrl =
    "type.googleapis.com/security.zip.ReaderCallbackFailure";

}  // namespace

void SetReaderStatus(absl::Status status) {
  DCHECK(!status.ok()) << "Only a failure is worth recording";
  ThreadReaderStatus() = std::move(status);
}

void ClearReaderStatus() { ThreadReaderStatus() = absl::OkStatus(); }

absl::Status MarkReaderCallbackFailure(absl::Status status) {
  DCHECK(!status.ok()) << "Only a failure can come from a failed callback";
  status.SetPayload(kReaderCallbackFailureUrl, absl::Cord());
  return status;
}

bool IsReaderCallbackFailure(const absl::Status& status) {
  return status.GetPayload(kReaderCallbackFailureUrl).has_value();
}

absl::Status ResolveReaderStatus(absl::Status status) {
  absl::Status& recorded = ThreadReaderStatus();
  absl::Status recorded_status = std::move(recorded);
  recorded = absl::OkStatus();
  if (!IsReaderCallbackFailure(status)) {
    return status;
  }
  if (recorded_status.ok()) {
    // A callback failed without recording why; all that is left to report is
    // the bridge's own error.
    status.ErasePayload(kReaderCallbackFailureUrl);
    return status;
  }
  return recorded_status;
}

}  // namespace security::zip::internal
