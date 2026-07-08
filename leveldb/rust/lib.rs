//! This crate provides C++ bindings for the rusty_leveldb crate.
//! This crate defines types and methods that expose the rusty_leveldb API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use rusty_leveldb directly.
#![feature(string_from_utf8_lossy_owned)]

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

mod cmp_wrapper;
mod db_impl;
mod db_iter;
mod env_wrapper;
mod filter_wrapper;
mod logger_wrapper;
mod options;
mod snapshot;
mod status;
mod vec_u8;
mod write_batch;

pub use crate::status::{LevelDBError, LevelDBErrorCode};

pub fn in_memory() -> Options {
    Options { inner: rusty_leveldb::in_memory() }
}

// Types wrapped by this crate.
pub use db_impl::{DBValue, DB};
pub use db_iter::{DBIterator, DBKeyValueTuple};
pub use options::Options;
pub use snapshot::Snapshot;
pub use write_batch::WriteBatch;

// Utility types
pub use vec_u8::VecU8;

/// ONLY USED FOR TESTING.
/// Returns the number of children for a given path.
pub fn debug_env_children_count(options: &Options, path: &[u8]) -> usize {
    let path = Path::new(OsStr::from_bytes(path));
    options.inner.env.children(path).unwrap_or_default().len()
}

/// ONLY USED FOR TESTING.
/// Returns the child at the specified index.
pub fn debug_env_children_get(options: &Options, path: &[u8], i: usize) -> Option<VecU8> {
    let path = Path::new(OsStr::from_bytes(path));
    let files = options.inner.env.children(path).unwrap_or_default();
    files.get(i).map(|f| f.as_os_str().to_string_lossy().to_string().into())
}

/// ONLY USED FOR TESTING.
/// Opens a file for sequential reading and returns its entire content.
pub fn debug_env_open_and_read(options: &Options, path: &[u8]) -> Option<VecU8> {
    let path = Path::new(OsStr::from_bytes(path));
    let mut file = options.inner.env.open_sequential_file(path).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf.into())
}

/// ONLY USED FOR TESTING.
/// Opens a file for writing and writes the provided data.
pub fn debug_env_open_and_write(options: &Options, path: &[u8], data: &[u8]) {
    let path = Path::new(OsStr::from_bytes(path));
    let mut file = options.inner.env.open_writable_file(path).expect("failed to open file");
    file.write_all(data).expect("failed to write file");
    file.flush().expect("failed to flush file");
}
