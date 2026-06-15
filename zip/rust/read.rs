// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use crate::{BufferedZipFile, FsZipFile, VecU8, ZipError};
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive as WrappedZipArchive;

#[derive(Default)]
pub struct BufferedZipArchive {
    reader: Option<WrappedZipArchive<Cursor<Vec<u8>>>>,
}

impl Debug for BufferedZipArchive {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("BufferedZipArchive")
            .field(
                "reader",
                &if self.reader.is_some() { "Some(WrappedZipArchive)" } else { "None" },
            )
            .finish()
    }
}

impl BufferedZipArchive {
    /// Initializes an empty `BufferedZipArchive`.
    ///
    /// An archive created this way will be in an empty state. Use `new_from_data`
    /// to read an archive from a data buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipArchive` from data.
    ///
    /// Returns an error if `data` is not a valid zip archive.
    pub fn new_from_data(data: VecU8) -> Result<BufferedZipArchive, ZipError> {
        let mut archive = Self::default();
        if let Err(e) = archive.open(data) {
            return Err(ZipError::invalid_argument(format!("Failed to open zip buffer: {}", e)));
        }
        Ok(archive)
    }

    /// Opens a zip archive from data.
    ///
    /// Returns an error if `data` is not a valid zip archive.
    fn open(&mut self, data: VecU8) -> Result<(), String> {
        let cursor = Cursor::new(data.into_vec());
        match WrappedZipArchive::new(cursor) {
            Ok(reader) => {
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Returns whether the zip archive is none (due to being
    /// default-constructed or moved-from).
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns the number of files in the archive.
    pub fn get_length(&self) -> usize {
        get_length_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<BufferedZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index(index) {
                Ok(file) => Ok(BufferedZipFile::new(file)),
                Err(e) => Err(ZipError::out_of_range(e.to_string())),
            },
            None => Ok(BufferedZipFile::default()),
        }
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<BufferedZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index_raw(index) {
                Ok(file) => Ok(BufferedZipFile::new(file)),
                Err(e) => Err(ZipError::out_of_range(e.to_string())),
            },
            None => Ok(BufferedZipFile::default()),
        }
    }
}

#[derive(Default)]
pub struct FsZipArchive {
    reader: Option<WrappedZipArchive<File>>,
}

impl Debug for FsZipArchive {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("FsZipArchive")
            .field("reader", &if self.reader.is_some() { "Some(FsZipArchive)" } else { "None" })
            .finish()
    }
}

impl FsZipArchive {
    /// Initializes an empty `FsZipArchive`.
    ///
    /// An archive file must be opened by calling `open` before it can be used.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `FsZipArchive` from a path.
    ///
    /// Returns an error if `path` cannot be opened or is not a valid zip
    /// archive.
    pub fn new_from_path(path: &[u8]) -> Result<FsZipArchive, ZipError> {
        let mut archive = Self::default();
        if let Err(e) = archive.open(path) {
            return Err(ZipError::invalid_argument(format!("Failed to open zip archive: {}", e)));
        }
        Ok(archive)
    }

    /// Opens a zip archive from a path.
    ///
    /// Returns an error if the archive is already open, if `path` cannot be
    /// opened, or if `path` does not point to a valid zip archive.
    fn open(&mut self, path: &[u8]) -> Result<(), String> {
        if self.reader.is_some() {
            return Err("Zip archive is already open".into());
        }
        let path_str = match std::str::from_utf8(path) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string()),
        };
        match File::open(path_str) {
            Ok(file) => match WrappedZipArchive::new(file) {
                Ok(reader) => {
                    self.reader = Some(reader);
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            },
            Err(e) => Err(e.to_string()),
        }
    }

    /// Returns whether the zip archive is none (due to being
    /// default-constructed or moved-from).
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns the number of files in the archive.
    pub fn get_length(&self) -> usize {
        get_length_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<FsZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index(index) {
                Ok(file) => Ok(FsZipFile::new(file)),
                Err(e) => Err(ZipError::out_of_range(e.to_string())),
            },
            None => Ok(FsZipFile::default()),
        }
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<FsZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index_raw(index) {
                Ok(file) => Ok(FsZipFile::new(file)),
                Err(e) => Err(ZipError::out_of_range(e.to_string())),
            },
            None => Ok(FsZipFile::default()),
        }
    }
}

fn get_length_impl<R: Read + Seek>(reader: &Option<WrappedZipArchive<R>>) -> usize {
    match reader.as_ref() {
        Some(r) => r.len(),
        None => 0,
    }
}
