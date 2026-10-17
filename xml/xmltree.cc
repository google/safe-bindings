#include "xmltree.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "crubit/xmltree_rust.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"
#include "support/rs_std/option.h"
#include "support/rs_std/result.h"
#include "crubit/alloc.h"
#include "crubit/std.h"
#include "support/rs_std/slice_ref.h"
#include "support/rs_std/str_ref.h"

namespace security::xml {

namespace {

using ::rs::alloc::string::String;
using ::rs_std::Option;
using ::rs_std::Result;
using ::rs_std::SliceRef;
using ::rs_std::StrRef;
using ::xmltree_rust::NodeId;
using ::xmltree_rust::ParseError;
using ::xmltree_rust::TreeError;
using ::xmltree_rust::TreeErrorKind;

std::string RustStringToString(const String& rust_str) {
  return std::string(rust_str.as_str());
}

std::string RustStrRefToString(const StrRef& rust_str) {
  return std::string(rust_str.to_string_view());
}

std::string TreeErrorToString(const TreeError& err) {
  return absl::StrCat(err);
}

absl::Status ParseErrorToStatus(const ParseError& err) {
  return absl::InvalidArgumentError(RustStringToString(err.message));
}

absl::Status TreeErrorToStatus(const TreeError& err) {
  std::string msg = TreeErrorToString(err);
  switch (err.kind().tag) {
    case TreeErrorKind::Tag::DocumentMismatch:
      return absl::InvalidArgumentError(msg);
    case TreeErrorKind::Tag::NodeNotFound:
      return absl::NotFoundError(msg);
    case TreeErrorKind::Tag::InvalidNodeType:
      return absl::FailedPreconditionError(msg);
    case TreeErrorKind::Tag::InvalidOperation:
      return absl::InvalidArgumentError(msg);
  }
  return absl::UnknownError(msg);
}

}  // namespace

absl::StatusOr<XmlDocument> XmlDocument::Parse(absl::string_view xml,
                                               size_t max_depth) {
  Result<xmltree_rust::XmlDocument, ParseError> res =
      xmltree_rust::parse(
          absl::MakeSpan(reinterpret_cast<const uint8_t*>(xml.data()),
                         xml.size()),
          max_depth);
  if (!res.has_value()) {
    return ParseErrorToStatus(res.err());
  }
  return XmlDocument(*std::move(res));
}

absl::StatusOr<std::vector<XmlNodeId>> XmlDocument::ParseFragment(
    absl::string_view xml, size_t max_depth) {
  Result<xmltree_rust::NodeId, ParseError> res =
      xmltree_rust::parse_fragment(
          doc_,
          absl::MakeSpan(reinterpret_cast<const uint8_t*>(xml.data()),
                         xml.size()),
          max_depth);
  if (!res.has_value()) {
    return ParseErrorToStatus(res.err());
  }

  XmlNodeId dummy_node(*res);
  ASSIGN_OR_RETURN(std::vector<XmlNodeId> children, GetChildren(dummy_node));

  for (XmlNodeId child : children) {
    RETURN_IF_ERROR(DetachNode(child));
  }
  RETURN_IF_ERROR(DeleteNode(dummy_node));
  return children;
}

XmlNodeId XmlDocument::Root() const {
  Option<NodeId> root = doc_.root();
  if (!root.has_value()) return XmlNodeId();
  return XmlNodeId(*root);
}

absl::StatusOr<NodeType> XmlDocument::GetNodeType(XmlNodeId node_id) const {
  Result<xmltree_rust::NodeType, TreeError> res =
      doc_.node_type(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());

  switch (res->tag) {
    case xmltree_rust::NodeType::Tag::Element:
      return NodeType::kElement;
    case xmltree_rust::NodeType::Tag::Comment:
      return NodeType::kComment;
    case xmltree_rust::NodeType::Tag::CData:
      return NodeType::kCData;
    case xmltree_rust::NodeType::Tag::Text:
      return NodeType::kText;
    case xmltree_rust::NodeType::Tag::ProcessingInstruction:
      return NodeType::kProcessingInstruction;
  }
  return absl::InternalError("Unknown NodeType tag");
}

absl::StatusOr<std::string> XmlDocument::GetName(XmlNodeId node_id) const {
  Result<StrRef, TreeError> res = doc_.node_name(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  return RustStrRefToString(*res);
}

absl::StatusOr<std::string> XmlDocument::GetPrefix(XmlNodeId node_id) const {
  Result<Option<StrRef>, TreeError> res = doc_.node_prefix(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  const Option<StrRef>& prefix = *res;
  if (!prefix.has_value()) return "";
  return RustStrRefToString(*prefix);
}

absl::StatusOr<std::string> XmlDocument::GetNamespace(XmlNodeId node_id) const {
  Result<Option<StrRef>, TreeError> res = doc_.node_namespace(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  const Option<StrRef>& namespace_uri = *res;
  if (!namespace_uri.has_value()) return "";
  return RustStrRefToString(*namespace_uri);
}

absl::StatusOr<std::vector<Attribute>> XmlDocument::GetAttributes(
    XmlNodeId node_id) const {
  Result<SliceRef<const xmltree_rust::Attribute>, TreeError> res =
      doc_.node_attributes(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());

  const SliceRef<const xmltree_rust::Attribute>& attrs = *res;
  std::vector<Attribute> result;
  result.reserve(attrs.size());
  for (size_t i = 0; i < attrs.size(); ++i) {
    const xmltree_rust::Attribute& attr = attrs.data()[i];
    result.push_back(Attribute{
        .key = RustStringToString(attr.key),
        .prefix = attr.prefix.has_value()
                      ? std::make_optional(RustStringToString(*attr.prefix))
                      : std::nullopt,
        .namespace_uri =
            attr.namespace_uri.has_value()
                ? std::make_optional(RustStringToString(*attr.namespace_uri))
                : std::nullopt,
        .value = RustStringToString(attr.value),
    });
  }
  return result;
}

absl::StatusOr<std::string> XmlDocument::GetAttribute(
    XmlNodeId node_id, absl::string_view name) const {
  std::optional<StrRef> str_ref = StrRef::FromUtf8(name);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 name in GetAttribute");
  }

  Result<StrRef, TreeError> res = doc_.get_attribute(node_id.id_, *str_ref);
  if (!res.has_value()) {
    if (res.err().kind().tag ==
        xmltree_rust::TreeErrorKind::Tag::InvalidOperation) {
      return absl::NotFoundError(absl::StrCat("Attribute not found: ", name));
    }
    return TreeErrorToStatus(res.err());
  }
  return RustStrRefToString(*res);
}

absl::StatusOr<std::optional<std::string>> XmlDocument::FindAttributeByName(
    XmlNodeId node_id, absl::string_view name) const {
  std::optional<StrRef> str_ref = StrRef::FromUtf8(name);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError(
        "Invalid UTF-8 name in FindAttributeByName");
  }

  Result<Option<StrRef>, TreeError> res =
      doc_.find_attribute_by_name(node_id.id_, *str_ref);
  if (!res.has_value()) return TreeErrorToStatus(res.err());

  const Option<StrRef>& attr = *res;
  if (!attr.has_value()) return std::nullopt;
  return RustStrRefToString(*attr);
}

absl::StatusOr<std::optional<std::string>>
XmlDocument::FindAttributeByNameAndNamespace(
    XmlNodeId node_id, absl::string_view name,
    std::optional<absl::string_view> namespace_uri) const {
  std::optional<StrRef> name_ref = StrRef::FromUtf8(name);
  if (!name_ref.has_value()) {
    return absl::InvalidArgumentError(
        "Invalid UTF-8 name in FindAttributeByNameAndNamespace");
  }
  std::optional<StrRef> ns_ref;
  if (namespace_uri.has_value()) {
    if (namespace_uri->empty()) {
      return absl::InvalidArgumentError(
          "Attribute namespace_uri cannot be empty; use std::nullopt to match "
          "attributes without a namespace");
    }
    ns_ref = StrRef::FromUtf8(*namespace_uri);
    if (!ns_ref.has_value()) {
      return absl::InvalidArgumentError(
          "Invalid UTF-8 namespace_uri in FindAttributeByNameAndNamespace");
    }
  }

  Result<Option<StrRef>, TreeError> res =
      doc_.find_attribute_by_name_and_namespace(node_id.id_, *name_ref, ns_ref);
  if (!res.has_value()) return TreeErrorToStatus(res.err());

  const Option<StrRef>& attr = *res;
  if (!attr.has_value()) return std::nullopt;
  return RustStrRefToString(*attr);
}

absl::StatusOr<std::vector<XmlNodeId>> XmlDocument::GetChildren(
    XmlNodeId node_id) const {
  Result<SliceRef<const NodeId>, TreeError> res =
      doc_.node_children(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());

  const SliceRef<const NodeId>& children = *res;
  std::vector<XmlNodeId> result;
  result.reserve(children.size());
  for (size_t i = 0; i < children.size(); ++i) {
    const NodeId& child_id = children.data()[i];
    result.push_back(XmlNodeId(child_id));
  }
  return result;
}

absl::StatusOr<XmlNodeId> XmlDocument::GetParent(XmlNodeId node_id) const {
  Result<Option<NodeId>, TreeError> res = doc_.node_parent(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  const Option<NodeId>& parent = *res;
  if (!parent.has_value()) return XmlNodeId();
  return XmlNodeId(*parent);
}

absl::StatusOr<std::string> XmlDocument::GetText(XmlNodeId node_id) const {
  Result<String, TreeError> res = doc_.node_text(node_id.id_);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  return RustStringToString(*res);
}

absl::StatusOr<XmlNodeId> XmlDocument::FindChildByName(
    XmlNodeId node_id, absl::string_view name) const {
  std::optional<StrRef> str_ref = StrRef::FromUtf8(name);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 name in FindChildByName");
  }

  Result<Option<NodeId>, TreeError> res =
      doc_.find_child_by_name(node_id.id_, *str_ref);
  if (!res.has_value()) return TreeErrorToStatus(res.err());
  const Option<NodeId>& child = *res;
  if (!child.has_value()) return XmlNodeId();
  return XmlNodeId(*child);
}

absl::Status XmlDocument::AddChild(XmlNodeId parent, XmlNodeId child) {
  Result<NodeId, TreeError> res = doc_.add_child(parent.id_, child.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::InsertChildAfter(XmlNodeId parent, XmlNodeId child,
                                           XmlNodeId ref_child) {
  Result<NodeId, TreeError> res =
      doc_.insert_child_after(parent.id_, child.id_, ref_child.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::DetachNode(XmlNodeId node_id) {
  Result<NodeId, TreeError> res = doc_.detach_node(node_id.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::DeleteNode(XmlNodeId node_id) {
  Result<NodeId, TreeError> res = doc_.delete_node(node_id.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::SetRoot(XmlNodeId node_id) {
  Result<NodeId, TreeError> res = doc_.set_root(node_id.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::StatusOr<XmlNodeId> XmlDocument::CreateElement(absl::string_view name) {
  std::optional<StrRef> str_ref = StrRef::FromUtf8(name);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 name in CreateElement");
  }
  return XmlNodeId(doc_.create_element(*str_ref));
}

absl::StatusOr<XmlNodeId> XmlDocument::CreateTextNode(
    absl::string_view content) {
  std::optional<StrRef> str_ref = StrRef::FromUtf8(content);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError(
        "Invalid UTF-8 content in CreateTextNode");
  }
  return XmlNodeId(doc_.create_text_node(*str_ref));
}

absl::Status XmlDocument::SetAttribute(XmlNodeId node_id, absl::string_view key,
                                       absl::string_view value) {
  return SetAttribute(node_id, key, std::nullopt, value);
}

absl::Status XmlDocument::SetAttribute(XmlNodeId node_id, absl::string_view key,
                                       std::optional<Namespace> ns,
                                       absl::string_view value) {
  if (ns.has_value()) {
    if (ns->prefix.empty()) {
      return absl::InvalidArgumentError("Attribute prefix cannot be empty");
    }
    if (ns->uri.empty()) {
      return absl::InvalidArgumentError(
          "Attribute namespace URI cannot be empty");
    }
  }
  std::optional<StrRef> key_ref = StrRef::FromUtf8(key);
  if (!key_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 key in SetAttribute");
  }
  std::optional<StrRef> prefix_ref;
  std::optional<StrRef> uri_ref;
  if (ns.has_value()) {
    prefix_ref = StrRef::FromUtf8(ns->prefix);
    if (!prefix_ref.has_value()) {
      return absl::InvalidArgumentError("Invalid UTF-8 prefix in SetAttribute");
    }
    uri_ref = StrRef::FromUtf8(ns->uri);
    if (!uri_ref.has_value()) {
      return absl::InvalidArgumentError(
          "Invalid UTF-8 namespace_uri in SetAttribute");
    }
  }
  std::optional<StrRef> val_ref = StrRef::FromUtf8(value);
  if (!val_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 value in SetAttribute");
  }
  Result<NodeId, TreeError> res = doc_.set_attribute_with_ns(
      node_id.id_, *key_ref, prefix_ref, uri_ref, *val_ref);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::DeleteAttribute(XmlNodeId node_id,
                                          absl::string_view key) {
  std::optional<StrRef> key_ref = StrRef::FromUtf8(key);
  if (!key_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 key in DeleteAttribute");
  }
  Result<NodeId, TreeError> res = doc_.delete_attribute(node_id.id_, *key_ref);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::DeleteAttribute(
    XmlNodeId node_id, absl::string_view key,
    std::optional<absl::string_view> namespace_uri) {
  std::optional<StrRef> key_ref = StrRef::FromUtf8(key);
  if (!key_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 key in DeleteAttribute");
  }
  std::optional<StrRef> ns_ref;
  if (namespace_uri.has_value()) {
    if (namespace_uri->empty()) {
      return absl::InvalidArgumentError(
          "Attribute namespace_uri cannot be empty; use std::nullopt to match "
          "attributes without a namespace");
    }
    ns_ref = StrRef::FromUtf8(*namespace_uri);
    if (!ns_ref.has_value()) {
      return absl::InvalidArgumentError(
          "Invalid UTF-8 namespace_uri in DeleteAttribute");
    }
  }

  Result<NodeId, TreeError> res = doc_.delete_attribute_by_name_and_namespace(
      node_id.id_, *key_ref, ns_ref);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::SetElementName(XmlNodeId node_id,
                                         absl::string_view name) {
  std::optional<StrRef> name_ref = StrRef::FromUtf8(name);
  if (!name_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 name in SetElementName");
  }
  Result<NodeId, TreeError> res = doc_.set_element_name(node_id.id_, *name_ref);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::Status XmlDocument::SetText(XmlNodeId node_id, absl::string_view text) {
  std::optional<StrRef> text_ref = StrRef::FromUtf8(text);
  if (!text_ref.has_value()) {
    return absl::InvalidArgumentError("Invalid UTF-8 text in SetText");
  }
  Result<NodeId, TreeError> res = doc_.set_text(node_id.id_, *text_ref);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return absl::OkStatus();
}

absl::StatusOr<XmlNodeId> XmlDocument::CloneNode(XmlNodeId node_id) {
  Result<NodeId, TreeError> res = doc_.clone_node(node_id.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return XmlNodeId(*res);
}

absl::StatusOr<std::string> XmlDocument::SerializeDocument() const {
  Result<String, TreeError> res = doc_.serialize_document();
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return RustStringToString(*res);
}

absl::StatusOr<std::string> XmlDocument::SerializeNode(
    XmlNodeId node_id) const {
  Result<String, TreeError> res = doc_.serialize_node(node_id.id_);
  if (!res.has_value()) {
    return TreeErrorToStatus(res.err());
  }
  return RustStringToString(*res);
}

}  // namespace security::xml
