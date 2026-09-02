#ifndef SECURITY_JSON_SERDE_JSON_FAST_SERDE_JSON_BRIDGE_H_
#define SECURITY_JSON_SERDE_JSON_FAST_SERDE_JSON_BRIDGE_H_

#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

#include <google/protobuf/struct.pb.h>
#include "rust/fast_serde_json_rs.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::json::fast_serde_json_bridge {

// A high-performance, handle-based JSON document parser backed by Rust's
// serde_json.
//
// THREAD-SAFETY NOTICE: FastSerdeJson is NOT thread-safe for concurrent reads
// or writes. Under the hood, handle resolution utilizes an internal pointer
// cache inside a HandleRegistry. Rather than paying the runtime overhead of
// wrapping this cache in a mutex or atomics—which would force all consumers to
// pay a performance penalty even in single-threaded workflows—getter and
// navigation accessors are deliberately marked non-const. This makes it
// explicit to C++ consumers that internal state (the pointer cache) is modified
// during read operations, serving as a clear signal that external
// synchronization (such as a mutex) is required if an instance is shared across
// multiple threads.
class FastSerdeJson final {
 public:
  // Opaque handle referencing a specific node within a FastSerdeJson document.
  // Handles are cheap to copy or move, and can be reused across document
  // clones. A handle remains valid as long as its corresponding node in the
  // document exists.
  using NodeHandle = fast_serde_json_rs::NodeHandle;

  FastSerdeJson(FastSerdeJson&&) = default;
  FastSerdeJson& operator=(FastSerdeJson&&) = default;

  // Move-only (not copyable). Explicit CloneSubtree method for copying.
  FastSerdeJson(const FastSerdeJson&) = delete;
  FastSerdeJson& operator=(const FastSerdeJson&) = delete;

  // Explicit clone of the document or a subtree specified by handle.
  absl::StatusOr<FastSerdeJson> CloneSubtree(NodeHandle handle = {});

  bool operator==(const FastSerdeJson& other) const;

  // Parses a raw JSON string into a FastSerdeJson object.
  static absl::StatusOr<FastSerdeJson> Parse(absl::string_view data);

  // Creates a new FastSerdeJson of a given type.
  static FastSerdeJson CreateObject();
  static FastSerdeJson CreateArray();
  static FastSerdeJson CreateInt(int64_t value);
  static FastSerdeJson CreateBool(bool value);
  static absl::StatusOr<FastSerdeJson> CreateDouble(double value);
  static FastSerdeJson CreateNull();
  static absl::StatusOr<FastSerdeJson> CreateString(absl::string_view value);

  // Returns the value of the current json node (or root if handle is default).
  absl::StatusOr<int64_t> GetInt(NodeHandle handle = {});
  absl::StatusOr<bool> GetBool(NodeHandle handle = {});
  absl::StatusOr<std::string> GetString(NodeHandle handle = {});
  absl::StatusOr<double> GetDouble(NodeHandle handle = {});
  absl::StatusOr<std::vector<NodeHandle>> GetArray(NodeHandle handle = {});
  absl::StatusOr<NodeHandle> GetArrayElement(size_t index,
                                             NodeHandle handle = {});

  // Returns a handle to the corresponding `key` field of this json object.
  absl::StatusOr<NodeHandle> GetField(absl::string_view key,
                                      NodeHandle handle = {});

  // Returns the value of the corresponding field of this json object.
  absl::StatusOr<std::string> GetFieldString(absl::string_view key,
                                             NodeHandle handle = {});
  absl::StatusOr<bool> GetFieldBool(absl::string_view key,
                                    NodeHandle handle = {});
  absl::StatusOr<int64_t> GetFieldInt(absl::string_view key,
                                      NodeHandle handle = {});
  absl::StatusOr<double> GetFieldDouble(absl::string_view key,
                                        NodeHandle handle = {});
  absl::StatusOr<NodeHandle> GetFieldObject(absl::string_view key,
                                            NodeHandle handle = {});
  absl::StatusOr<std::vector<NodeHandle>> GetFieldArray(absl::string_view key,
                                                        NodeHandle handle = {});
  absl::StatusOr<NodeHandle> GetFieldArrayElement(absl::string_view key,
                                                  size_t index,
                                                  NodeHandle handle = {});

  // Methods for checking the type of a json node.
  // Note: IsEmpty returns an error status if `handle` is invalid.
  // All other predicate methods (IsNull, IsObject, IsArray, etc.) return
  // `false` if `handle` is invalid or refers to a non-existent path.
  bool IsNull(NodeHandle handle = {});
  absl::StatusOr<bool> IsEmpty(NodeHandle handle = {});
  bool IsObject(NodeHandle handle = {});
  bool IsArray(NodeHandle handle = {});
  bool IsString(NodeHandle handle = {});
  bool IsNumber(NodeHandle handle = {});
  bool IsDouble(NodeHandle handle = {});
  bool IsBool(NodeHandle handle = {});
  bool IsInt(NodeHandle handle = {});

  // If the target node is an object, returns whether the field exists.
  absl::StatusOr<bool> HasField(absl::string_view key, NodeHandle handle = {});

  // Returns the keys of a json object.
  absl::StatusOr<std::vector<std::string>> GetKeys(NodeHandle handle = {});

  std::string ToString(bool sort_keys = true, NodeHandle handle = {});
  absl::StatusOr<google::protobuf::Struct> ToProtoStruct(
      NodeHandle handle = {});
  absl::StatusOr<google::protobuf::Value> ToProtoValue(NodeHandle handle = {});

  // Methods for adding fields to a JSON object node.
  absl::Status AddFieldBool(absl::string_view key, bool value,
                            NodeHandle handle = {});
  absl::Status AddFieldDouble(absl::string_view key, double value,
                              NodeHandle handle = {});
  absl::Status AddFieldInt(absl::string_view key, int64_t value,
                           NodeHandle handle = {});
  absl::Status AddFieldNull(absl::string_view key, NodeHandle handle = {});
  absl::Status AddFieldObject(absl::string_view key, FastSerdeJson value,
                              NodeHandle handle = {});
  absl::Status AddFieldString(absl::string_view key, absl::string_view value,
                              NodeHandle handle = {});
  absl::Status AddFieldArray(absl::string_view key,
                             std::vector<FastSerdeJson> value,
                             NodeHandle handle = {});

 private:
  explicit FastSerdeJson(fast_serde_json_rs::FastSerdeJson sj);

  fast_serde_json_rs::FastSerdeJson json_obj_;
};

}  // namespace security::json::fast_serde_json_bridge

#endif  // SECURITY_JSON_SERDE_JSON_FAST_SERDE_JSON_BRIDGE_H_
