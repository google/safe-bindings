#ifndef SECURITY_XML_XMLTREE_H_
#define SECURITY_XML_XMLTREE_H_

#include <cstddef>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "crubit/xmltree_rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::xml {

class XmlDocument;

// Default maximum nesting depth for XML parsing (matches libxml's default).
inline constexpr size_t kDefaultMaxDepth = xmltree_rust::DEFAULT_MAX_DEPTH;

// Represents a handle to an XML node in an `XmlDocument`.
//
// This is a lightweight handle containing a document ID and node ID.
// It does not own memory and cannot dangle even if the node is deleted or the
// document is modified.
class XmlNodeId {
 public:
  XmlNodeId() = default;

  // Checks if this node handle is non-empty.
  bool IsValid() const { return id_.generation != 0; }

  bool operator==(const XmlNodeId& other) const { return id_ == other.id_; }

 private:
  friend class XmlDocument;

  explicit XmlNodeId(xmltree_rust::NodeId id) : id_(id) {}

  xmltree_rust::NodeId id_{};
};

// Represents an XML namespace binding with a URI and prefix.
struct Namespace {
  absl::string_view uri;
  absl::string_view prefix;

  bool operator==(const Namespace& other) const = default;
};

// Represents an XML attribute, including its key (local name), optional prefix,
// namespace URI, and value.
struct Attribute {
  std::string key;
  std::optional<std::string> prefix;
  std::optional<std::string> namespace_uri;
  std::string value;

  bool operator==(const Attribute& other) const = default;
};

// Types of XML nodes.
enum class NodeType {
  kElement,
  kComment,
  kCData,
  kText,
  kProcessingInstruction,
};

// Represents an XML DOM document backed by an Arena.
//
// Provides memory-safe operations on XML nodes via `XmlNodeId` handles.
// `XmlDocument` owns all the nodes in the tree, and it can also own nodes in a
// detached state (typically temporarily, during tree edit operations).
//
// This class is not thread safe. Creating `XmlDocument` objects concurrently
// from different threads is supported, but a single `XmlDocument` instance
// can't be used simultaneously from multiple threads.
class XmlDocument {
 public:
  // Non-copyable to prevent accidental, expensive deep copies of DOM trees.
  // Pass by reference or std::move instead. If deep copying is ever needed, an
  // explicit Clone() method should be added to the API.
  XmlDocument(const XmlDocument&) = delete;
  XmlDocument& operator=(const XmlDocument&) = delete;

  // Movable
  XmlDocument(XmlDocument&&) = default;
  XmlDocument& operator=(XmlDocument&&) = default;

  // Parses an XML string into an `XmlDocument`.
  static absl::StatusOr<XmlDocument> Parse(absl::string_view xml,
                                           size_t max_depth = kDefaultMaxDepth);

  // Parses an XML fragment (e.g., `<p>one</p><p>two</p>`) into a sequence of
  // detached nodes that belong to this document.
  //
  // Standard XML parsers strictly require a single root element and will fail
  // on fragments. This method safely bypasses that restriction by wrapping the
  // fragment in a dummy root during parsing, extracting the children, and
  // deleting the dummy root. This allows you to construct and inject subtrees
  // directly from strings.
  absl::StatusOr<std::vector<XmlNodeId>> ParseFragment(
      absl::string_view xml, size_t max_depth = kDefaultMaxDepth);

  // Returns the root element of the document. Guaranteed to return a valid
  // node for documents created via `Parse()`. Returns an invalid `XmlNodeId` if
  // the document is moved-from, or if the root has been detached.
  XmlNodeId Root() const;

  // Returns the node type of `node_id`, or status error if `node_id` is
  // invalid or not in this document.
  absl::StatusOr<NodeType> GetNodeType(XmlNodeId node_id) const;

  // Returns the tag name if `node_id` is an Element (or target name for
  // ProcessingInstruction), or status error if invalid or not an Element/PI.
  absl::StatusOr<std::string> GetName(XmlNodeId node_id) const;

