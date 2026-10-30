#ifndef SECURITY_ZIP_READER_H_
#define SECURITY_ZIP_READER_H_

#include <cstddef>
#include <cstdint>
#include <type_traits>
#include <utility>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"

namespace security::zip {

// A random-access byte source that an archive can be read through.
//
// Implement this to read an archive that lives somewhere this library knows
// nothing about: a remote object store, a cache, an encrypted volume. Opening
// the archive reads only its end-of-central-directory record and central
// directory, and each entry's bytes are read when that entry is read, so a
// multi-gigabyte archive can be listed, and one entry extracted from it,
// without the rest ever being transferred. Pass an implementation to
// ZipArchive::FromReader.
//
// Prefer ZipArchive::FromFile for a local file: it reads through the operating
// system's page cache, which this interface cannot.
//
// The archive must start at offset 0 of the reader. Data in front of the
// archive, as in a self-extracting executable, is rejected rather than skipped:
// locating a displaced archive means scanning it from the front, which would
// defeat the purpose of this interface. FromBuffer and FromFile do accept
// such archives.
//
// The archive assumes the bytes do not change while it is open. It parses the
// central directory once and then trusts the offsets it found there, so a
// reader whose contents change underneath it will produce entries that are
// truncated, garbled, or attributed to the wrong name rather than an error.
//
// An archive never calls into its reader from two threads at once, but an
// archive may be moved between threads, and archives that share a reader
// through ZipArchive::TryClone may be used concurrently. Implementations must
// therefore be thread-compatible at minimum, and thread-safe if the caller
// clones the archive.
//
// Implement read-ahead if reads are expensive. The archive asks in small
// pieces: 1 KiB windows walking back from the end while locating the
// end-of-central-directory record, a few tens of bytes for each entry's local
// header, and about 8 KiB at a time while decompressing. Only a compressed
// entry is buffered on the way out, and only by 8 KiB; an entry stored without
// compression is passed through one ReadAt per read the caller makes. Against a
// remote object that is a request each, so a reader over one should serve out
// of a block of cache rather than issuing what it is asked for. Blocks of a few
// hundred kilobytes work well; much larger wastes the read-ahead done just
// before each local header, which is discarded.
class RandomAccessReader {
 public:
  RandomAccessReader() = default;
  RandomAccessReader(const RandomAccessReader&) = delete;
  RandomAccessReader& operator=(const RandomAccessReader&) = delete;
  virtual ~RandomAccessReader() = default;

  // Returns the total number of bytes that can be read.
  //
  // Called once when the archive is opened with ZipArchive::FromReader() (or
  // CallbackZipArchive::NewFromReader()), and once on the replacement reader
  // passed to TryCloneWithReader(). Failing here fails the open or clone with
  // the returned status; values exceeding INT64_MAX fail with
  // `absl::StatusCode::kOutOfRange`.
  virtual absl::StatusOr<uint64_t> Size() = 0;

  // Reads into `buffer` starting at `offset` and returns how many bytes were
  // written to it.
  //
  // Returning fewer bytes than `buffer.size()` is allowed and not an error; the
  // archive asks again for the rest. Return 0 only at end of input, otherwise
  // the archive reports the entry as truncated. `offset` is never past the size
  // reported by Size(), and the returned count must not exceed `buffer.size()`.
  virtual absl::StatusOr<size_t> ReadAt(uint64_t offset,
                                        absl::Span<char> buffer) = 0;
};

namespace internal {

// Failure reporting for the callbacks that bridge a RandomAccessReader into the
// Rust archive. Implementation detail of this library; not for use elsewhere.
//
// The bridge can only tell Rust that a read failed, not why, and the resulting
// error would otherwise say no more than that some callback failed. A reader's
// own status is far more useful, naming the object that could not be read and
// whether the failure is worth retrying, so it is stashed here and picked back
// up once Rust reports the failure. The status is per-thread because the
// callbacks run synchronously on the thread that called into the archive; that
// keeps two threads reading two clones of one archive from overwriting each
// other's failures.

// Records `status`, which must not be OK, as the failure of the reader callback
// running on this thread.
void SetReaderStatus(absl::Status status);

// Discards any status recorded on this thread. Call before an operation that
// may read, so that a failure of that operation cannot pick up an older one.
void ClearReaderStatus();

// Returns the status recorded on this thread and clears it, or `fallback` if
// none was recorded.
absl::Status TakeReaderStatus(absl::Status fallback);

// Runs `op`, which may read through a RandomAccessReader, and returns its
// result (an absl::Status or absl::StatusOr<T>). When `op` fails, the status
// the reader recorded on this thread replaces the generic bridge error. This is
// the one place that encodes the invariant: always clear the slot before the
// call, always drain it after a failure.
template <typename F>
auto ThroughReader(F&& op) {
  ClearReaderStatus();
  auto result = std::forward<F>(op)();
  using Result = decltype(result);
  if (!result.ok()) {
    if constexpr (std::is_same_v<Result, absl::Status>) {
      return TakeReaderStatus(std::move(result));
    } else {
      return Result(TakeReaderStatus(std::move(result).status()));
    }
  }
  return result;
}

}  // namespace internal

}  // namespace security::zip

#endif  // SECURITY_ZIP_READER_H_
