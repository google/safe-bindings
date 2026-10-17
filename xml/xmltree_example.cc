#include <array>
#include <iostream>
#include <optional>
#include <string>
#include <vector>

#include "examples.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "xmltree.h"

namespace security::xml::examples {
namespace {

using ::security::xml::Namespace;
using ::security::xml::XmlDocument;
using ::security::xml::XmlNodeId;

absl::Status Print(const XmlDocument& doc, XmlNodeId node) {
  ASSIGN_OR_RETURN(std::string xml, doc.SerializeNode(node));
  std::cout << "  " << xml << "\n";
  return absl::OkStatus();
}

absl::Status ReadAndAdd() {
  ASSIGN_OR_RETURN(
      XmlDocument doc,
      XmlDocument::Parse("<catalog>"
                         "<book id=\"1\"><title>Dune</title></book>"
                         "<book id=\"2\"/>"
                         "</catalog>"));
  XmlNodeId root = doc.Root();

  ASSIGN_OR_RETURN(std::vector<XmlNodeId> children, doc.GetChildren(root));
  for (XmlNodeId child : children) {
    ASSIGN_OR_RETURN(security::xml::NodeType type, doc.GetNodeType(child));
    if (type != security::xml::NodeType::kElement) continue;
    ASSIGN_OR_RETURN(std::string id, doc.GetAttribute(child, "id"));
    ASSIGN_OR_RETURN(XmlNodeId title, doc.FindChildByName(child, "title"));
    std::string title_text = "(none)";
    if (title.IsValid()) {
      ASSIGN_OR_RETURN(title_text, doc.GetText(title));
    }
    std::cout << "  book id=" << id << " title=" << title_text << "\n";
  }

  ASSIGN_OR_RETURN(XmlNodeId book, doc.CreateElement("book"));
  RETURN_IF_ERROR(doc.SetAttribute(book, "id", "3"));
  ASSIGN_OR_RETURN(XmlNodeId title, doc.CreateElement("title"));
  ASSIGN_OR_RETURN(XmlNodeId text, doc.CreateTextNode("Neuromancer"));
  RETURN_IF_ERROR(doc.AddChild(title, text));
  RETURN_IF_ERROR(doc.AddChild(book, title));
  RETURN_IF_ERROR(doc.AddChild(root, book));

  ASSIGN_OR_RETURN(std::string serialized, doc.SerializeNode(root));
  std::cout << "  " << serialized << "\n";
  return Check(serialized ==
                   "<catalog>"
                   "<book id=\"1\"><title>Dune</title></book>"
                   "<book id=\"2\" />"
                   "<book id=\"3\"><title>Neuromancer</title></book>"
                   "</catalog>",
               "catalog with new book");
}

absl::Status EditList() {
  ASSIGN_OR_RETURN(XmlDocument doc,
                   XmlDocument::Parse("<ul><li>one</li></ul>"));
  XmlNodeId list = doc.Root();

  ASSIGN_OR_RETURN(std::vector<XmlNodeId> fragment,
                   doc.ParseFragment("<li>two</li><li>three</li>"));
  for (XmlNodeId item : fragment) RETURN_IF_ERROR(doc.AddChild(list, item));
  std::cout << "  after ParseFragment + AddChild:\n";
  RETURN_IF_ERROR(Print(doc, list));

  ASSIGN_OR_RETURN(std::vector<XmlNodeId> items, doc.GetChildren(list));
  XmlNodeId one = items[0];
  XmlNodeId two = items[1];
  XmlNodeId three = items[2];

  ASSIGN_OR_RETURN(XmlNodeId copy, doc.CloneNode(one));
  ASSIGN_OR_RETURN(std::vector<XmlNodeId> copy_children, doc.GetChildren(copy));
  RETURN_IF_ERROR(doc.SetText(copy_children[0], "one (copy)"));
  RETURN_IF_ERROR(doc.InsertChildAfter(list, copy, one));

  RETURN_IF_ERROR(doc.DetachNode(two));
  RETURN_IF_ERROR(doc.AddChild(list, two));

  RETURN_IF_ERROR(doc.DeleteNode(three));
  RETURN_IF_ERROR(doc.SetElementName(list, "ol"));
  std::cout << "  after CloneNode, InsertChildAfter, DetachNode, DeleteNode, "
               "SetElementName:\n";
  RETURN_IF_ERROR(Print(doc, list));

  absl::StatusOr<std::string> deleted_text = doc.GetText(three);
  std::cout << "  GetText on the deleted node: "
            << (deleted_text.ok()
                    ? *deleted_text
                    : std::string(deleted_text.status().message()))
            << "\n";
  RETURN_IF_ERROR(Check(!deleted_text.ok(), "deleted node is unusable"));

  ASSIGN_OR_RETURN(std::string xml, doc.SerializeNode(list));
  return Check(xml == "<ol><li>one</li><li>one (copy)</li><li>two</li></ol>",
               "edited list");
}

absl::Status ReplaceRoot() {
  ASSIGN_OR_RETURN(XmlDocument doc, XmlDocument::Parse("<body>hi</body>"));
  XmlNodeId body = doc.Root();
  ASSIGN_OR_RETURN(XmlNodeId html, doc.CreateElement("html"));
  RETURN_IF_ERROR(doc.SetRoot(html));
  RETURN_IF_ERROR(doc.AddChild(html, body));

  ASSIGN_OR_RETURN(XmlNodeId parent, doc.GetParent(body));
  ASSIGN_OR_RETURN(std::string parent_name, doc.GetName(parent));
  std::cout << "  parent of <body>: <" << parent_name << ">\n";
  RETURN_IF_ERROR(Print(doc, doc.Root()));
  return Check(parent_name == "html", "new parent");
}

absl::Status Namespaces() {
  ASSIGN_OR_RETURN(
      XmlDocument doc,
      XmlDocument::Parse("<svg:svg xmlns:svg=\"http://www.w3.org/2000/svg\" "
                         "width=\"10\"/>"));
  XmlNodeId svg = doc.Root();
  ASSIGN_OR_RETURN(std::string prefix, doc.GetPrefix(svg));
  ASSIGN_OR_RETURN(std::string uri, doc.GetNamespace(svg));
  std::cout << "  root: prefix=\"" << prefix << "\" namespace=\"" << uri
            << "\"\n";

  RETURN_IF_ERROR(doc.SetAttribute(svg, "href", std::nullopt, "plain.png"));
  RETURN_IF_ERROR(doc.SetAttribute(
      svg, "href",
      Namespace{.uri = "http://www.w3.org/1999/xlink", .prefix = "xlink"},
      "linked.png"));
  ASSIGN_OR_RETURN(std::optional<std::string> plain,
                   doc.FindAttributeByNameAndNamespace(svg, "href"));
  ASSIGN_OR_RETURN(std::optional<std::string> linked,
                   doc.FindAttributeByNameAndNamespace(
                       svg, "href", "http://www.w3.org/1999/xlink"));
  std::cout << "  href without namespace: " << plain.value_or("(none)") << "\n"
            << "  href in xlink namespace: " << linked.value_or("(none)")
            << "\n";
  RETURN_IF_ERROR(Check(plain == "plain.png", "plain href"));
  RETURN_IF_ERROR(Check(linked == "linked.png", "xlink href"));

  ASSIGN_OR_RETURN(std::optional<std::string> height,
                   doc.FindAttributeByName(svg, "height"));
  absl::StatusOr<std::string> missing_height = doc.GetAttribute(svg, "height");
  std::cout << "  FindAttributeByName(height): "
            << height.value_or("std::nullopt") << "\n"
            << "  GetAttribute(height): "
            << absl::StatusCodeToString(missing_height.status().code()) << "\n";
  RETURN_IF_ERROR(Check(!height.has_value(), "FindAttributeByName"));
  RETURN_IF_ERROR(Check(absl::IsNotFound(missing_height.status()),
                        "GetAttribute NOT_FOUND"));

  RETURN_IF_ERROR(
      doc.DeleteAttribute(svg, "href", "http://www.w3.org/1999/xlink"));
  ASSIGN_OR_RETURN(std::vector<security::xml::Attribute> attributes,
                   doc.GetAttributes(svg));
  std::cout << "  attributes after deleting xlink:href:";
  for (const security::xml::Attribute& attribute : attributes) {
    std::cout << " " << attribute.prefix.value_or("")
              << (attribute.prefix.has_value() ? ":" : "") << attribute.key
              << "=\"" << attribute.value << "\"";
  }
  std::cout << "\n";
  return Check(attributes.size() == 2, "remaining attributes");
}

}  // namespace

bool RunXmlTreeExample() {
  const auto sections = std::to_array<Section>({
      {"Reading and adding nodes", ReadAndAdd},
      {"Editing a list", EditList},
      {"Replacing the root", ReplaceRoot},
      {"Namespaces and attributes", Namespaces},
  });
  return RunSections("XmlDocument", sections);
}

}  // namespace security::xml::examples
