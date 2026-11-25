#ifndef SAFE_BINDINGS_ZIP_READER_H_
#define SAFE_BINDINGS_ZIP_READER_H_

#include <cstddef>
#include <cstdint>

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
// uses archives that share the reader from several threads at once. A clone
// made with ZipArchive::TryCloneWithReader has a reader of its own.
//
// Implement read-ahead if reads are expensive. The archive asks in small
// pieces: 1 KiB windows walking back from the end while locating the
// end-of-central-directory record, a few tens of bytes at a time through the
// central directory, which it goes through twice, a few tens of bytes for each
// entry's local header, and about 8 KiB at a time while decompressing. Only a
// compressed entry is buffered on the way out, and only by 8 KiB; an entry
// stored without compression is passed through one ReadAt per read the caller
// makes, or per 1 MiB of it. Against a remote object that is a request each, so
// a reader over one should serve out of a block of cache rather than issuing
// what it is asked for. Blocks of a few hundred kilobytes work well; much
// larger wastes the read-ahead done just before each local header, which is
// discarded.
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
  // `buffer` is never empty, never longer than 1 MiB and never reaches past the
  // size reported by Size(), so there is always at least one byte to read.
  // Returning fewer bytes than `buffer.size()` is allowed and not an error; the
  // archive asks again for the rest. Returning 0 is taken to mean that the
  // input ends before the size reported by Size(), and fails the operation
  // that was reading as truncated input. The returned count must not exceed
  // `buffer.size()`.
  //
  // While the archive is being opened, a failure fails the open with the
  // returned status and a count of 0 fails it with
  // `absl::StatusCode::kInvalidArgument`; ReadAt is not called again after
  // either.
  virtual absl::StatusOr<size_t> ReadAt(uint64_t offset,
                                        absl::Span<char> buffer) = 0;
};

}  // namespace security::zip

#endif  // SAFE_BINDINGS_ZIP_READER_H_
