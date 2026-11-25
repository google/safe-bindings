cmake_minimum_required(VERSION 3.22)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

project(lzma
  VERSION 0.1.0
  LANGUAGES CXX)

include(FetchContent)

FetchContent_Declare(
  Corrosion
  GIT_REPOSITORY https://github.com/corrosion-rs/corrosion
  # Post-v0.6.1 commit with Crubit support (PR #666).
  GIT_TAG c4a90eaef5700fcfd2630e8d886563bfc2385d8f
)

FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(MANIFEST_PATH rust/Cargo.toml)

add_library(lzma INTERFACE)
target_include_directories(lzma INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(lzma INTERFACE rust)

add_executable(lzma_example main.cc)
target_link_libraries(lzma_example PRIVATE lzma)
