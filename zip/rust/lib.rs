//! This crate provides C++ bindings for the zip crate.
//! This crate defines types and methods that expose the zip API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use zip directly.

mod file;
pub use file::{BufferedZipFile, FsZipFile};

mod read;
pub use read::{
    read_buffered_zipfile_from_stream, read_buffered_zipfile_from_stream_with_compressed_size,
    read_fs_zipfile_from_stream, read_fs_zipfile_from_stream_with_compressed_size,
    BufferedZipArchive, BufferedZipStreamReader, FsZipArchive, FsZipStreamReader,
};

mod write;
pub use write::{BufferedZipWriter, CompressionMethod, FsZipWriter, ZipWriterFileOptions};

mod vec_u8;
pub use vec_u8::VecU8;

mod error;
pub use error::{ZipError, ZipErrorCode};
