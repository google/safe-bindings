#ifndef SECURITY_XML_QUICK_WRITER_H_
#define SECURITY_XML_QUICK_WRITER_H_

#include <cstddef>
#include <string>

#include "quick_reader.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"

namespace security::xml {

// QuickWriter wraps the Rust quick-xml Writer in an idiomatic C++ interface.
// It is the counterpart to QuickReader and supports both writing events parsed
// by QuickReader and streaming XML generation.
//
// For simplicity, it restricts support for some features (e.g., non-UTF-8
// encodings, doc version, etc.). Those can be added if needed.
class QuickWriter {
 public:
  // LINT.IfChange(WriterConfig)
  struct Config {
    // If true, adds a space before the slash in empty elements (<tag />).
    bool add_space_before_slash_in_empty_elements = false;
    // If true, indents nested elements.
    bool perform_indent = false;
    // Character used for indentation when perform_indent is true.
    char indent_char = ' ';
    // Number of indent characters per level.
    size_t indent_size = 2;
  };
  // LINT.ThenChange(
  //     rust/quick_writer.rs:WriterConfig,
  //     quick_writer.cc:WriterConfig
  // )

  QuickWriter();
  explicit QuickWriter(Config config);

  // Type-compatible API with QuickReader::Event.
  // Consumes a QuickReader event and emits the corresponding XML output.
  absl::Status WriteEvent(QuickReader::Event& event);

  // Streaming API.
  absl::Status StartDocument();
  absl::Status StartElement(absl::string_view name);
  absl::Status AddAttribute(absl::string_view name, absl::string_view value);
  absl::Status EndElement(absl::string_view name);
  absl::Status WriteEmptyElement(absl::string_view name);
  absl::Status WriteText(absl::string_view content);
  absl::Status WriteCData(absl::string_view content);
  absl::Status WriteComment(absl::string_view comment);
  absl::Status WriteProcessingInstruction(absl::string_view content);
  absl::Status WriteGeneralReference(absl::string_view name);

  // Returns the written XML document string buffer.
  absl::StatusOr<std::string> Content();

 private:
  rust::QuickWriter rust_writer_;
};

}  // namespace security::xml

#endif  // SECURITY_XML_QUICK_WRITER_H_
