# Kamadak Exif

Wrapper for the [kamadak-exif](https://crates.io/crates/kamadak-exif) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-19
rustup component add rust-src rustc-dev llvm-tools-preview

cd kamadak_exif
cmake -B build
cmake --build build --parallel

build/kamadak_exif_example
```
