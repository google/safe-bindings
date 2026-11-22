cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(kamadak-exif
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

FetchContent_Declare(
  googletest
  GIT_REPOSITORY https://github.com/google/googletest.git
  GIT_TAG v1.18.0
  FIND_PACKAGE_ARGS NAMES GTest
)
FetchContent_MakeAvailable(googletest)

set(ABSL_BUILD_TEST_HELPERS ON CACHE BOOL "" FORCE)
set(ABSL_USE_EXTERNAL_GOOGLETEST ON CACHE BOOL "" FORCE)
set(ABSL_FIND_GOOGLETEST OFF CACHE BOOL "" FORCE)
FetchContent_Declare(
  absl
  GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git
  FIND_PACKAGE_ARGS NAMES absl
)
FetchContent_MakeAvailable(absl)

corrosion_experimental_crubit(rust)
corrosion_import_crate(MANIFEST_PATH rust/Cargo.toml)

add_library(crubit_helpers INTERFACE)
target_include_directories(crubit_helpers INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(crubit_helpers INTERFACE absl::core_headers absl::strings absl::span)

add_library(exif_bridge exif_bridge.cc exif_bridge.h exif_reader.cc exif_reader.h)
target_include_directories(exif_bridge PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_include_directories(exif_bridge PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/..")
target_link_libraries(exif_bridge
  PUBLIC
    rust
    absl::base
    absl::chunked_queue
    absl::core_headers
    absl::status
    absl::statusor
    absl::strings
    absl::span
  PRIVATE
    crubit_helpers
)

add_executable(kamadak_exif_example main.cc)
target_link_libraries(kamadak_exif_example PRIVATE rust exif_bridge absl::statusor absl::strings)

enable_testing()
include(GoogleTest)

add_test(NAME kamadak_exif_example COMMAND kamadak_exif_example)
set_tests_properties(kamadak_exif_example PROPERTIES LABELS example)

add_executable(exif_bridge_test exif_bridge_test.cc)
target_link_libraries(exif_bridge_test PRIVATE
  exif_bridge
  rust
  crubit_helpers
  absl::status
  absl::status_matchers
  absl::statusor
  absl::strings
  absl::span
  GTest::gmock
  GTest::gtest_main
)
gtest_discover_tests(exif_bridge_test)
