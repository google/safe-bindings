# Zip

Wrapper for the [zip](https://crates.io/crates/zip) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-19
rustup component add rust-src rustc-dev llvm-tools-preview

cd zip
cmake -B build
cmake --build build --parallel

build/zip_example
```
