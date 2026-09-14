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

corrosion_experimental_crubit(deflate_cpp_bindings)
corrosion_import_crate(MANIFEST_PATH Cargo.toml)

# Crubit names the static library after the package (libdeflate-cpp-bindings.a),
# while Corrosion expects Cargo library target naming (libdeflate_cpp_bindings.a).
# Copy the byproduct before Corrosion attempts to copy it.
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
"file(GLOB_RECURSE libs
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*deflate-cpp-bindings.a\"
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*deflate-cpp-bindings.lib\"
)
foreach(lib IN LISTS libs)
  string(REPLACE \"deflate-cpp-bindings\" \"deflate_cpp_bindings\" target_lib \"\${lib}\")
  file(COPY_FILE \"\${lib}\" \"\${target_lib}\")
endforeach()
")
add_custom_command(
  TARGET _cargo-build_deflate_cpp_bindings
  POST_BUILD
  COMMAND ${CMAKE_COMMAND} -P "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
)

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

add_library(flate2 flate2.cc flate2.h)
target_include_directories(flate2 PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(flate2 PRIVATE deflate_cpp_bindings absl::status absl::statusor absl::cord absl::strings absl::span)

add_library(gzip_wrapper gzip_wrapper.cc gzip_wrapper.h)
target_link_libraries(gzip_wrapper PRIVATE flate2 deflate_cpp_bindings absl::status absl::statusor absl::strings absl::span)

add_executable(deflate_example main.cc)
target_link_libraries(deflate_example PRIVATE deflate_cpp_bindings flate2 gzip_wrapper absl::statusor absl::strings)
