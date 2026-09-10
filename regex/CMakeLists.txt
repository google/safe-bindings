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

corrosion_experimental_crubit(regex_cpp_bindings)
corrosion_import_crate(MANIFEST_PATH Cargo.toml)

# Crubit names the static library after the package (libregex-cpp-bindings.a),
# while Corrosion expects Cargo library target naming (libregex_cpp_bindings.a).
# Copy the byproduct before Corrosion attempts to copy it.
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
"file(GLOB_RECURSE libs
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*regex-cpp-bindings.a\"
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*regex-cpp-bindings.lib\"
)
foreach(lib IN LISTS libs)
  string(REPLACE \"regex-cpp-bindings\" \"regex_cpp_bindings\" target_lib \"\${lib}\")
  file(COPY_FILE \"\${lib}\" \"\${target_lib}\")
endforeach()
")
add_custom_command(
  TARGET _cargo-build_regex_cpp_bindings
  POST_BUILD
  COMMAND ${CMAKE_COMMAND} -P "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
)

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
