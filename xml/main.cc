#include <array>
#include <cstdlib>
#include <iostream>
#include <iterator>
#include <string>
#include <vector>

#include "examples.h"
#include "absl/strings/string_view.h"

namespace {

using ::security::xml::examples::RunQuickReaderExample;
using ::security::xml::examples::RunQuickWriterExample;
using ::security::xml::examples::RunXmlTreeExample;

struct Example {
  absl::string_view name;
  bool (*run)();
};

constexpr auto kExamples = std::to_array<Example>({
    {"quick_reader", RunQuickReaderExample},
    {"quick_writer", RunQuickWriterExample},
    {"xmltree", RunXmlTreeExample},
});

const Example* FindExample(absl::string_view name) {
  for (const Example& example : kExamples) {
    if (example.name == name) return &example;
  }
  return nullptr;
}

}  // namespace

int main(int argc, char** argv) {
  std::vector<const Example*> selected;
  for (int i = 1; i < argc; ++i) {
    const Example* example = FindExample(argv[i]);
    if (example == nullptr) {
      std::cerr << "Unknown example: " << argv[i] << "\nAvailable:";
      for (const Example& e : kExamples) std::cerr << " " << e.name;
      std::cerr << "\n";
      return EXIT_FAILURE;
    }
    selected.push_back(example);
  }
  if (selected.empty()) {
    selected.reserve(std::size(kExamples));
    for (const Example& example : kExamples) selected.push_back(&example);
  }

  bool ok = true;
  for (const Example* example : selected) ok &= example->run();
  return ok ? EXIT_SUCCESS : EXIT_FAILURE;
}
