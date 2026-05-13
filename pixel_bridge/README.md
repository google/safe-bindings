# Pixel Bridge

Wrapper for the [image](https://crates.io/crates/image) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-05-01
rustup component add rust-src rustc-dev llvm-tools-preview

cd pixel_bridge
cmake -B build
cmake --build build --parallel

build/pixel_bridge_example path/to/some/image.jpg
```
