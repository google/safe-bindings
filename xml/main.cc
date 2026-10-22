#include <cstdlib>
#include <iostream>
#include <string>
#include <variant>
#include <vector>

#include "quick_reader.h"
#include "quick_writer.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "xmltree.h"

namespace {

using ::security::xml::QuickReader;
using ::security::xml::QuickWriter;
using ::security::xml::XmlDocument;
using ::security::xml::XmlNodeId;

constexpr absl::string_view kXml =
    "<catalog><book id=\"1\">Dune &amp; Children</book></catalog>";

absl::Status ReadEvents() {
  ASSIGN_OR_RETURN(QuickReader reader, QuickReader::Create(kXml));
  absl::StatusOr<QuickReader::Event> event;
  for (event = reader.NextEvent();
       event.ok() && !std::holds_alternative<QuickReader::Eof>(*event);
       event = reader.NextEvent()) {
    std::cout << security::xml::DescribeEvent(*event) << "\n";
  }
  return event.status();
}

absl::Status WriteDocument() {
  QuickWriter writer;
  RETURN_IF_ERROR(writer.StartElement("greeting"));
  RETURN_IF_ERROR(writer.AddAttribute("lang", "en"));
  RETURN_IF_ERROR(writer.WriteText("Hello & welcome"));
  RETURN_IF_ERROR(writer.EndElement("greeting"));
  ASSIGN_OR_RETURN(std::string xml, writer.Content());
  std::cout << xml << "\n";
  return absl::OkStatus();
}

absl::Status EditTree() {
  ASSIGN_OR_RETURN(XmlDocument doc, XmlDocument::Parse(kXml));
  ASSIGN_OR_RETURN(std::vector<XmlNodeId> books, doc.GetChildren(doc.Root()));
  if (books.empty()) return absl::NotFoundError("no <book> element");
  RETURN_IF_ERROR(doc.SetAttribute(books[0], "lang", "en"));
  ASSIGN_OR_RETURN(std::string xml, doc.SerializeNode(doc.Root()));
  std::cout << xml << "\n";
  return absl::OkStatus();
}

}  // namespace

int main() {
  for (absl::Status (*step)() : {ReadEvents, WriteDocument, EditTree}) {
    if (absl::Status status = step(); !status.ok()) {
      std::cerr << "error: " << status.message() << "\n";
      return EXIT_FAILURE;
    }
  }
  return EXIT_SUCCESS;
}
