#include <array>
#include <iostream>
#include <optional>
#include <string>
#include <variant>

#include "examples.h"
#include "quick_reader.h"
#include "absl/functional/overload.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/ascii.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"

namespace security::xml::examples {
namespace {

using ::security::xml::DescribeEvent;
using ::security::xml::GetXmlEncoding;
using ::security::xml::QuickReader;

constexpr absl::string_view kCatalog =
    "<?xml version=\"1.0\"?>\n"
    "<catalog>\n"
    "  <!-- two books -->\n"
    "  <book id=\"1\" lang=\"en\">\n"
    "    <title>Dune &amp; Children</title>\n"
    "    <note><![CDATA[<not a tag>]]></note>\n"
    "  </book>\n"
    "  <book id=\"2\" lang=\"fr\"/>\n"
    "</catalog>\n";

std::string OrNone(std::optional<absl::string_view> value) {
  return value.has_value() ? absl::StrCat("\"", *value, "\"") : "(none)";
}

absl::Status PrintAttributes(QuickReader::Attributes& attributes) {
  absl::StatusOr<std::optional<QuickReader::Attribute>> attribute;
  for (attribute = attributes.NextAttribute();
       attribute.ok() && attribute->has_value();
       attribute = attributes.NextAttribute()) {
    std::cout << "    attr    " << (*attribute)->QualifiedName() << "=\""
              << (*attribute)->Value() << "\"\n";
  }
  return attribute.status();
}

absl::Status PrintEvent(QuickReader::Event& event) {
  return std::visit(
      absl::Overload{
          [](QuickReader::Decl& decl) -> absl::Status {
            std::cout << "  decl    <?" << decl.data.Content() << "?>\n";
            return absl::OkStatus();
          },
          [](QuickReader::Start& start) -> absl::Status {
            std::cout << "  start   <" << start.name.QualifiedName() << ">"
                      << (start.self_closing ? " (self-closing)" : "") << "\n";
            RETURN_IF_ERROR(PrintAttributes(start.attributes));
            return absl::OkStatus();
          },
          [](QuickReader::End& end) -> absl::Status {
            std::cout << "  end     </" << end.name.QualifiedName() << ">\n";
            return absl::OkStatus();
          },
          [](QuickReader::Text& text) -> absl::Status {
            absl::string_view content =
                absl::StripAsciiWhitespace(text.data.Content());
            if (!content.empty()) {
              std::cout << "  text    \"" << content << "\"\n";
            }
            return absl::OkStatus();
          },
          [](QuickReader::CData& cdata) -> absl::Status {
            std::cout << "  cdata   \"" << cdata.data.Content() << "\"\n";
            return absl::OkStatus();
          },
          [](QuickReader::Comment& comment) -> absl::Status {
            std::cout << "  comment \""
                      << absl::StripAsciiWhitespace(comment.data.Content())
                      << "\"\n";
            return absl::OkStatus();
          },
          [](QuickReader::PI& pi) -> absl::Status {
            std::cout << "  pi      <?" << pi.data.Content() << "?>\n";
            return absl::OkStatus();
          },
          [](QuickReader::DocType& doctype) -> absl::Status {
            std::cout << "  doctype " << doctype.data.Content() << "\n";
            return absl::OkStatus();
          },
          [](QuickReader::GeneralRef& ref) -> absl::Status {
            std::cout << "  ref     &" << ref.data.Content() << "; -> \""
                      << ref.expanded << "\"\n";
            return absl::OkStatus();
          },
          [](QuickReader::Eof) -> absl::Status {
            std::cout << "  eof\n";
            return absl::OkStatus();
          },
      },
      event);
}

absl::Status PrintEvents(absl::string_view xml, QuickReader::Config config) {
  ASSIGN_OR_RETURN(QuickReader reader,
                   QuickReader::CreateWithConfig(xml, config));
  absl::StatusOr<QuickReader::Event> event;
  for (event = reader.NextEvent();
       event.ok() && !std::holds_alternative<QuickReader::Eof>(*event);
       event = reader.NextEvent()) {
    RETURN_IF_ERROR(PrintEvent(*event));
  }
  RETURN_IF_ERROR(event.status());
  return PrintEvent(*event);
}

absl::Status WalkDocument() {
  return PrintEvents(kCatalog, QuickReader::Config{});
}

absl::Status FirstElement() {
  ASSIGN_OR_RETURN(QuickReader reader,
                   QuickReader::Create("<user name=\"ada\" role=\"admin\"/>"));
  ASSIGN_OR_RETURN(QuickReader::Event event, reader.NextEvent());
  auto* start = std::get_if<QuickReader::Start>(&event);
  RETURN_IF_ERROR(Check(start != nullptr, "expected a start tag"));
  std::cout << "  element " << start->name.LocalName() << "\n";
  return PrintAttributes(start->attributes);
}

absl::Status MalformedInput() {
  absl::Status status =
      PrintEvents("<catalog><book></catalog>",
                  QuickReader::Config{.xml_substrings = false});
  std::cout << "  error: " << status.message() << "\n";
  return Check(!status.ok(), "malformed input is rejected");
}

absl::Status Namespaces() {
  static constexpr absl::string_view kXml =
      "<feed xmlns=\"http://www.w3.org/2005/Atom\""
      "      xmlns:media=\"http://search.yahoo.com/mrss/\">"
      "<media:thumbnail url=\"cover.png\" media:width=\"75\"/>"
      "</feed>";
  ASSIGN_OR_RETURN(QuickReader reader, QuickReader::Create(kXml));
  absl::StatusOr<QuickReader::Event> event;
  for (event = reader.NextEvent();
       event.ok() && !std::holds_alternative<QuickReader::Eof>(*event);
       event = reader.NextEvent()) {
    auto* start = std::get_if<QuickReader::Start>(&*event);
    if (start == nullptr) continue;

    ASSIGN_OR_RETURN(absl::string_view element_ns,
                     start->name.ResolvedNamespace());
    std::cout << "  element " << start->name.QualifiedName()
              << ": local=" << start->name.LocalName()
              << " prefix=" << OrNone(start->name.NamespacePrefix())
              << " namespace=\"" << element_ns << "\"\n";
    if (start->name.LocalName() == "thumbnail") {
      RETURN_IF_ERROR(Check(element_ns == "http://search.yahoo.com/mrss/",
                            "media:thumbnail namespace"));
    }

    absl::StatusOr<std::optional<QuickReader::Attribute>> attribute;
    for (attribute = start->attributes.NextAttribute();
         attribute.ok() && attribute->has_value();
         attribute = start->attributes.NextAttribute()) {
      QuickReader::Attribute& attr = **attribute;
      std::string attribute_ns = "(none)";
      if (attr.NamespacePrefix().has_value()) {
        ASSIGN_OR_RETURN(absl::string_view resolved, attr.ResolvedNamespace());
        attribute_ns = absl::StrCat("\"", resolved, "\"");
      }
      std::cout << "    attr  " << attr.QualifiedName()
                << ": local=" << attr.LocalName()
                << " prefix=" << OrNone(attr.NamespacePrefix())
                << " namespace=" << attribute_ns << " value=\"" << attr.Value()
                << "\"\n";
      if (attr.LocalName() == "width") {
        RETURN_IF_ERROR(
            Check(attribute_ns == "\"http://search.yahoo.com/mrss/\"",
                  "media:width namespace"));
      }
    }
    RETURN_IF_ERROR(attribute.status());
  }
  return event.status();
}

absl::Status NameValidity() {
  static constexpr absl::string_view kXml = "<a:b:c/>";
  ASSIGN_OR_RETURN(QuickReader reader,
                   QuickReader::CreateWithConfig(
                       kXml, QuickReader::Config{.ignore_namespaces = true}));
  ASSIGN_OR_RETURN(QuickReader::Event event, reader.NextEvent());
  auto* start = std::get_if<QuickReader::Start>(&event);
  RETURN_IF_ERROR(Check(start != nullptr, "expected a start tag"));
  std::cout << "  <" << start->name.QualifiedName()
            << ">: LocalNameValid=" << start->name.LocalNameValid()
            << " PrefixValid=" << start->name.PrefixValid() << "\n";
  RETURN_IF_ERROR(Check(!start->name.LocalNameValid(), "local name invalid"));
  RETURN_IF_ERROR(Check(start->name.PrefixValid(), "prefix valid"));
  return absl::OkStatus();
}

absl::Status Substrings() {
  static constexpr absl::string_view kXml =
      "<p class=\"intro\">Hello <b>world</b></p>";
  ASSIGN_OR_RETURN(QuickReader reader, QuickReader::Create(kXml));
  ASSIGN_OR_RETURN(QuickReader::Event event, reader.NextEvent());
  auto* start = std::get_if<QuickReader::Start>(&event);
  RETURN_IF_ERROR(Check(start != nullptr, "expected a start tag"));
  std::cout << "  inner_xml: " << OrNone(start->inner_xml) << "\n"
            << "  outer_xml: " << OrNone(start->outer_xml) << "\n";
  RETURN_IF_ERROR(Check(start->inner_xml == "Hello <b>world</b>", "inner_xml"));
  RETURN_IF_ERROR(Check(start->outer_xml == kXml, "outer_xml"));
  return absl::OkStatus();
}

absl::Status DescribeAll(absl::string_view xml, QuickReader::Config config) {
  ASSIGN_OR_RETURN(QuickReader reader,
                   QuickReader::CreateWithConfig(xml, config));
  absl::StatusOr<QuickReader::Event> event;
  for (event = reader.NextEvent();
       event.ok() && !std::holds_alternative<QuickReader::Eof>(*event);
       event = reader.NextEvent()) {
    std::cout << "    " << DescribeEvent(*event);
    if (auto* start = std::get_if<QuickReader::Start>(&*event)) {
      absl::StatusOr<std::optional<QuickReader::Attribute>> attribute;
      for (attribute = start->attributes.NextAttribute();
           attribute.ok() && attribute->has_value();
           attribute = start->attributes.NextAttribute()) {
        std::cout << " " << (*attribute)->QualifiedName() << "=\""
                  << (*attribute)->Value() << "\"";
      }
      RETURN_IF_ERROR(attribute.status());
    }
    std::cout << "\n";
  }
  RETURN_IF_ERROR(event.status());
  std::cout << "    " << DescribeEvent(*event) << "\n";
  return absl::OkStatus();
}

absl::Status ReaderOptions() {
  static constexpr absl::string_view kXml =
      "<doc xmlns:x=\"urn:x\"><x:br/></doc>";
  std::cout << "  defaults:\n";
  RETURN_IF_ERROR(DescribeAll(kXml, QuickReader::Config{}));
  std::cout << "  expand_empty_elements = true:\n";
  RETURN_IF_ERROR(
      DescribeAll(kXml, QuickReader::Config{.expand_empty_elements = true}));
  std::cout << "  yield_xmlns_attributes = true:\n";
  RETURN_IF_ERROR(
      DescribeAll(kXml, QuickReader::Config{.yield_xmlns_attributes = true}));
  return absl::OkStatus();
}

absl::Status Encoding() {
  static constexpr absl::string_view kLatin1 =
      "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a/>";
  static constexpr absl::string_view kUndeclared = "<a/>";
  std::optional<std::string> latin1_encoding = GetXmlEncoding(kLatin1);
  std::optional<std::string> undeclared_encoding = GetXmlEncoding(kUndeclared);
  std::cout << "  " << kLatin1 << " -> " << latin1_encoding.value_or("(none)")
            << "\n"
            << "  " << kUndeclared << " -> "
            << undeclared_encoding.value_or("(none)") << "\n";
  RETURN_IF_ERROR(Check(latin1_encoding == "ISO-8859-1", "declared encoding"));
  RETURN_IF_ERROR(
      Check(!undeclared_encoding.has_value(), "no declared encoding"));
  return absl::OkStatus();
}

}  // namespace

bool RunQuickReaderExample() {
  const auto sections = std::to_array<Section>({
      {"Walking a document", WalkDocument},
      {"Reading attributes", FirstElement},
      {"Malformed input", MalformedInput},
      {"Namespaces", Namespaces},
      {"Name validity", NameValidity},
      {"Inner and outer XML", Substrings},
      {"Reader options", ReaderOptions},
      {"Declared encoding", Encoding},
  });
  return RunSections("QuickReader", sections);
}

}  // namespace security::xml::examples
