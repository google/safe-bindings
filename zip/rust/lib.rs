//! This crate provides C++ bindings for the zip crate.
//! This crate defines types and methods that expose the zip API one to one,
//! but in a way that Crubit understands and is able to generate valid C++ headers.
//!
//! WARNING: This crate should never be used from Rust, instead use zip directly.

#![forbid(unsafe_code)]

mod callback_reader;
pub use callback_reader::stage_callback_chunk;

mod entry_metadata;
pub use entry_metadata::ZipEntryMetadata;

mod file;
pub use file::{BufferedZipFile, CallbackZipFile, FsZipFile};

mod read;
pub use read::{
    BufferedZipArchive, BufferedZipStreamReader, CallbackZipArchive, FsZipArchive,
    FsZipStreamReader,
};

mod write;
pub use write::{
    new_buffered_zip_stream_writer, new_fs_zip_stream_writer, BufferedZipStreamWriter,
    BufferedZipWriter, CompressionMethod, FsZipStreamWriter, FsZipWriter, ZipWriterFileOptions,
};

mod vec_u8;
pub use vec_u8::VecU8;

mod error;
pub use error::{ZipError, ZipErrorCode};
