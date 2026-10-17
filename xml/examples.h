#ifndef THIRD_PARTY_SAFE_BINDINGS_XML_EXAMPLES_H_
#define THIRD_PARTY_SAFE_BINDINGS_XML_EXAMPLES_H_

#include <iostream>

#include "absl/status/status.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "absl/types/span.h"

namespace security::xml::examples {

bool RunQuickReaderExample();
bool RunQuickWriterExample();
bool RunXmlTreeExample();

struct Section {
  absl::string_view title;
  absl::Status (*run)();
};

inline absl::Status Check(bool condition, absl::string_view message) {
  if (condition) return absl::OkStatus();
  return absl::InternalError(absl::StrCat("check failed: ", message));
}

inline bool RunSections(absl::string_view example,
                        absl::Span<const Section> sections) {
  bool ok = true;
  for (const Section& section : sections) {
    std::cout << "=== " << example << ": " << section.title << " ===\n";
    absl::Status status = section.run();
    if (!status.ok()) {
      std::cout << "  FAILED: " << status.message() << "\n";
      ok = false;
    }
    std::cout << "\n";
  }
  return ok;
}

}  // namespace security::xml::examples

#endif  // THIRD_PARTY_SAFE_BINDINGS_XML_EXAMPLES_H_
