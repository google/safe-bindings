// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use cc_std::std::string_view;
use rust_vec_u8::VecU8;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive as WrappedZipArchive;

use crate::{BufferedZipFile, FsZipFile};
use crate::{ResultBufferedZipArchive, ResultBufferedZipFile, ResultFsZipArchive, ResultFsZipFile};

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
    pub fn new_from_data(data: VecU8) -> ResultBufferedZipArchive {
        let mut archive = Self::default();
        if let Err(e) = archive.open(data) {
            return ResultBufferedZipArchive::from(anyhow::anyhow!(
                "Failed to open zip buffer: {}",
                e
            ));
        }
        ResultBufferedZipArchive::from_ok(archive)
    }

    /// Opens a zip archive from data.
    ///
    /// Returns an error if `data` is not a valid zip archive.
    fn open(&mut self, data: VecU8) -> anyhow::Result<()> {
        let cursor = Cursor::new(data.into_vec());
        match WrappedZipArchive::new(cursor) {
            Ok(reader) => {
                self.reader = Some(reader);
                Ok(())
            }
            Err(e) => Err(e.into()),
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
    pub fn get_file_by_index(&mut self, index: usize) -> ResultBufferedZipFile<'_> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index(index) {
                Ok(file) => ResultBufferedZipFile::from_ok(BufferedZipFile::new(file)),
                Err(e) => ResultBufferedZipFile::from(anyhow::Error::from(e)),
            },
            None => ResultBufferedZipFile::from_ok(BufferedZipFile::default()),
        }
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> ResultBufferedZipFile<'_> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index_raw(index) {
                Ok(file) => ResultBufferedZipFile::from_ok(BufferedZipFile::new(file)),
                Err(e) => ResultBufferedZipFile::from(anyhow::Error::from(e)),
            },
            None => ResultBufferedZipFile::from_ok(BufferedZipFile::default()),
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
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    pub unsafe fn new_from_path(path: string_view) -> ResultFsZipArchive {
        let mut archive = Self::default();
        if let Err(e) = archive.open(path) {
            return ResultFsZipArchive::from(anyhow::anyhow!("Failed to open zip archive: {}", e));
        }
        ResultFsZipArchive::from_ok(archive)
    }

    /// Opens a zip archive from a path.
    ///
    /// Returns an error if the archive is already open, if `path` cannot be
    /// opened, or if `path` does not point to a valid zip archive.
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    unsafe fn open(&mut self, path: string_view) -> anyhow::Result<()> {
        if self.reader.is_some() {
            return Err(anyhow::anyhow!("Zip archive is already open"));
        }
        // SAFETY: The caller of `open` must ensure that `path` is valid for
        // the duration of the call. All borrows in `open` do not outlive `path`.
        match File::open(String::from_utf8_lossy(unsafe { path.as_bytes() }).as_ref()) {
            Ok(file) => match WrappedZipArchive::new(file) {
                Ok(reader) => {
                    self.reader = Some(reader);
                    Ok(())
                }
                Err(e) => Err(e.into()),
            },
            Err(e) => Err(e.into()),
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
    pub fn get_file_by_index(&mut self, index: usize) -> ResultFsZipFile<'_> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index(index) {
                Ok(file) => ResultFsZipFile::from_ok(FsZipFile::new(file)),
                Err(e) => ResultFsZipFile::from(anyhow::Error::from(e)),
            },
            None => ResultFsZipFile::from_ok(FsZipFile::default()),
        }
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an empty zip file if archive is not open.
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> ResultFsZipFile<'_> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index_raw(index) {
                Ok(file) => ResultFsZipFile::from_ok(FsZipFile::new(file)),
                Err(e) => ResultFsZipFile::from(anyhow::Error::from(e)),
            },
            None => ResultFsZipFile::from_ok(FsZipFile::default()),
        }
    }
}

fn get_length_impl<R: Read + Seek>(reader: &Option<WrappedZipArchive<R>>) -> usize {
    match reader.as_ref() {
        Some(r) => r.len(),
        None => 0,
    }
}
