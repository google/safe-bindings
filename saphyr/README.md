# Saphyr

Wrapper for the [saphyr](https://crates.io/crates/saphyr) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-08-20
rustup component add rust-src rustc-dev llvm-tools-preview

cd saphyr
cmake -B build
cmake --build build --parallel

build/saphyr_example
```
