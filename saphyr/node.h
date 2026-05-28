#ifndef SECURITY_YAML_NODE_H_
#define SECURITY_YAML_NODE_H_

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>

#include "support/rs_std/str_ref.h"
#include "rust.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::yaml {

// `NodeView` provides a non-owning view into a YAML node structure.
// It is used for efficient read-only access to YAML data
// without incurring copy costs.
class NodeView {
 public:
  NodeView() = default;
  explicit NodeView(rust::NodeView view) : view_(view) {}

  // Returns true if the node is a YAML sequence (list).
  bool IsSequence() const;
  // Returns true if the node is a YAML map.
  bool IsMap() const;
  // Returns true if the node is a scalar value (not a sequence or map).
  bool IsScalar() const;
  // Returns true if the node is defined (not null or bad value).
  bool IsDefined() const;
  // Returns the number of elements if the node is a sequence or map, 0
  // otherwise.
  size_t size() const;
  // Returns true if the node is an empty sequence or map.
  bool IsEmpty() const;
  // Returns true if the node is defined.
  explicit operator bool() const;

  // Accesses an element in a sequence by index.
  // Returns a `NodeView` at `index`
  // Use `IsDefined()` to check if the returned node is defined to avoid
  // invalid pointer access.
  NodeView operator[](size_t index) const;
  // Required to resolve overload ambiguity between size_t and const char*
  // for numeric literals like 0. It also checks for negative indices.
  NodeView operator[](int index) const {
    return index < 0 ? NodeView() : operator[](static_cast<size_t>(index));
  }

  // Accesses an element in a map by key.
  // Returns a `NodeView` for which `IsDefined()` is false if the node is not a
  // map or the key is not found.
  NodeView operator[](rs_std::StrRef key) const;
  NodeView operator[](const char* key) const {
    return operator[](rs_std::StrRef::FromUtf8Unchecked(key));
  }
  NodeView operator[](const std::string& key) const {
    return operator[](rs_std::StrRef::FromUtf8Unchecked(key));
  }

  // A safer version of `operator[]` for sequence.
  std::optional<NodeView> Get(size_t index) const;

  // A safer version of `operator[]` for map.
  std::optional<NodeView> Get(rs_std::StrRef key) const;

  // For map iteration by index.
  NodeView get_key_at_index(size_t index) const;
  NodeView get_value_at_index(size_t index) const;

  // Iterator support for sequence and map iteration.
  class Iterator;

  Iterator begin() const;
  Iterator end() const;

  // Tries to convert the node to the specified type `T`.
  // Returns `std::nullopt` if the conversion fails or the node is undefined.
  template <typename T>
  std::optional<T> as_optional() const;

  // Converts the node to a YAML string.
  absl::StatusOr<std::string> Dump() const;

 private:
  friend class Node;
  rust::NodeView view_;
};

// Emits the node to the given output stream.
std::ostream& operator<<(std::ostream& out, const NodeView& node);

// `NodeView::Iterator` provides efficient iteration over YAML sequences and
// maps. It bridges directly to a Rust iterator for O(1) per-step complexity.
class NodeView::Iterator {
 public:
  // A wrapper to mimic std::pair for map iteration and implicit NodeView
  // conversion for sequence iteration.
  struct Proxy {
    NodeView first;
    NodeView second;
    operator NodeView() const {  // NOLINT(google-explicit-constructor)
      return first;
    }
  };

  // Constructs an iterator. Called by NodeView::begin().
  explicit Iterator(const NodeView* node) : node_(node), index_(0) {
    if (node_ && node_->IsDefined()) {
      rust_iter_ = node_->view_.get_iterator();
      has_value_ = true;
      FetchNext();
    } else {
      has_value_ = false;
    }
  }

  // Constructs an end iterator. Called by NodeView::end().
  Iterator() : node_(nullptr), index_(0), has_value_(false) {}

  // Equality comparison.
  // go/totw/226: the compiler will synthesize this with operator!= to support
  // range-based for loops (i.e. while(it != end)) ).
  // See https://en.cppreference.com/w/cpp/language/range-for.html
  bool operator==(const Iterator& other) const {
    if (has_value_ != other.has_value_) {
      return false;
    }
    // If both are false, they are end iterators and should be equal.
    if (!has_value_) {
      return true;
    }
    // Equal if they point to the same element in the same node.
    return node_ == other.node_ && index_ == other.index_;
  }

  // Pre-increment operator. Advances the iterator.
  Iterator& operator++() {
    index_++;
    FetchNext();
    return *this;
  }

