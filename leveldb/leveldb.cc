#include "leveldb.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>

#include "support/rs_std/result.h"
#include "support/rs_std/slice_ref.h"
#include "support/rs_std/str_ref.h"
#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/log/check.h"
#include "absl/memory/memory.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::leveldb {

namespace {

using ::security::crubit_helpers::StringViewFromVecU8;

rs_std::SliceRef<const uint8_t> StringViewToSliceRef(absl::string_view sv) {
  return rs_std::SliceRef<const uint8_t>(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(sv.data()), sv.size()));
}

absl::Status LevelDBErrorToStatus(const rust::LevelDBError& err) {
  absl::string_view msg = StringViewFromVecU8(err.message);
  absl::StatusCode absl_code;
  switch (err.code.tag) {
    case rust::LevelDBErrorCode::Tag::NotFound:
      absl_code = absl::StatusCode::kNotFound;
      break;
    case rust::LevelDBErrorCode::Tag::Corruption:
    case rust::LevelDBErrorCode::Tag::InvalidData:
      absl_code = absl::StatusCode::kDataLoss;
      break;
    case rust::LevelDBErrorCode::Tag::NotSupported:
      absl_code = absl::StatusCode::kUnimplemented;
      break;
    case rust::LevelDBErrorCode::Tag::InvalidArgument:
      absl_code = absl::StatusCode::kInvalidArgument;
      break;
    case rust::LevelDBErrorCode::Tag::AlreadyExists:
      absl_code = absl::StatusCode::kAlreadyExists;
      break;
    case rust::LevelDBErrorCode::Tag::LockError:
      absl_code = absl::StatusCode::kUnavailable;
      break;
    case rust::LevelDBErrorCode::Tag::PermissionDenied:
      absl_code = absl::StatusCode::kPermissionDenied;
      break;
    default:
      absl_code = absl::StatusCode::kInternal;
      break;
  }
  return absl::Status(absl_code, msg);
}

template <typename T>
absl::Status ResultToStatus(
    rs_std::Result<T, rust::LevelDBError>&& result) {
  if (result.has_value()) {
    return absl::OkStatus();
  }
  return LevelDBErrorToStatus(std::move(result).err());
}

}  // namespace

Options::Options() : rs_options_(rust::Options::create()) {}
Options::~Options() = default;

Options in_memory() { return Options(rust::in_memory()); }

void Options::SetCreateIfMissing(bool val) {
  rs_options_.set_create_if_missing(val);
}
void Options::SetErrorIfExists(bool val) {
  rs_options_.set_error_if_exists(val);
}
void Options::SetParanoidChecks(bool val) {
  rs_options_.set_paranoid_checks(val);
}
void Options::SetWriteBufferSize(size_t val) {
  rs_options_.set_write_buffer_size(val);
}
void Options::SetMaxOpenFiles(size_t val) {
  rs_options_.set_max_open_files(val);
}
void Options::SetMaxFileSize(size_t val) { rs_options_.set_max_file_size(val); }
void Options::SetBlockCacheCapacityBytes(size_t val) {
  rs_options_.set_block_cache_capacity_bytes(val);
}
void Options::SetBlockSize(size_t val) { rs_options_.set_block_size(val); }
void Options::SetBlockRestartInterval(size_t val) {
  rs_options_.set_block_restart_interval(val);
}
void Options::SetCompressor(uint8_t val) { rs_options_.set_compressor(val); }
void Options::SetReuseLogs(bool val) { rs_options_.set_reuse_logs(val); }
void Options::SetReuseManifest(bool val) {
  rs_options_.set_reuse_manifest(val);
}
void Options::SetComparator(std::unique_ptr<Comparator> cmp) {
  rs_options_.set_cmp(cmp.release());
}

void Options::SetEnv(std::unique_ptr<Env> env) {
  rs_options_.set_env(env.release());
}
void Options::SetFilterPolicy(std::unique_ptr<FilterPolicy> filter_policy) {
  rs_options_.set_filter_policy(filter_policy.release());
}
void Options::SetInfoLog(std::unique_ptr<Logger> logger) {
  rs_options_.set_info_log(logger.release());
}

WriteBatch::WriteBatch() = default;
WriteBatch::~WriteBatch() = default;

void WriteBatch::Put(absl::string_view key, absl::string_view value) {
  rs_batch_.put(StringViewToSliceRef(key), StringViewToSliceRef(value));
}
void WriteBatch::Delete(absl::string_view key) {
  rs_batch_.delete_(StringViewToSliceRef(key));
}
void WriteBatch::Clear() { rs_batch_.clear(); }

