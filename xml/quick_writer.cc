#include "quick_writer.h"

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "crubit/support/rs_std/slice_ref.h"
#include "crubit_helpers/string_conversions.h"
#include "quick_reader.h"
#include "crubit/rust.h"
#include "absl/functional/overload.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::xml {

namespace {

using ::security::crubit_helpers::StringViewFromVecU8;
using WriteResult = rs_std::Result<bool, rust::WriterError>;

rs_std::SliceRef<const uint8_t> SliceFromView(absl::string_view s) {
  return rs_std::SliceRef<const uint8_t>(absl::Span<const uint8_t>(
      reinterpret_cast<const uint8_t*>(s.data()), s.size()));
}

// NOTE(b/535961534): Derive the correct status code later instead of always
// returning InvalidArgumentError.
absl::Status WriterErrorToStatus(const rust::WriterError& err) {
  return absl::InvalidArgumentError(StringViewFromVecU8(err.message));
}

rust::QuickWriterConfig ToRustConfig(
    const QuickWriter::Config& config) {
  rust::QuickWriterConfig rust_config;
  // LINT.IfChange(WriterConfig)
  rust_config.add_space_before_slash_in_empty_elements =
      config.add_space_before_slash_in_empty_elements;
  rust_config.perform_indent = config.perform_indent;
  rust_config.indent_char = static_cast<uint8_t>(config.indent_char);
  rust_config.indent_size = config.indent_size;
  // LINT.ThenChange(
  //     rust/quick_writer.rs:WriterConfig,
  //     quick_writer.h:WriterConfig
  // )
  return rust_config;
}

}  // namespace

QuickWriter::QuickWriter() : QuickWriter(Config()) {}

QuickWriter::QuickWriter(Config config)
    : rust_writer_(
          rust::QuickWriter::new_with_config(ToRustConfig(config))) {}

absl::Status QuickWriter::StartDocument() {
  WriteResult res = rust_writer_.write_start_document();
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::StartElement(absl::string_view name) {
  WriteResult res = rust_writer_.write_start_element(SliceFromView(name));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::AddAttribute(absl::string_view name,
                                       absl::string_view value) {
  WriteResult res =
      rust_writer_.write_attribute(SliceFromView(name), SliceFromView(value));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::EndElement(absl::string_view name) {
  WriteResult res = rust_writer_.write_end_element(SliceFromView(name));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteEmptyElement(absl::string_view name) {
  WriteResult res = rust_writer_.write_empty_element(SliceFromView(name));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteText(absl::string_view content) {
  WriteResult res = rust_writer_.write_characters(SliceFromView(content));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteCData(absl::string_view content) {
  WriteResult res = rust_writer_.write_cdata(SliceFromView(content));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteComment(absl::string_view comment) {
  WriteResult res = rust_writer_.write_comment(SliceFromView(comment));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteProcessingInstruction(
    absl::string_view content) {
  WriteResult res =
      rust_writer_.write_processing_instruction(SliceFromView(content));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteGeneralReference(absl::string_view name) {
  WriteResult res = rust_writer_.write_general_reference(SliceFromView(name));
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  return absl::OkStatus();
}

absl::Status QuickWriter::WriteEvent(QuickReader::Event& event) {
  return std::visit(
      absl::Overload{
          [this](QuickReader::Decl& decl) -> absl::Status {
            // Ignores version, encoding, and the standalone flag for
            // simplicity.
            return StartDocument();
          },
          [this](QuickReader::Start& start) -> absl::Status {
            absl::string_view qname = start.name.QualifiedName();
            absl::Status s = StartElement(qname);
            if (!s.ok()) return s;
            while (true) {
              absl::StatusOr<std::optional<QuickReader::Attribute>> attr =
                  start.attributes.NextAttribute();
              if (!attr.ok()) return attr.status();
              if (!attr->has_value()) break;
              s = AddAttribute((**attr).QualifiedName(), (**attr).Value());
              if (!s.ok()) return s;
            }
            if (start.self_closing) {
              // If the QuickReader parsed this as a self-closing tag (e.g.,
              // <a/>), we need to close the element. The current Rust writer
              // API does not provide a way to write a self-closing tag with
              // attributes after calling StartElement and AddAttribute. Calling
              // EndElement will result in <a></a> instead of <a/>, but it
              // ensures the XML remains well-formed. To achieve <a/>, the
              // underlying Rust quick_writer would need enhancement.
              return EndElement(qname);
            }
            return absl::OkStatus();
          },
          [this](QuickReader::End& end) -> absl::Status {
            return EndElement(end.name.QualifiedName());
          },
          [this](QuickReader::Text& text) -> absl::Status {
            return WriteText(text.data.Content());
          },
          [this](QuickReader::CData& cdata) -> absl::Status {
            return WriteCData(cdata.data.Content());
          },
          [this](QuickReader::Comment& comment) -> absl::Status {
            return WriteComment(comment.data.Content());
          },
          [this](QuickReader::PI& pi) -> absl::Status {
            return WriteProcessingInstruction(pi.data.Content());
          },
          [](QuickReader::DocType&) -> absl::Status {
            return absl::OkStatus();
          },
          [this](QuickReader::GeneralRef& ref) -> absl::Status {
            return WriteGeneralReference(ref.data.Content());
          },
          [](QuickReader::Eof) -> absl::Status { return absl::OkStatus(); },
      },
      event);
}

absl::StatusOr<std::string> QuickWriter::Content() {
  rs_std::Result<rs_std::SliceRef<const uint8_t>, rust::WriterError>
      res = rust_writer_.content();
  if (!res.has_value()) {
    return WriterErrorToStatus(std::move(res).err());
  }
  rs_std::SliceRef<const uint8_t> slice = std::move(res).value();
  if (slice.size() == 0) {
    return std::string();
  }
  return std::string(reinterpret_cast<const char*>(slice.data()), slice.size());
}

}  // namespace security::xml
