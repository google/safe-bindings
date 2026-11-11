#include "node.h"

#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <ostream>
#include <string>
#include <utility>

#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/log/die_if_null.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "support/rs_std/slice_ref.h"
#include "support/rs_std/str_ref.h"

namespace security::yaml {

using ::security::crubit_helpers::StringViewFromVecU8;

// NodeView
bool NodeView::IsSequence() const { return view_.is_sequence(); }
bool NodeView::IsMap() const { return view_.is_mapping(); }
bool NodeView::IsScalar() const { return view_.is_scalar(); }
bool NodeView::IsDefined() const { return view_.is_defined(); }
size_t NodeView::size() const { return view_.len(); }
bool NodeView::IsEmpty() const { return view_.is_empty(); }
NodeView::operator bool() const { return IsDefined(); }

NodeView NodeView::operator[](size_t index) const {
  auto val = view_.get_at_index(index);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}

NodeView NodeView::operator[](rs_std::StrRef key) const {
  auto val = view_.get_at_key(key);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}

std::optional<NodeView> NodeView::Get(size_t index) const {
  if (auto val = view_.get_at_index(index); val.has_value()) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

std::optional<NodeView> NodeView::Get(rs_std::StrRef key) const {
  if (std::optional<rust::NodeView> val = view_.get_at_key(key);
      val.has_value()) {
    return NodeView(*val);
  }
  return std::nullopt;
}

NodeView NodeView::get_key_at_index(size_t index) const {
  auto val = view_.get_key_at_index(index);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}

NodeView NodeView::get_value_at_index(size_t index) const {
  auto val = view_.get_value_at_index(index);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}

template <>
std::optional<int64_t> NodeView::as_optional<int64_t>() const {
  return view_.as_i64();
}

template <>
std::optional<int> NodeView::as_optional<int>() const {
  auto v = view_.as_i64();
  if (!v.has_value()) return std::nullopt;
  if (*v < std::numeric_limits<int>::min() ||
      *v > std::numeric_limits<int>::max()) {
    return std::nullopt;
  }
  return static_cast<int>(*v);
}

template <>
std::optional<size_t> NodeView::as_optional<size_t>() const {
  auto v = view_.as_i64();
  if (!v.has_value() || *v < 0) return std::nullopt;
  return static_cast<size_t>(*v);
}

template <>
std::optional<bool> NodeView::as_optional<bool>() const {
  return view_.as_bool();
}

template <>
std::optional<absl::string_view> NodeView::as_optional<absl::string_view>()
    const {
  if (auto sv = view_.as_str(); sv.has_value()) {
    return *sv;
  }
  return std::nullopt;
}

template <>
std::optional<std::string> NodeView::as_optional<std::string>() const {
  if (std::optional<rs_std::StrRef> sv = view_.as_str(); sv.has_value()) {
    // `*sv` is safe to dereference because `sv.has_value()` is true, and the
    // `rs_std::StrRef` borrows from the node referenced by `view_`, which stays
    // alive while `std::string` copies the referenced characters.
    return std::string(*sv);
  }
  return std::nullopt;
}

template <>
std::optional<double> NodeView::as_optional<double>() const {
  return view_.as_f64();
}

absl::StatusOr<std::string> NodeView::Dump() const {
  rs_std::Result<rust::vec_u8::VecU8, rust::vec_u8::VecU8> result =
      rust::dump(view_);
  if (!result.has_value()) {
    return absl::InternalError(StringViewFromVecU8(std::move(result).err()));
  }
  return std::string(StringViewFromVecU8(std::move(result).value()));
}

std::ostream& operator<<(std::ostream& out, const NodeView& node) {
  if (absl::StatusOr<std::string> val = node.Dump(); val.ok()) {
    return out << *val;
  } else {
    return out << val.status();
  }
}

// Node
bool Node::IsSequence() const { return node_.is_sequence(); }
bool Node::IsMap() const { return node_.is_mapping(); }
bool Node::IsScalar() const { return node_.is_scalar(); }
bool Node::IsDefined() const { return node_.is_defined(); }
size_t Node::size() const { return node_.len(); }
bool Node::IsEmpty() const { return node_.is_empty(); }
Node::operator bool() const { return IsDefined(); }

NodeView Node::operator[](rs_std::StrRef key) const {
  auto val = node_.get_at_key(key);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}
NodeView Node::operator[](size_t index) const {
  auto val = node_.get_at_index(index);
  return NodeView(val.has_value() ? *val : rust::NodeView());
}
std::optional<NodeView> Node::Get(size_t index) const {
  if (auto val = node_.get_at_index(index); val.has_value()) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

std::optional<NodeView> Node::Get(rs_std::StrRef key) const {
  if (auto val = node_.get_at_key(key); val.has_value()) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

void Node::SetAtIndex(size_t index, rust::YamlOwned value) {
  node_.set_at_index(index, std::move(value));
}

template <>
std::optional<absl::string_view> Node::as_optional<absl::string_view>() const {
  return as_view().as_optional<absl::string_view>();
}

NodeView Node::as_view() const { return NodeView(node_.as_view()); }

absl::StatusOr<std::string> Dump(const Node& node) {
  return node.as_view().Dump();
}

std::ostream& operator<<(std::ostream& out, const Node& node) {
  if (absl::StatusOr<std::string> val = Dump(node); val.ok()) {
    return out << *val;
  } else {
    return out << val.status();
  }
}

// Load functions
absl::StatusOr<Node> Load(rs_std::StrRef input) {
  rs_std::Result<rust::NodeOwned, rust::vec_u8::VecU8> result =
      rust::load(input);
  if (!result.has_value()) {
    return absl::InvalidArgumentError(
        StringViewFromVecU8(std::move(result).err()));
  }
  rust::NodeOwned docs = std::move(result).value();
  std::optional<rust::NodeView> first_doc = docs.get_at_index(0);
  if (!first_doc.has_value()) {
    return absl::InvalidArgumentError("Empty YAML document");
  }
  return Node(first_doc->to_owned());
}

absl::StatusOr<Node> Load(absl::string_view input) {
  std::optional<rs_std::StrRef> utf8_input = rs_std::StrRef::FromUtf8(input);
  if (!utf8_input.has_value()) {
    return absl::InvalidArgumentError("YAML input is not valid UTF-8");
  }
  return Load(*utf8_input);
}

absl::StatusOr<Node> Load(const char* input) {
  if (input == nullptr) {
    return absl::InvalidArgumentError("YAML input must not be null");
  }
  return Load(absl::string_view(input));
}

rust::YamlOwned ToYaml(int64_t value) {
  return rust::yaml_owned_from_i64(value);
}
rust::YamlOwned ToYaml(int value) {
  return rust::yaml_owned_from_i64(static_cast<int64_t>(value));
}
rust::YamlOwned ToYaml(double value) {
  return rust::yaml_owned_from_f64(value);
}
rust::YamlOwned ToYaml(bool value) {
  return rust::yaml_owned_from_bool(value);
}
rust::YamlOwned ToYaml(const char* value) {
  // C++ overload resolution prefers the built-in pointer-to-`bool` conversion
  // (`ToYaml(bool)`) over user-defined conversions (`absl::string_view`) for
  // string literals, so `const char*` provides an exact match while still
  // dying on null pointers.
  return ToYaml(absl::string_view(ABSL_DIE_IF_NULL(value)));
}
rust::YamlOwned ToYaml(absl::string_view value) {
  return rust::yaml_owned_from_bytes(rs_std::SliceRef<const uint8_t>(
      reinterpret_cast<const uint8_t*>(value.data()), value.size()));
}

}  // namespace security::yaml