  // Returns the namespace prefix if `node_id` is an Element, or status error
  // if `node_id` is invalid. Returns an empty string if the element has no
  // namespace prefix.
  absl::StatusOr<std::string> GetPrefix(XmlNodeId node_id) const;

  // Returns the namespace URI if `node_id` is an Element, or status error
  // if `node_id` is invalid. Returns an empty string if the element is not in
  // a namespace.
  absl::StatusOr<std::string> GetNamespace(XmlNodeId node_id) const;

  // Returns the attributes if `node_id` is an Element, or status error if
  // invalid or not an element.
  absl::StatusOr<std::vector<Attribute>> GetAttributes(XmlNodeId node_id) const;

  // Returns the value of the specified attribute matching solely by local name
  // (ignoring prefix and namespace), or a NOT_FOUND status error if the
  // attribute does not exist.
  //
  // Unlike `FindAttributeByName`, which returns `std::nullopt` when an
  // attribute is missing, `GetAttribute` returns an `absl::NotFoundError`.
  //
  // If multiple attributes share the same local name under different
  // namespaces, returns the first matching attribute. Returns an error if
  // `node_id` is invalid or not an element.
  absl::StatusOr<std::string> GetAttribute(XmlNodeId node_id,
                                           absl::string_view name) const;

  // Finds the value of the specified attribute matching solely by local name
  // (ignoring prefix and namespace), returning std::nullopt if the attribute
  // does not exist.
  //
  // Unlike `GetAttribute`, which returns `NOT_FOUND` when an attribute is
  // missing, `FindAttributeByName` returns `std::nullopt`.
  //
  // If multiple attributes share the same local name under different
  // namespaces, returns the first matching attribute. Returns an error if
  // `node_id` is invalid or not an element.
  absl::StatusOr<std::optional<std::string>> FindAttributeByName(
      XmlNodeId node_id, absl::string_view name) const;

  // Finds the value of the specified attribute by name and optional namespace
  // URI, returning std::nullopt if the attribute does not exist. std::nullopt
  // matches attributes without a namespace. Passing an empty string for
  // `namespace_uri` is invalid and returns an INVALID_ARGUMENT error; use
  // std::nullopt to match attributes without a namespace. Returns an error if
  // `node_id` is invalid or not an element.
  absl::StatusOr<std::optional<std::string>> FindAttributeByNameAndNamespace(
      XmlNodeId node_id, absl::string_view name,
      std::optional<absl::string_view> namespace_uri = std::nullopt) const;

  // Returns the child nodes of `node_id`, or status error if `node_id` is
  // invalid or not an element.
  absl::StatusOr<std::vector<XmlNodeId>> GetChildren(XmlNodeId node_id) const;

  // Returns the parent node of `node_id` (nullopt for root), or status error
  // if `node_id` is invalid.
  absl::StatusOr<XmlNodeId> GetParent(XmlNodeId node_id) const;

  // Returns text content for text/comment/cdata nodes (or data portion for
  // PI nodes), or concatenated text of all descendant text/cdata nodes for
  // Element nodes.
  absl::StatusOr<std::string> GetText(XmlNodeId node_id) const;

  // NOTE(b/543934169): Figure out how to support namespaces and prefixes in
  // the API.

  // Finds a direct child element of `node_id` with the specified tag name.
  // Returns nullopt if no such child is found.
  absl::StatusOr<XmlNodeId> FindChildByName(XmlNodeId node_id,
                                            absl::string_view name) const;

  // Adds an existing detached `child` node as a child of `parent`.
  // Returns Ok status on success.
  // Returns `absl::InvalidArgumentError` if `child` is not detached (i.e., it
  // already has a parent) or if `child` is the document root.
  absl::Status AddChild(XmlNodeId parent, XmlNodeId child);

  // Inserts a detached `child` node as a child of `parent`, immediately after
  // `ref_child`.
  // Returns Ok status on success.
  // Returns `absl::InvalidArgumentError` if `child` is not detached (i.e., it
  // already has a parent), if `child` is the document root, or if `ref_child`
  // is not a child of `parent`.
  absl::Status InsertChildAfter(XmlNodeId parent, XmlNodeId child,
                                XmlNodeId ref_child);

