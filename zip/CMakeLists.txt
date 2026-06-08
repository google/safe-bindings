cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_COMPILER clang)
set(CMAKE_CXX_COMPILER clang++)

project(zip
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

add_library(zip_converters converters.cc converters.h)
target_link_libraries(zip_converters PRIVATE rust absl::status absl::statusor absl::strings absl::span)

# Include the root directory for crubit_helpers/string_conversions.h
target_include_directories(zip_converters PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/..)

add_library(zip_file file.cc file.h)
target_link_libraries(zip_file PRIVATE zip_converters rust absl::status absl::status_macros absl::statusor)

add_library(zip_read read.cc read.h)
target_link_libraries(zip_read PRIVATE zip_converters zip_file rust absl::status_macros absl::statusor absl::strings absl::span)

add_library(zip_write write.cc write.h)
target_link_libraries(zip_write PRIVATE zip_converters zip_file rust absl::status absl::status_macros absl::statusor absl::strings absl::span)

add_executable(zip_example main.cc)
target_link_libraries(zip_example PRIVATE rust zip_converters zip_file zip_read zip_write absl::statusor absl::strings)
