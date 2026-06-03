# Serde JSON

Wrapper for the [serde_json](https://crates.io/crates/serde_json) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-19
rustup component add rust-src rustc-dev llvm-tools-preview

cd serde_json
cmake -B build
cmake --build build --parallel

build/serde_json_example
```
