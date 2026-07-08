# LevelDB

Wrapper for the [leveldb](https://crates.io/crates/leveldb) crate.

## Usage

**Note:** The Safe Bindings for `rusty-leveldb` are currently experimental and
are not yet buildable with CMake.

This is due to the following current limitations:

1. The bindings depend on a customized version of the `rusty-leveldb` crate containing patches not yet available upstream.
2. They tightly couple with `cc_std` (Crubit's standard library bindings), which is not fully supported by the current CMake build structure.