absl::StatusOr<std::unique_ptr<DB>> DB::Open(absl::string_view name,
                                             Options options) {
  auto name_ref = rs_std::StrRef::FromUtf8(name);
  if (!name_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 in database name");
  }

  auto result =
      rust::DB::open(name_ref.value(), std::move(options.rs_options_));
  if (result.has_value()) {
    // Using `new` to access private constructor.
    return absl::WrapUnique(new DB(std::move(result).value()));
  }
  return ResultToStatus(std::move(result));
}

DB::DB(rust::DB rs_db) : rs_db_(std::move(rs_db)) {}
DB::~DB() = default;

absl::Status DB::Put(absl::string_view key, absl::string_view value,
                     bool sync) {
  absl::Status status = ResultToStatus(
      rs_db_.put(StringViewToSliceRef(key), StringViewToSliceRef(value)));
  if (sync && status.ok()) {
    status = ResultToStatus(rs_db_.flush());
  }
  return status;
}

absl::Status DB::Delete(absl::string_view key, bool sync) {
  absl::Status status =
      ResultToStatus(rs_db_.delete_(StringViewToSliceRef(key)));
  if (sync && status.ok()) {
    status = ResultToStatus(rs_db_.flush());
  }
  return status;
}

absl::Status DB::Write(const WriteBatch& updates, bool sync) {
  return ResultToStatus(rs_db_.write(updates.rs_batch_, sync));
}

absl::StatusOr<std::string> DB::Get(absl::string_view key) {
  auto result = rs_db_.get(StringViewToSliceRef(key));
  if (!result.has_value()) {
    return ResultToStatus(std::move(result));
  }
  std::optional<rust::DBValue> opt = std::move(result).value();
  if (!opt.has_value()) {
    return absl::NotFoundError("Key not found");
  }
  return std::string(opt->to_string());
}

absl::StatusOr<std::string> DB::GetAt(const Snapshot& snapshot,
                                      absl::string_view key) {
  auto result = rs_db_.get_at(snapshot.rs_snapshot_, StringViewToSliceRef(key));
  if (!result.has_value()) {
    return ResultToStatus(std::move(result));
  }
  std::optional<rust::DBValue> opt = std::move(result).value();
  if (!opt.has_value()) {
    return absl::NotFoundError("Key not found");
  }
  return std::string(opt->to_string());
}

std::unique_ptr<Iterator> DB::NewIterator() {
  auto result = rs_db_.new_iter();
  if (result.has_value()) {
    // Using `new` to access private constructor.
    return absl::WrapUnique(new Iterator(std::move(result).value()));
  }
  return nullptr;
}

std::unique_ptr<Iterator> DB::NewIteratorAt(const Snapshot& snapshot) {
  auto result = rs_db_.new_iter_at(snapshot.rs_snapshot_);
  if (result.has_value()) {
    // Using `new` to access private constructor.
    return absl::WrapUnique(new Iterator(std::move(result).value()));
  }
  return nullptr;
}

std::unique_ptr<Snapshot> DB::GetSnapshot() {
  auto result = rs_db_.get_snapshot();
  if (result.has_value()) {
    // Using `new` to access private constructor.
    return absl::WrapUnique(new Snapshot(std::move(result).value()));
  }
  return nullptr;
}

absl::Status DB::CompactRange(absl::string_view start, absl::string_view end) {
  return ResultToStatus(rs_db_.compact_range(StringViewToSliceRef(start),
                                             StringViewToSliceRef(end)));
}

Iterator::Iterator(rust::DBIterator rs_iter)
    : rs_iter_(std::move(rs_iter)) {
  RefreshCache();
}
Iterator::~Iterator() = default;

bool Iterator::Valid() const { return current_key_.has_value(); }

void Iterator::SeekToFirst() {
  rs_iter_.seek_to_first();
  RefreshCache();
}

void Iterator::Seek(absl::string_view target) {
  rs_iter_.seek(StringViewToSliceRef(target));
  RefreshCache();
}

void Iterator::Next() {
  rs_iter_.advance();
  RefreshCache();
}

void Iterator::Prev() {
  rs_iter_.prev();
  RefreshCache();
}

void Iterator::RefreshCache() {
  if (!rs_iter_.valid()) {
    current_key_.reset();
    current_value_.reset();
    return;
  }
  auto opt = rs_iter_.current();
  if (opt.has_value()) {
    current_key_ = std::string(opt->key().to_string());
    current_value_ = std::string(opt->value().to_string());
  } else {
    current_key_.reset();
    current_value_.reset();
  }
}

absl::string_view Iterator::key() const {
  CHECK(current_key_.has_value()) << "Iterator is invalid";
  return *current_key_;
}

absl::string_view Iterator::value() const {
  CHECK(current_value_.has_value()) << "Iterator is invalid";
  return *current_value_;
}

Snapshot::Snapshot(rust::Snapshot rs_snapshot)
    : rs_snapshot_(std::move(rs_snapshot)) {}
Snapshot::~Snapshot() = default;

}  // namespace security::leveldb
