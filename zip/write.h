#ifndef SECURITY_ZIP_WRITE_H_
#define SECURITY_ZIP_WRITE_H_

#include <cstdint>
#include <utility>
#include <variant>

#include "converters.h"
#include "file.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::zip {

class BufferedZipWriter;
class FsZipWriter;
class BufferedZipStreamWriter;
class FsZipStreamWriter;

class ZipWriterFileOptions {
 public:
  ZipWriterFileOptions()
      : options_(rust::ZipWriterFileOptions::new_()) {}

  absl::Status SetCompressionMethod(CompressionMethod method);

  // The level is validated against the compression method (kDeflated if none
  // was set) when the entry is started: kDeflated and kBzip2 accept 1 to 9,
  // kXz 0 to 9, kZstd -7 to 22, and kStored no level at all. Anything else
  // makes `StartFile`/`AddDirectory` fail with kInvalidArgument. Leaving the
  // level unset selects the method's default.
  void SetCompressionLevel(int64_t level) {
    options_ = options_.compression_level(level);
  }

  void SetUnixPermissions(uint32_t permissions) {
    options_ = options_.unix_permissions(permissions);
  }

  void SetIsLargeFile(bool is_large_file) {
    options_ = options_.large_file(is_large_file);
  }

 private:
  rust::ZipWriterFileOptions options_;
  friend class BufferedZipWriter;
  friend class FsZipWriter;
  friend class BufferedZipStreamWriter;
  friend class FsZipStreamWriter;
};

// Seekable zip writers.
// - `NewFromPath`/`FromFile` validate that `path` is non-empty, free of
//   embedded NUL bytes, and valid UTF-8 (kInvalidArgument), propagate OS
//   file-open errors (such as kNotFound or kPermissionDenied), and when
//   `append` is true open the existing archive without creating it (a missing
//   archive is kNotFound) and validate it (kInvalidArgument on corrupt/empty
//   data, kFailedPrecondition on unsupported archives).
//   `NewFromData`/`FromBuffer` similarly validate `data` when `append` is true.
// - `StartFile`/`AddDirectory` return kInvalidArgument on non-UTF-8, duplicate,
//   or overlong (>= 64 KiB local header) entry names or out-of-range
//   compression levels (see `SetCompressionLevel`), and kFailedPrecondition on
//   unsupported compression methods (such as kLzma, which zip-rs only decodes).
// - `WriteData`/`WriteZipFileContent`/`WriteFileContent` append to the entry
//   started by the last `StartFile()` and return kInternal if no file entry is
//   open, including right after `AddDirectory()` (a directory entry takes no
//   data). `WriteFileContent` validates and opens `path` before that check, so
//   an empty, embedded-NUL or non-UTF-8 `path` still returns kInvalidArgument
//   and an unreadable one its OS error (such as kNotFound or
//   kPermissionDenied).
// - `SetComment(comment)` stores the archive comment that `Finish()` writes;
//   `comment` must be at most 65535 bytes (the zip format's u16 limit),
//   otherwise kInvalidArgument is returned and the previously set comment is
//   left untouched.
// - `Flush()` flushes the underlying sink; `Finish()` writes the central
//   directory. `IsSeekPossible()` is always true. All methods on a moved-from
//   or finished writer return kFailedPrecondition; `IsNone()` reports that
//   state.
class BufferedZipWriter final {
 public:
  static absl::StatusOr<BufferedZipWriter> NewFromData(absl::string_view data,
                                                       bool append);
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit BufferedZipWriter(rust::BufferedZipWriter writer)
      : writer_(std::move(writer)) {}
  rust::BufferedZipWriter writer_;
};

class FsZipWriter final {
 public:
  static absl::StatusOr<FsZipWriter> NewFromPath(absl::string_view path,
                                                 bool append);
  absl::Status Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit FsZipWriter(rust::FsZipWriter writer)
      : writer_(std::move(writer)) {}
  rust::FsZipWriter writer_;
};

class ZipWriter final {
 public:
  static absl::StatusOr<ZipWriter> FromFile(absl::string_view path,
                                            bool append);
  static absl::StatusOr<ZipWriter> FromBuffer(absl::string_view data,
                                              bool append);
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipWriter, FsZipWriter>;

  explicit ZipWriter(BackendType writer) : writer_(std::move(writer)) {}
  BackendType writer_;
};

