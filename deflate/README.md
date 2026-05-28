# Deflate

Wrapper for the [flate2](https://crates.io/crates/flate2) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-19
rustup component add rust-src rustc-dev llvm-tools-preview

cd deflate
cmake -B build
cmake --build build --parallel

build/deflate_example
```
