# Roaring Bridge

Wrapper for the [roaring](https://crates.io/crates/roaring) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-19
rustup component add rust-src rustc-dev llvm-tools-preview

cd roaring_bridge  # this directory
cmake -B build
cmake --build build --parallel

cd build
ctest
```