cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(xml
  VERSION 0.1.0
  LANGUAGES CXX)

set(CMAKE_SKIP_INSTALL_RULES ON)
include(FetchContent)

FetchContent_Declare(
  Corrosion
  GIT_REPOSITORY https://github.com/thunderseethe/corrosion
  GIT_TAG master
)

FetchContent_MakeAvailable(Corrosion)

corrosion_experimental_crubit(rust)
corrosion_import_crate(MANIFEST_PATH rust/Cargo.toml)

corrosion_experimental_crubit(xmltree_rust)
corrosion_import_crate(MANIFEST_PATH xmltree_rust/Cargo.toml)

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  GIT_TAG 20250127.1
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

add_library(xml_lib 
    quick_reader.cc 
    quick_writer.cc 
    xmltree.cc
)
target_include_directories(xml_lib PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_compile_definitions(xml_lib PUBLIC ABSL_DEFINE_UNQUALIFIED_STATUS_MACROS)
target_link_libraries(xml_lib
  PUBLIC
    absl::die_if_null
    absl::status
    absl::statusor
    absl::strings
    absl::span
    rust
    xmltree_rust
  PRIVATE
    absl::log
    absl::core_headers
    absl::status_macros
)
# The rust and xmltree_rust crates each get their own Crubit bindings, and both
# contain identically named thunks for the same Rust standard library generics.
# The copies are identical, so let the linker keep one of them.
target_link_options(xml_lib INTERFACE "LINKER:--allow-multiple-definition")

add_executable(xml_example
    main.cc
    quick_reader_example.cc
    quick_writer_example.cc
    xmltree_example.cc
)
target_link_libraries(xml_example PRIVATE xml_lib absl::status_macros)

enable_testing()
foreach(example quick_reader quick_writer xmltree)
  add_test(NAME ${example}_example COMMAND xml_example ${example})
  set_tests_properties(${example}_example PROPERTIES LABELS example)
endforeach()
