# FFT

Wrapper for the [ndrustfft](https://crates.io/crates/ndrustfft) crate.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-08-20
rustup component add rust-src rustc-dev llvm-tools-preview

cd fft
cmake -B build
cmake --build build --parallel
```
