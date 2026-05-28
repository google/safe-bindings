#include <iostream>
#include <ostream>
#include <string>
#include <utility>

#include "node.h"

int main(int argc, char** argv) {
  std::string yaml_content = "name: Saphyr Example\nvalues: [1, 2, 3]";
  std::cout << "Parsing YAML:\n" << yaml_content << "\n" << std::endl;

  auto node = security::yaml::Load(yaml_content);
  if (!node.ok()) {
    std::cerr << "Error loading YAML: " << node.status() << std::endl;
    return 1;
  }

  std::cout << "Parsed successfully!" << std::endl;
  std::cout << "Root is map: " << (node->IsMap() ? "true" : "false")
            << std::endl;

  auto name_node = node->Get("name");
  if (name_node.has_value()) {
    std::cout << "name: "
              << name_node->as_optional<std::string_view>().value_or("")
              << std::endl;
  }

  auto values_node = node->Get("values");
  if (values_node.has_value() && values_node->IsSequence()) {
    std::cout << "values size: " << values_node->size() << std::endl;
    for (security::yaml::NodeView elem : *values_node) {
      std::cout << "- " << elem.as_optional<int64_t>().value_or(0) << std::endl;
    }
  }

  return 0;
}
