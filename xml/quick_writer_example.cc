#include <array>
#include <iostream>
#include <string>
#include <variant>

#include "examples.h"
#include "quick_reader.h"
#include "quick_writer.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::xml::examples {
namespace {

using ::security::xml::QuickReader;
using ::security::xml::QuickWriter;

absl::Status StreamingApi() {
  QuickWriter writer(QuickWriter::Config{.perform_indent = true});
  RETURN_IF_ERROR(writer.StartDocument());
  RETURN_IF_ERROR(writer.StartElement("order"));
  RETURN_IF_ERROR(writer.AddAttribute("id", "42"));
  RETURN_IF_ERROR(writer.StartElement("item"));
  RETURN_IF_ERROR(writer.AddAttribute("sku", "A-1"));
  RETURN_IF_ERROR(writer.WriteText("Salt & Pepper <large>"));
  RETURN_IF_ERROR(writer.EndElement("item"));
  RETURN_IF_ERROR(writer.WriteEmptyElement("gift-wrap"));
  RETURN_IF_ERROR(writer.EndElement("order"));
  ASSIGN_OR_RETURN(std::string output, writer.Content());
  std::cout << output << "\n";
  return Check(
      output ==
          "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
          "<order id=\"42\">\n"
          "  <item sku=\"A-1\">Salt &amp; Pepper &lt;large&gt;</item>\n"
          "  <gift-wrap/>\n"
          "</order>",
      "written document");
}

absl::StatusOr<std::string> Redact(absl::string_view xml) {
  ASSIGN_OR_RETURN(QuickReader reader, QuickReader::Create(xml));
  QuickWriter writer;
  bool in_password = false;
  absl::StatusOr<QuickReader::Event> event;
  for (event = reader.NextEvent();
       event.ok() && !std::holds_alternative<QuickReader::Eof>(*event);
       event = reader.NextEvent()) {
    if (std::holds_alternative<QuickReader::Comment>(*event)) continue;

    if (auto* start = std::get_if<QuickReader::Start>(&*event)) {
      in_password = start->name.LocalName() == "password";
    } else if (std::holds_alternative<QuickReader::End>(*event)) {
      in_password = false;
    } else if (in_password &&
               std::holds_alternative<QuickReader::Text>(*event)) {
      RETURN_IF_ERROR(writer.WriteText("***"));
      continue;
    }
    RETURN_IF_ERROR(writer.WriteEvent(*event));
  }
  RETURN_IF_ERROR(event.status());
  return writer.Content();
}

absl::Status RoundTrip() {
  static constexpr absl::string_view kInput =
      "<?xml version=\"1.0\"?>"
      "<login><!-- from the signup form -->"
      "<user role=\"admin\">ada</user>"
      "<password>hunter2</password>"
      "</login>";
  ASSIGN_OR_RETURN(std::string output, Redact(kInput));
  std::cout << "  input:  " << kInput << "\n"
            << "  output: " << output << "\n";
  return Check(output ==
                   "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
                   "<login>"
                   "<user role=\"admin\">ada</user>"
                   "<password>***</password>"
                   "</login>",
               "redacted output");
}

absl::Status OtherNodes() {
  QuickWriter writer(
      QuickWriter::Config{.add_space_before_slash_in_empty_elements = true});
  RETURN_IF_ERROR(
      writer.WriteProcessingInstruction("xml-stylesheet href=\"style.css\""));
  RETURN_IF_ERROR(writer.StartElement("page"));
  RETURN_IF_ERROR(writer.WriteComment(" generated "));
  RETURN_IF_ERROR(writer.StartElement("script"));
  RETURN_IF_ERROR(writer.WriteCData("if (a < b) run();"));
  RETURN_IF_ERROR(writer.EndElement("script"));
  RETURN_IF_ERROR(writer.StartElement("p"));
  RETURN_IF_ERROR(writer.WriteText("Copyright "));
  RETURN_IF_ERROR(writer.WriteGeneralReference("copy"));
  RETURN_IF_ERROR(writer.EndElement("p"));
  RETURN_IF_ERROR(writer.WriteEmptyElement("hr"));
  RETURN_IF_ERROR(writer.EndElement("page"));
  ASSIGN_OR_RETURN(std::string output, writer.Content());
  std::cout << "  " << output << "\n";
  return Check(output ==
                   "<?xml-stylesheet href=\"style.css\"?>"
                   "<page><!-- generated -->"
                   "<script><![CDATA[if (a < b) run();]]></script>"
                   "<p>Copyright &copy;</p>"
                   "<hr /></page>",
               "written output");
}

}  // namespace

bool RunQuickWriterExample() {
  const auto sections = std::to_array<Section>({
      {"Streaming API", StreamingApi},
      {"Read, modify, and write back", RoundTrip},
      {"Other node types and options", OtherNodes},
  });
  return RunSections("QuickWriter", sections);
}

}  // namespace security::xml::examples
