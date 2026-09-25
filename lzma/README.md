# LZMA

Wrapper for the [lzma-rust2](https://crates.io/crates/lzma-rust2) crate.

## Usage

Requires installation of clang (for `bindgen`) and a recent stable version of
Rust (Edition 2024).

```
cd lzma
cmake -B build
cmake --build build --parallel
```
