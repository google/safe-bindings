#include <iostream>
#include <string>
#include <string_view>

#include "serde_json.h"
#include "serde_json_ignore_comments.h"

int main(int argc, char** argv) {
  std::string json_string = R"({
    // Example JSON with a comment
    "name": "John Doe",
    "age": 30,
    "is_student": false,
    "scores": [85, 92, 78]
  })";

  auto json_or = rust::ParseIgnoreComments(json_string);
  if (!json_or.has_value()) {
    std::cerr << "Failed to parse JSON: "
              << std::string_view(json_or.err().as_str()) << std::endl;
    return 1;
  }
  auto& json = *json_or;

  std::cout << "Parsed JSON: "
            << std::string_view(
                   rust::ToString(json).as_str())
            << std::endl;

  if (auto name = json["name"].as_str(); name.has_value()) {
    std::cout << "Name: " << std::string_view(*name) << std::endl;
  }

  if (auto age = json["age"].as_int64(); age.has_value()) {
    std::cout << "Age: " << *age << std::endl;
  }

  if (auto is_student = json["is_student"].as_bool(); is_student.has_value()) {
    std::cout << "Is student: " << (*is_student ? "yes" : "no") << std::endl;
  }

  if (json["scores"].is_array()) {
    std::cout << "Scores: ";
    for (const auto* score_json : json["scores"]) {
      if (auto score = score_json->as_int64(); score.has_value()) {
        std::cout << *score << " ";
      }
    }
    std::cout << std::endl;
  }

  return 0;
}
