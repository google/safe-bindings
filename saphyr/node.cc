#include "node.h"

#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <ostream>
#include <string>
#include <utility>

#include "crubit/support/rs_std/str_ref.h"
#include "rust/saphyr_rust_wrapper.h"
#include "absl/log/die_if_null.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::yaml {

// NodeView
bool NodeView::IsSequence() const { return view_.is_sequence(); }
bool NodeView::IsMap() const { return view_.is_mapping(); }
bool NodeView::IsScalar() const { return view_.is_scalar(); }
bool NodeView::IsDefined() const { return view_.is_defined(); }
size_t NodeView::size() const { return view_.len(); }
bool NodeView::IsEmpty() const { return view_.is_empty(); }
NodeView::operator bool() const { return IsDefined(); }

NodeView NodeView::operator[](size_t index) const {
  return NodeView(view_.get_at_index(index).value_or(saphyr::NodeView()));
}

NodeView NodeView::operator[](rs_std::StrRef key) const {
  return NodeView(view_.get_at_key(key).value_or(saphyr::NodeView()));
}

std::optional<NodeView> NodeView::Get(size_t index) const {
  if (auto val = view_.get_at_index(index)) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

NodeView NodeView::get_key_at_index(size_t index) const {
  return NodeView(view_.get_key_at_index(index).value_or(saphyr::NodeView()));
}

NodeView NodeView::get_value_at_index(size_t index) const {
  return NodeView(view_.get_value_at_index(index).value_or(saphyr::NodeView()));
}

template <>
std::optional<int64_t> NodeView::as_optional<int64_t>() const {
  return view_.as_i64();
}

template <>
std::optional<int> NodeView::as_optional<int>() const {
  auto v = view_.as_i64();
  if (!v) return std::nullopt;
  if (*v < std::numeric_limits<int>::min() ||
      *v > std::numeric_limits<int>::max()) {
    return std::nullopt;
  }
  return static_cast<int>(*v);
}

template <>
std::optional<size_t> NodeView::as_optional<size_t>() const {
  auto v = view_.as_i64();
  if (!v || *v < 0) return std::nullopt;
  return static_cast<size_t>(*v);
}

template <>
std::optional<bool> NodeView::as_optional<bool>() const {
  return view_.as_bool();
}

template <>
std::optional<absl::string_view> NodeView::as_optional<absl::string_view>()
    const {
  return view_.as_str();
}

template <>
std::optional<std::string> NodeView::as_optional<std::string>() const {
  if (auto sv = view_.as_str()) {
    return std::make_optional(std::string(*sv));
  }
  return std::nullopt;
}

template <>
std::optional<double> NodeView::as_optional<double>() const {
  return view_.as_f64();
}

absl::StatusOr<std::string> NodeView::Dump() const {
  saphyr::ResultString result = saphyr::dump(view_);
  if (!result.is_ok()) {
    return absl::InternalError(std::move(result).unwrap_err());
  }
  return std::string(std::move(result).unwrap());
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
  return NodeView(node_.get_at_key(key).value_or(saphyr::NodeView()));
}
NodeView Node::operator[](size_t index) const {
  return NodeView(node_.get_at_index(index).value_or(saphyr::NodeView()));
}
std::optional<NodeView> Node::Get(size_t index) const {
  if (auto val = node_.get_at_index(index)) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

std::optional<NodeView> Node::Get(rs_std::StrRef key) const {
  if (auto val = node_.get_at_key(key)) {
    return std::make_optional(NodeView(*val));
  }
  return std::nullopt;
}

void Node::SetAtIndex(size_t index, saphyr::YamlOwned value) {
  node_.set_at_index(index, std::move(value));
}

template <>
std::optional<int> Node::as_optional<int>() const {
  auto v = node_.as_i64();
  if (!v) return std::nullopt;
  if (*v < std::numeric_limits<int>::min() ||
      *v > std::numeric_limits<int>::max()) {
    return std::nullopt;
  }
  return static_cast<int>(*v);
}

template <>
std::optional<size_t> Node::as_optional<size_t>() const {
  auto v = node_.as_i64();
  if (!v || *v < 0) return std::nullopt;
  return static_cast<size_t>(*v);
}

template <>
std::optional<bool> Node::as_optional<bool>() const {
  return node_.as_bool();
}

template <>
std::optional<absl::string_view> Node::as_optional<absl::string_view>() const {
  return node_.as_str();
}

template <>
std::optional<double> Node::as_optional<double>() const {
  return node_.as_f64();
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
  saphyr::ResultNodeOwned result = saphyr::load(input);
  if (!result.is_ok()) {
    return absl::InvalidArgumentError(std::move(result).unwrap_err());
  }
  saphyr::NodeOwned docs = std::move(result).unwrap();
  return Node(docs.get_at_index(0)->to_owned());
}

saphyr::YamlOwned ToYaml(int64_t value) {
  return saphyr::yaml_owned_from_i64(value);
}
saphyr::YamlOwned ToYaml(int value) {
  return saphyr::yaml_owned_from_i64(static_cast<int64_t>(value));
}
saphyr::YamlOwned ToYaml(double value) {
  return saphyr::yaml_owned_from_f64(value);
}
saphyr::YamlOwned ToYaml(bool value) {
  return saphyr::yaml_owned_from_bool(value);
}
saphyr::YamlOwned ToYaml(const char* value) {
  // Ideally, we want to use `rs_std::StrRef` at the function API to avoid
  // using FromUtf8Unchecked but the compiler does not prefer the user-defined
  // over the`bool` (built-in) conversion.
  //
  // So we have to use char* to provide exact match for string literals
  return saphyr::yaml_owned_from_str(rs_std::StrRef::FromUtf8Unchecked(
      // Check for nullptr before passing into rust
      ABSL_DIE_IF_NULL(value)));
}

}  // namespace security::yaml
