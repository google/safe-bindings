cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(deflate
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

add_library(flate2 flate2.cc flate2.h)
target_include_directories(flate2 PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(flate2 PRIVATE rust absl::status absl::statusor absl::cord absl::strings absl::span)

add_library(gzip_wrapper gzip_wrapper.cc gzip_wrapper.h)
target_link_libraries(gzip_wrapper PRIVATE flate2 rust absl::status absl::statusor absl::strings absl::span)

add_executable(deflate_example main.cc)
target_link_libraries(deflate_example PRIVATE rust flate2 gzip_wrapper absl::statusor absl::strings)