  // Detaches `node_id` from its parent, making it a detached node.
  // If `node_id` is the root, the document root is cleared.
  // Returns Ok status on success, or status error if `node_id` is invalid.
  absl::Status DetachNode(XmlNodeId node_id);

  // Deletes `node_id` and all its descendants from the document.
  // If `node_id` has a parent, it is unlinked from its parent.
  // If `node_id` is the root, the document root is cleared.
  // Returns Ok status on success, or status error if `node_id` is invalid.
  absl::Status DeleteNode(XmlNodeId node_id);

  // Sets `node_id` as the root node of the document. `node_id` must be a
  // detached Element node. The previous root node (if any) becomes detached.
  // Returns the new root node handle on success, or status error on failure.
  absl::Status SetRoot(XmlNodeId node_id);

  // Node Creation

  // Creates a new, detached Element node.
  absl::StatusOr<XmlNodeId> CreateElement(absl::string_view name);

  // Creates a new, detached Text node.
  absl::StatusOr<XmlNodeId> CreateTextNode(absl::string_view content);

  // Mutation

  // Sets or updates an attribute on an Element node without a namespace.
  absl::Status SetAttribute(XmlNodeId node_id, absl::string_view key,
                            absl::string_view value);

  // Sets or updates an attribute on an Element node with an optional namespace
  // (URI and prefix). Passing an empty string for either URI or prefix is
  // invalid and returns an INVALID_ARGUMENT error.
  //
  // Example usage:
  //   doc.SetAttribute(node, "type",
  //                    Namespace{.uri = "http://example.com", .prefix = "ext"},
  //                    "val");
  //   doc.SetAttribute(node, "type", Namespace{"http://example.com", "ext"},
  //                    "val");
  //   doc.SetAttribute(node, "type", std::nullopt, "val");
  absl::Status SetAttribute(XmlNodeId node_id, absl::string_view key,
                            std::optional<Namespace> ns,
                            absl::string_view value);

  // Deletes all attributes matching solely by local name (ignoring prefix and
  // namespace) from an Element node. Returns an error if `node_id` is invalid
  // or not an element.
  absl::Status DeleteAttribute(XmlNodeId node_id, absl::string_view key);

  // Deletes an attribute from an Element node matching by local name and
  // optional namespace URI. std::nullopt matches attributes without a
  // namespace. Passing an empty string for `namespace_uri` is invalid and
  // returns an INVALID_ARGUMENT error; use std::nullopt to match attributes
  // without a namespace. Returns an error if `node_id` is invalid or not an
  // element.
  //
  // Example usage:
  //   doc.DeleteAttribute(node, "type", "http://example.com");
  //   doc.DeleteAttribute(node, "type", std::nullopt);
  absl::Status DeleteAttribute(XmlNodeId node_id, absl::string_view key,
                               std::optional<absl::string_view> namespace_uri);

  // Sets the name of an Element node.
  absl::Status SetElementName(XmlNodeId node_id, absl::string_view name);

  // Sets the text content of a Text, Comment, or CData node.
  absl::Status SetText(XmlNodeId node_id, absl::string_view text);

  // Deep copy

  // Creates a deep copy of the node and its descendants. The new node is
  // detached. Returns the new detached node handle on success, or status error
  // on failure.
  absl::StatusOr<XmlNodeId> CloneNode(XmlNodeId node_id);

  // Serialization

  // Serializes the entire document into an XML string.
  absl::StatusOr<std::string> SerializeDocument() const;

  // Serializes a specific node into an XML string.
  absl::StatusOr<std::string> SerializeNode(XmlNodeId node_id) const;

 private:
  explicit XmlDocument(xmltree_rust::XmlDocument doc)
      : doc_(std::move(doc)) {}

  xmltree_rust::XmlDocument doc_;
};

}  // namespace security::xml

#endif  // SECURITY_XML_XMLTREE_H_
