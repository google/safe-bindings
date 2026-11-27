#ifndef SAFE_BINDINGS_ZIP_READER_STATUS_H_
#define SAFE_BINDINGS_ZIP_READER_STATUS_H_

#include <type_traits>
#include <utility>

#include "absl/status/status.h"

namespace security::zip::internal {

// Implementation detail of this library; not for use elsewhere.
//
// Makes an archive operation return the status its RandomAccessReader failed
// with. Rust calls the reader through callbacks that can only signal failure,
// so a failing ReadAt() or Size() would otherwise reach the caller as a
// generic kInternal error. Instead the callback records the reader's status
// (SetReaderStatus), and the operation that called into Rust returns it in
// place of the generic error (ThroughReader).
//
// The status is recorded per thread, because a callback runs on the thread
// that called into the archive. Two threads reading clones of one archive
// therefore cannot overwrite each other's failures.
//
// Only an error that Rust marks as caused by a failed callback is replaced
// (MarkReaderCallbackFailure); an error the archive raises for another reason
// is returned unchanged. Rust marks a failed read, and also an open during
// which any read failed, because zip-rs does not propagate each of those
// failures itself.

// Records `status`, which must not be OK, as the failure of the reader callback
// running on this thread.
void SetReaderStatus(absl::Status status);

// Discards any status recorded on this thread.
void ClearReaderStatus();

// Marks `status`, which must not be OK, as the direct result of a failed
// reader callback, so that ResolveReaderStatus replaces it with the status the
// callback recorded.
absl::Status MarkReaderCallbackFailure(absl::Status status);

// Returns whether `status` carries the MarkReaderCallbackFailure marker.
bool IsReaderCallbackFailure(const absl::Status& status);

// Returns what an operation that finished with `status` should report, and
// clears the status recorded on this thread either way. If `status` is marked
// as a reader callback failure and a status was recorded, that recorded status
// is returned; otherwise `status` itself, with the marker (if any) removed.
absl::Status ResolveReaderStatus(absl::Status status);

// Runs `op`, which may read through a RandomAccessReader, and returns its
// result (an absl::Status or absl::StatusOr<T>). When `op` fails because a
// callback did, the status the reader recorded on this thread replaces the
// generic error. This is the one place that encodes the invariant: no status
// is recorded before the call and none is left recorded after it, whatever
// happened. Not re-entrant: a nested call made while a status is recorded
// (from the destructor of a reader whose open failed, say) discards it.
template <typename F>
auto ThroughReader(F&& op) {
  ClearReaderStatus();
  auto result = std::forward<F>(op)();
  using Result = decltype(result);
  if constexpr (std::is_same_v<Result, absl::Status>) {
    return ResolveReaderStatus(std::move(result));
  } else {
    if (!result.ok()) {
      return Result(ResolveReaderStatus(std::move(result).status()));
    }
    ClearReaderStatus();
    return result;
  }
}

}  // namespace security::zip::internal

#endif  // SAFE_BINDINGS_ZIP_READER_STATUS_H_
