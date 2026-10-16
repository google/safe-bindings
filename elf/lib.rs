//! Crubit-compatible wrapper for the `elf` crate.
//!
//! Currently provides the `ElfBytes` struct and implements parsing elf class and machine metadata.
pub mod compression;
pub mod constants;
mod crubit_utils;
pub mod dynamic;
pub mod elf_bytes;
pub mod elf_stream;
pub mod file;
pub mod gnu_symver;
pub mod hash;
pub mod note;
pub mod relocation;
pub mod section;
pub mod segment;
pub mod string_table;
pub mod symbol;
pub mod to_str;
pub mod types;