  const Proxy& operator*() const { return proxy_; }
  const Proxy* operator->() const { return &proxy_; }

 private:
  // Advances the iterator and updates the cached Proxy.
  void FetchNext() {
    if (!node_ || !has_value_ || !rust_iter_.has_value()) return;

    rust::NodeView key;
    rs_std::Option<rust::NodeView> value;
    has_value_ = rust_iter_->next(key, value);

    if (has_value_) {
      std::optional<rust::NodeView> std_value = std::move(value);
      if (std_value.has_value()) {
        proxy_.first = NodeView(key);
        proxy_.second = NodeView(*std_value);
      } else {
        proxy_.first = NodeView(key);
        proxy_.second = NodeView();
      }
    }
  }

  const NodeView* node_;
  std::optional<rust::YamlIterator> rust_iter_;
  Proxy proxy_;
  // The index of the current element in the sequence or map.
  size_t index_ = 0;
  bool has_value_ = true;
};

inline NodeView::Iterator NodeView::begin() const { return Iterator(this); }
inline NodeView::Iterator NodeView::end() const { return Iterator(); }

// `Node` represents an owning YAML node. It is used when parsing YAML data or
// when modifications to the YAML structure are needed.
class Node {
 public:
  Node() = default;
  explicit Node(rust::NodeOwned node) : node_(std::move(node)) {}

  // Returns true if the node is a YAML sequence (list).
  bool IsSequence() const;
  // Returns true if the node is a YAML map.
  bool IsMap() const;
  // Returns true if the node is a scalar value (not a sequence or map).
  bool IsScalar() const;
  // Returns true if the node is defined (not null or bad value).
  bool IsDefined() const;
  // Returns the number of elements if the node is a sequence or map, 0
  // otherwise.
  size_t size() const;
  // Returns true if the node is an empty sequence or map.
  bool IsEmpty() const;

  // Returns true if the node is defined.
  explicit operator bool() const;

  // Accesses an element in a map by key.
  // Returns a `NodeView` for which `IsDefined()` is false if the node is not a
  // map or the key is not found.
  NodeView operator[](rs_std::StrRef key) const;
  NodeView operator[](const char* key) const {
    return operator[](rs_std::StrRef::FromUtf8Unchecked(key));
  }
  NodeView operator[](absl::string_view key) const {
    return operator[](rs_std::StrRef::FromUtf8Unchecked(key));
  }
  // Accesses an element in a sequence by index.
  // Returns a `NodeView` for which `IsDefined()` is false if the node is not a
  // sequence or the index is out of bounds.
  NodeView operator[](size_t index) const;
  NodeView operator[](int index) const {
    return index < 0 ? NodeView() : operator[](static_cast<size_t>(index));
  }

  // A safer version of `operator[]` for sequence.
  std::optional<NodeView> Get(size_t index) const;
  // A safer version of `operator[]` for map.
  std::optional<NodeView> Get(rs_std::StrRef key) const;

  // Sets the value at a given index in a sequence.
  // If `index` is within bounds, the existing value is replaced.
  // If `index` is equal to `size()`, the value is appended.
  // If the node is not a sequence, this function does nothing.
  void SetAtIndex(size_t index, rust::YamlOwned value);

  // Tries to convert the node to the specified type `T`.
  // Returns `std::nullopt` if the conversion fails or the node is undefined.
  template <typename T>
  std::optional<T> as_optional() const;

  // Returns a view into the node.
  NodeView as_view() const;

 private:
  rust::NodeOwned node_;
};

// Converts the node to a YAML string.
absl::StatusOr<std::string> Dump(const Node& node);
// Emits the node to the given output stream.
std::ostream& operator<<(std::ostream& out, const Node& node);

// Parses a YAML string `input` and returns an owning `Node`.
// Returns an error if parsing fails.
absl::StatusOr<Node> Load(rs_std::StrRef input);

inline absl::StatusOr<Node> Load(absl::string_view input) {
  return Load(rs_std::StrRef::FromUtf8Unchecked(input));
}

inline absl::StatusOr<Node> Load(const char* input) {
  return Load(rs_std::StrRef::FromUtf8Unchecked(input));
}

// Helper functions to convert primitive C++ types to `rust::YamlOwned`
// values.
rust::YamlOwned ToYaml(int64_t value);
rust::YamlOwned ToYaml(int value);
rust::YamlOwned ToYaml(double value);
rust::YamlOwned ToYaml(bool value);
rust::YamlOwned ToYaml(const char* value);

}  // namespace security::yaml

#endif  // SECURITY_YAML_NODE_H_