// Streaming zip writers write zip archives sequentially without seeking. The
// contract below applies to all three classes.
//
// Construction: `New()`/`FromBuffer()`/`NewBufferedZipStreamWriter()` create
// an in-memory writer and never fail (StatusOr is kept for symmetry with the
// other factories). `NewFromPath`/`FromFile`/`NewFsZipStreamWriter` create or
// truncate the file at `path` (there is no append mode), returning
// kInvalidArgument if `path` is empty, contains NUL bytes, or is not valid
// UTF-8, and propagating OS file-open errors (such as kNotFound or
// kPermissionDenied). The file backend buffers its writes; `Finish()` flushes
// them.
//
// Writing: StartFile()/AddDirectory() begin a new entry; WriteData(),
// WriteZipFileContent() and WriteFileContent() append to the entry started by
// the last StartFile() and return kInternal if no file entry is open, including
// right after AddDirectory() (a directory entry takes no data).
// WriteFileContent() validates and opens `path` before that check, so an empty,
// embedded-NUL or non-UTF-8 `path` still returns kInvalidArgument and an
// unreadable one its OS error.
// Because the sink is not seekable, every entry is written with general
// purpose bit 3 set, local-header CRC-32 and sizes zeroed, and the actual
// CRC-32/sizes in a trailing data descriptor. Such an archive must be read
// back by seekable `ZipArchive` (which reads the central directory): neither
// `ZipStreamReader::ReadNextFile()` nor
// `ZipStreamReader::ReadNextFileWithCompressedSize()` can read it (the former
// rejects bit-3 entries with kFailedPrecondition, and the latter verifies the
// payload against the zeroed local-header CRC-32 and does not consume the
// trailing data descriptor). An entry that may exceed 4 GiB must be started
// with `SetIsLargeFile(true)`: zip-rs cannot upgrade an in-flight entry to
// Zip64 without seeking back.
//
// SetComment(): Unsupported. Always returns kFailedPrecondition, because
// zip-rs finalizes a commented archive by seeking back over the comment. The
// finished archive always has an empty comment. (On a moved-from or finished
// writer the message is the generic "writer is not open" one, like every other
// method's.)
//
// Flush(): Flushes the underlying sink; it does not finalize anything.
//
// Finish(): Writes the central directory and consumes the writer. The buffer
// backend returns the archive bytes; the file backend flushes its buffer and
// returns OK (and `ZipStreamWriter::Finish()` returns an empty
// `RustVecU8Wrapper` for it).
//
// IsSeekPossible(): Always false. IsNone(): True if moved-from or finished;
// every other method then returns kFailedPrecondition.
//
// Error codes: kFailedPrecondition (moved-from/finished writer, SetComment,
// unsupported compression method such as kLzma), kInvalidArgument (empty,
// embedded-NUL or non-UTF-8 path, non-UTF-8/duplicate/overlong entry name,
// out-of-range compression level), canonical OS error codes on file
// open/read/write (such as kNotFound or kPermissionDenied), kInternal (no file
// entry open, other I/O or zip-rs error).
class BufferedZipStreamWriter final {
 public:
  // See the contract at the top of this section.
  static absl::StatusOr<BufferedZipStreamWriter> New();
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit BufferedZipStreamWriter(rust::BufferedZipStreamWriter writer)
      : writer_(std::move(writer)) {}
  rust::BufferedZipStreamWriter writer_;
};

class FsZipStreamWriter final {
 public:
  // See the contract at the top of this section.
  static absl::StatusOr<FsZipStreamWriter> NewFromPath(absl::string_view path);
  absl::Status Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  explicit FsZipStreamWriter(rust::FsZipStreamWriter writer)
      : writer_(std::move(writer)) {}
  rust::FsZipStreamWriter writer_;
};

// Facade over BufferedZipStreamWriter / FsZipStreamWriter; every method
// forwards to the selected backend. See the contract at the top of this
// section.
class ZipStreamWriter final {
 public:
  // Creates or truncates the file at `path`; returns kInvalidArgument if `path`
  // is empty, contains NUL bytes, or is non-UTF-8 and propagates OS file-open
  // errors (e.g. kNotFound, kPermissionDenied).
  static absl::StatusOr<ZipStreamWriter> FromFile(absl::string_view path);
  // Writes to an in-memory buffer that Finish() returns. Takes no input
  // buffer (unlike `ZipWriter::FromBuffer`), since a stream writer cannot
  // append to an existing archive.
  static absl::StatusOr<ZipStreamWriter> FromBuffer();
  // Returns the archive bytes for the buffer backend and an empty
  // `RustVecU8Wrapper` for the file backend.
  absl::StatusOr<RustVecU8Wrapper> Finish();
  absl::Status StartFile(absl::string_view file_name,
                         const ZipWriterFileOptions& options);
  absl::Status AddDirectory(absl::string_view file_name,
                            const ZipWriterFileOptions& options);
  absl::Status WriteData(absl::string_view data);
  absl::Status WriteZipFileContent(ZipFile& file);
  absl::Status WriteFileContent(absl::string_view path);
  absl::Status SetComment(absl::string_view comment);
  absl::Status Flush();
  [[nodiscard]] bool IsSeekPossible() const;
  [[nodiscard]] bool IsNone() const;

 private:
  using BackendType = std::variant<BufferedZipStreamWriter, FsZipStreamWriter>;

  explicit ZipStreamWriter(BackendType writer) : writer_(std::move(writer)) {}
  BackendType writer_;
};

// Free function helpers mirroring rust::new_buffered_zip_stream_writer
// and rust::new_fs_zip_stream_writer; they are one-line aliases of
// BufferedZipStreamWriter::New() and FsZipStreamWriter::NewFromPath().
absl::StatusOr<BufferedZipStreamWriter> NewBufferedZipStreamWriter();
absl::StatusOr<FsZipStreamWriter> NewFsZipStreamWriter(absl::string_view path);

}  // namespace security::zip

#endif  // SECURITY_ZIP_WRITE_H_
