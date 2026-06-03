cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(serde_json
  VERSION 0.1.0
  LANGUAGES CXX)

include(FetchContent)

FetchContent_Declare(
  Corrosion
  # NOTE: Replace with https://github.com/corrosion-rs/corrosion once
  # http://github.com/corrosion-rs/corrosion/pull/666 is merged.
  GIT_REPOSITORY https://github.com/thunderseethe/corrosion
  GIT_TAG master
)

FetchContent_MakeAvailable(Corrosion)

corrosion_experimental_crubit(rust)
corrosion_import_crate(MANIFEST_PATH rust/Cargo.toml)

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

find_package(Protobuf REQUIRED)

add_library(serde_json_bridge serde_json_bridge.cc serde_json_bridge.h)
target_link_libraries(serde_json_bridge PUBLIC
  absl::status
  absl::statusor
  absl::strings
  absl::span
  protobuf::libprotobuf
)
target_link_libraries(serde_json_bridge PRIVATE
  rust
)

# Include the root directory for crubit_helpers/string_conversions.h
target_include_directories(serde_json_bridge PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/..)

add_executable(serde_json_example main.cc)
target_link_libraries(serde_json_example PRIVATE
  serde_json_bridge
  rust
)
