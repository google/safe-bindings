cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(elf
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

corrosion_experimental_crubit(elf_cpp_bindings)
corrosion_import_crate(MANIFEST_PATH Cargo.toml)

# Crubit names the static library after the package (libelf-cpp-bindings.a),
# while Corrosion expects Cargo library target naming (libelf_cpp_bindings.a).
# Copy the byproduct before Corrosion attempts to copy it.
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
"file(GLOB_RECURSE libs
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*elf-cpp-bindings.a\"
  \"${CMAKE_CURRENT_BINARY_DIR}/cargo/*elf-cpp-bindings.lib\"
)
foreach(lib IN LISTS libs)
  string(REPLACE \"elf-cpp-bindings\" \"elf_cpp_bindings\" target_lib \"\${lib}\")
  file(COPY_FILE \"\${lib}\" \"\${target_lib}\")
endforeach()
")
add_custom_command(
  TARGET _cargo-build_elf_cpp_bindings
  POST_BUILD
  COMMAND ${CMAKE_COMMAND} -P "${CMAKE_CURRENT_BINARY_DIR}/copy_crubit_lib.cmake"
)

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

add_library(crubit_helpers INTERFACE)
target_include_directories(crubit_helpers INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(crubit_helpers INTERFACE absl::strings absl::span)

add_library(elf_bridge elf_bridge.cc elf_bridge.h dump.cc dump.h)
target_include_directories(elf_bridge PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(elf_bridge PUBLIC
  elf_cpp_bindings
  crubit_helpers
  absl::status
  absl::statusor
  absl::strings
  absl::str_format
)

add_executable(elf_example main.cc)
target_link_libraries(elf_example PRIVATE
  elf_bridge
  absl::statusor
  absl::strings
  absl::str_format
)
