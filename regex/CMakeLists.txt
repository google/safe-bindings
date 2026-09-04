cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(regex
  VERSION 0.1.0
  LANGUAGES CXX)

set(CMAKE_SKIP_INSTALL_RULES ON)
include(FetchContent)

FetchContent_Declare(
  Corrosion
  # NOTE: Replace with https://github.com/corrosion-rs/corrosion once
  # http://github.com/corrosion-rs/corrosion/pull/666 is merged.
  GIT_REPOSITORY https://github.com/thunderseethe/corrosion
  GIT_TAG master
)

FetchContent_MakeAvailable(Corrosion)

corrosion_experimental_crubit(regex-cpp-bindings)
corrosion_import_crate(MANIFEST_PATH Cargo.toml)


# NOTE(b/555854776) - remove this workaround.
# Crubit generates crubit/regex_cpp_bindings.h in namespace regex_cpp_bindings.
# Provide crubit/rust.h alias for compatibility with the C++ wrapper code.
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/include/crubit/rust.h"
"#pragma once
#include \"crubit/regex_cpp_bindings.h\"
namespace rust = regex_cpp_bindings;
")
target_include_directories(regex_cpp_bindings INTERFACE "${CMAKE_CURRENT_BINARY_DIR}/include")

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

FetchContent_Declare(
  re2
  GIT_REPOSITORY https://github.com/google/re2.git
  GIT_TAG main
  FIND_PACKAGE_ARGS NAMES re2
)
set(RE2_BUILD_TESTING OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(re2)

FetchContent_Declare(
  google_benchmark
  GIT_REPOSITORY https://github.com/google/benchmark.git
  GIT_TAG main
  FIND_PACKAGE_ARGS NAMES benchmark
)
set(BENCHMARK_ENABLE_TESTING OFF CACHE BOOL "" FORCE)
set(BENCHMARK_ENABLE_INSTALL OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(google_benchmark)

add_library(regex regex.cc regex.h regex_internal.h)
target_link_libraries(regex PRIVATE
  regex_cpp_bindings
  absl::check
  absl::status
  absl::statusor
  absl::strings
  absl::str_format
  absl::span
)

add_executable(regex_example main.cc)
target_link_libraries(regex_example PRIVATE
  regex_cpp_bindings
  regex
  absl::statusor
  absl::strings
)

add_executable(regex_benchmark regex_benchmark.cc)
target_link_libraries(regex_benchmark PRIVATE
  regex_cpp_bindings
  regex
  re2::re2
  benchmark::benchmark
  absl::check
  absl::random_random
  absl::statusor
  absl::strings
  absl::str_format
)
