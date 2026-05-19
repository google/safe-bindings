//! This crate provides C++ bindings for the zip crate.
//! This crate defines types and methods that expose the zip API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use zip directly.

use crubit_util::make_result_type;
use rust_vec_u8::VecU8;

mod file;
pub use file::{BufferedZipFile, FsZipFile};

mod read;
pub use read::{BufferedZipArchive, FsZipArchive};

mod write;
pub use write::{BufferedZipWriter, CompressionMethod, FsZipWriter, ZipWriterFileOptions};

make_result_type!(VecU8, ResultVecU8);
make_result_type!((), ResultUnit);

make_result_type!(read::BufferedZipArchive, ResultBufferedZipArchive);
make_result_type!(read::FsZipArchive, ResultFsZipArchive);
make_result_type!(file::BufferedZipFile<'a>, ResultBufferedZipFile<'a>);
make_result_type!(file::FsZipFile<'a>, ResultFsZipFile<'a>);
make_result_type!(write::BufferedZipWriter, ResultBufferedZipWriter);
make_result_type!(write::FsZipWriter, ResultFsZipWriter);
