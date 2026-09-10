# Safe Bindings

C++ wrappers (using Crubit) for Rust libraries.

## Bindings

<!-- keep-sorted start skip_lines=2 -->
| Wrapper      | Wrapped crate                                     | Status   |
| ------------ | ------------------------------------------------- | -------- |
| deflate      | [flate2](https://crates.io/crates/flate2)         | Compiles |
| kamadak_exif | [kamadak-exif](https://crates.io/crates/kamadak-exif) | Compiles      |
| leveldb      | [rusty_leveldb](https://crates.io/crates/rusty_leveldb)       | Doesn't compile - NOTE: Retry after `cc_std::virtual_unique_ptr` is supported |
| pixel_bridge | [image](https://crates.io/crates/image)           | Compiles |
| regex        | [regex](https://crates.io/crates/regex)           | Compiles |
| roaring      | [roaring](https://crates.io/crates/roaring)       | Compiles |
| saphyr       | [saphyr](https://crates.io/crates/saphyr)         | Compiles |
| serde_json   | [serde_json](https://crates.io/crates/serde_json) | Compiles |
| zip          | [zip](https://crates.io/crates/zip)               | Compiles |
<!-- keep-sorted end -->

## Contributing

We want to be open and set the right expectations: We're currently not set up to
accept contributions. We plan to open source more bindings and gather interest
and may accept contributions in the future.
