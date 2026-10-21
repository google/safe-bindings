# ELF

C++ wrapper for the [elf](https://crates.io/crates/elf) crate, for parsing
ELF (Executable and Linkable Format) files.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly
rustup component add rust-src rustc-dev llvm-tools-preview

cd elf
cmake -B build
cmake --build build --parallel
```
