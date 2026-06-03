#include <iostream>
#include <string>

#include "serde_json_bridge.h"

int main(int argc, char** argv) {
  std::string json_string = R"({
    "name": "John Doe",
    "age": 30,
    "is_student": false,
    "scores": [85, 92, 78]
  })";

  auto json_or =
      security::json::serde_json_bridge::SerdeJson::Parse(json_string);
  if (!json_or.ok()) {
    std::cerr << "Failed to parse JSON: " << json_or.status() << std::endl;
    return 1;
  }
  auto& json = *json_or;

  std::cout << "Parsed JSON: " << json.ToString() << std::endl;

  auto name = json.GetFieldString("name");
  if (name.ok()) {
    std::cout << "Name: " << *name << std::endl;
  }

  auto age = json.GetFieldInt("age");
  if (age.ok()) {
    std::cout << "Age: " << *age << std::endl;
  }

  auto is_student = json.GetFieldBool("is_student");
  if (is_student.ok()) {
    std::cout << "Is student: " << (*is_student ? "yes" : "no") << std::endl;
  }

  auto scores_or = json.GetFieldArray("scores");
  if (scores_or.ok()) {
    std::cout << "Scores: ";
    for (const auto& score_json : *scores_or) {
      auto score = score_json.GetInt();
      if (score.ok()) {
        std::cout << *score << " ";
      }
    }
    std::cout << std::endl;
  }

  return 0;
}
