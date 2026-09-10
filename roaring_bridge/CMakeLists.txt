cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(roaring_bridge
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

corrosion_experimental_crubit(roaring_bridge)
corrosion_import_crate(MANIFEST_PATH rust/Cargo.toml)

FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

add_library(roaring_bridge_lib INTERFACE)
target_include_directories(roaring_bridge_lib INTERFACE
  "${CMAKE_CURRENT_BINARY_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}/.."
)
target_link_libraries(roaring_bridge_lib INTERFACE
  roaring_bridge
)
target_link_libraries(roaring_bridge_lib INTERFACE crubit_support)
add_dependencies(roaring_bridge_lib crubit_support)

# GoogleTest
FetchContent_Declare(
  googletest
  GIT_REPOSITORY https://github.com/google/googletest.git
  GIT_TAG v1.18.0
  FIND_PACKAGE_ARGS NAMES GTest
)
FetchContent_MakeAvailable(googletest)

enable_testing()
include(GoogleTest)
target_include_directories(roaring_bridge_lib INTERFACE
  "${CMAKE_CURRENT_BINARY_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}"
  "${CMAKE_CURRENT_SOURCE_DIR}/.."
)

add_executable(roaring_bridge_cc_test roaring_bridge_cc_test.cc)
target_include_directories(roaring_bridge_cc_test PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}"
)
target_link_libraries(roaring_bridge_cc_test PRIVATE
  roaring_bridge_lib
  GTest::gtest_main
  GTest::gmock
)
gtest_discover_tests(roaring_bridge_cc_test)
