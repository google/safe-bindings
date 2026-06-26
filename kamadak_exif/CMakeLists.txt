cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(kamadak-exif
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

add_library(crubit_helpers INTERFACE)
target_include_directories(crubit_helpers INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(crubit_helpers INTERFACE absl::strings absl::span)

add_library(exif_bridge exif_bridge.cc exif_bridge.h exif_reader.cc exif_reader.h)
target_include_directories(exif_bridge PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(exif_bridge PRIVATE rust crubit_helpers absl::status absl::statusor absl::strings absl::span)

add_executable(kamadak_exif_example main.cc)
target_link_libraries(kamadak_exif_example PRIVATE rust exif_bridge absl::statusor absl::strings)
