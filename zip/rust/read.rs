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

    /// Returns the comment of the zip archive.
    pub fn get_comment(&self) -> VecU8 {
        match self.reader.as_ref() {
            Some(reader) => VecU8::copy_from_slice(reader.comment()),
            None => VecU8::default(),
        }
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
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
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
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
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

    /// Returns the comment of the zip archive.
    pub fn get_comment(&self) -> VecU8 {
        match self.reader.as_ref() {
            Some(reader) => VecU8::copy_from_slice(reader.comment()),
            None => VecU8::default(),
        }
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
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
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
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
        }
    }
}

fn get_length_impl<R: Read + Seek>(reader: &Option<WrappedZipArchive<R>>) -> usize {
    match reader.as_ref() {
        Some(r) => r.len(),
        None => 0,
    }
}

#[derive(Default)]
/// A streaming zip reader that reads from an in-memory buffer without seeking.
pub struct BufferedZipStreamReader {
    reader: Option<Cursor<Vec<u8>>>,
    finished: bool,
}

impl Debug for BufferedZipStreamReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferedZipStreamReader")
            .field("reader", &if self.reader.is_some() { "Some(Cursor<Vec<u8>>)" } else { "None" })
            .field("finished", &self.finished)
            .finish()
    }
}

impl BufferedZipStreamReader {
    /// Initializes an empty `BufferedZipStreamReader`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipStreamReader` from data.
    pub fn new_from_data(data: VecU8) -> Result<Self, ZipError> {
        Ok(Self { reader: Some(Cursor::new(data.into_vec())), finished: false })
    }

    /// Returns whether the stream reader is none (due to being default-constructed
    /// or moved-from).
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Reads the next zip file from the stream.
    ///
    /// When there are no more files in the archive (start of central directory
    /// is reached), returns a `BufferedZipFile` where `is_none()` returns true.
    pub fn read_next_file(&mut self) -> Result<BufferedZipFile<'_>, ZipError> {
        if self.finished {
            return Ok(BufferedZipFile::default());
        }
        match self.reader.as_mut() {
            Some(reader) => match zip::read::read_zipfile_from_stream(reader) {
                Ok(Some(file)) => Ok(BufferedZipFile::new(file)),
                Ok(None) => {
                    self.finished = true;
                    Ok(BufferedZipFile::default())
                }
                Err(e) => Err(ZipError::from(e)),
            },
            None => Err(ZipError::failed_precondition("Zip stream reader is not open")),
        }
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `BufferedZipFile`
    /// where `is_none()` returns true.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<BufferedZipFile<'_>, ZipError> {
        if self.finished {
            return Ok(BufferedZipFile::default());
        }
        match self.reader.as_mut() {
            Some(reader) => {
                match zip::read::read_zipfile_from_stream_with_compressed_size(
                    reader,
                    compressed_size,
                ) {
                    Ok(Some(file)) => Ok(BufferedZipFile::new(file)),
                    Ok(None) => {
                        self.finished = true;
                        Ok(BufferedZipFile::default())
                    }
                    Err(e) => Err(ZipError::from(e)),
                }
            }
            None => Err(ZipError::failed_precondition("Zip stream reader is not open")),
        }
    }
}

#[derive(Default)]
/// A streaming zip reader that reads sequentially from a file without seeking.
pub struct FsZipStreamReader {
    reader: Option<File>,
    finished: bool,
}

impl Debug for FsZipStreamReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsZipStreamReader")
            .field("reader", &if self.reader.is_some() { "Some(File)" } else { "None" })
            .field("finished", &self.finished)
            .finish()
    }
}

impl FsZipStreamReader {
    /// Initializes an empty `FsZipStreamReader`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `FsZipStreamReader` from a path.
    pub fn new_from_path(path: &[u8]) -> Result<Self, ZipError> {
        let mut reader = Self::default();
        if let Err(e) = reader.open(path) {
            return Err(ZipError::invalid_argument(format!("Failed to open zip stream: {}", e)));
        }
        Ok(reader)
    }

    fn open(&mut self, path: &[u8]) -> Result<(), String> {
        if self.reader.is_some() {
            return Err("Zip stream reader is already open".into());
        }
        let path_str = match std::str::from_utf8(path) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string()),
        };
        match File::open(path_str) {
            Ok(file) => {
                self.reader = Some(file);
                self.finished = false;
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Returns whether the stream reader is none (due to being default-constructed
    /// or moved-from).
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Reads the next zip file from the stream.
    ///
    /// When there are no more files in the archive (start of central directory
    /// is reached), returns a `FsZipFile` where `is_none()` returns true.
    pub fn read_next_file(&mut self) -> Result<FsZipFile<'_>, ZipError> {
        if self.finished {
            return Ok(FsZipFile::default());
        }
        match self.reader.as_mut() {
            Some(reader) => match zip::read::read_zipfile_from_stream(reader) {
                Ok(Some(file)) => Ok(FsZipFile::new(file)),
                Ok(None) => {
                    self.finished = true;
                    Ok(FsZipFile::default())
                }
                Err(e) => Err(ZipError::from(e)),
            },
            None => Err(ZipError::failed_precondition("Zip stream reader is not open")),
        }
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `FsZipFile`
    /// where `is_none()` returns true.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<FsZipFile<'_>, ZipError> {
        if self.finished {
            return Ok(FsZipFile::default());
        }
        match self.reader.as_mut() {
            Some(reader) => {
                match zip::read::read_zipfile_from_stream_with_compressed_size(
                    reader,
                    compressed_size,
                ) {
                    Ok(Some(file)) => Ok(FsZipFile::new(file)),
                    Ok(None) => {
                        self.finished = true;
                        Ok(FsZipFile::default())
                    }
                    Err(e) => Err(ZipError::from(e)),
                }
            }
            None => Err(ZipError::failed_precondition("Zip stream reader is not open")),
        }
    }
}

/// Reads the next zip file from a buffered stream reader.
pub fn read_buffered_zipfile_from_stream(
    reader: &mut BufferedZipStreamReader,
) -> Result<BufferedZipFile<'_>, ZipError> {
    reader.read_next_file()
}

/// Reads the next zip file from a buffered stream reader with an assumed compressed size.
pub fn read_buffered_zipfile_from_stream_with_compressed_size(
    reader: &mut BufferedZipStreamReader,
    compressed_size: u64,
) -> Result<BufferedZipFile<'_>, ZipError> {
    reader.read_next_file_with_compressed_size(compressed_size)
}

/// Reads the next zip file from a filesystem stream reader.
pub fn read_fs_zipfile_from_stream(
    reader: &mut FsZipStreamReader,
) -> Result<FsZipFile<'_>, ZipError> {
    reader.read_next_file()
}

/// Reads the next zip file from a filesystem stream reader with an assumed compressed size.
pub fn read_fs_zipfile_from_stream_with_compressed_size(
    reader: &mut FsZipStreamReader,
    compressed_size: u64,
) -> Result<FsZipFile<'_>, ZipError> {
    reader.read_next_file_with_compressed_size(compressed_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BufferedZipWriter, CompressionMethod, ZipWriterFileOptions};

    fn create_test_zip() -> VecU8 {
        let mut writer = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        let options = ZipWriterFileOptions::new().compression_method(CompressionMethod::Stored);
        writer.start_file(b"hello.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Hello, world!".to_vec())).unwrap();
        writer.start_file(b"second.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Second file content".to_vec())).unwrap();
        writer.finish().unwrap()
    }

    #[test]
    fn test_buffered_stream_reader_read_all_files() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        assert!(!reader.is_none());
        assert!(!reader.is_finished());

        {
            let mut file1 = reader.read_next_file().unwrap();
            assert!(!file1.is_none());
            assert_eq!(file1.get_file_name().as_slice(), b"hello.txt");
            assert!(file1.is_file());
            assert!(!file1.is_dir());
            assert_eq!(file1.get_file_data().unwrap().as_slice(), b"Hello, world!");
        }

        {
            let mut file2 = reader.read_next_file().unwrap();
            assert!(!file2.is_none());
            assert_eq!(file2.get_file_name().as_slice(), b"second.txt");
            assert_eq!(file2.get_file_data().unwrap().as_slice(), b"Second file content");
        }

        // Third read should return None (end of files)
        {
            let file3 = reader.read_next_file().unwrap();
            assert!(file3.is_none());
        }
        assert!(reader.is_finished());

        // Repeated reads after finish should be idempotent
        {
            let file4 = reader.read_next_file().unwrap();
            assert!(file4.is_none());
        }
    }

    #[test]
    fn test_buffered_stream_reader_chunked_read() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        let mut file = reader.read_next_file().unwrap();
        assert!(!file.is_none());

        // Read in chunks of 5 bytes
        let chunk1 = file.read_bytes(5).unwrap();
        assert_eq!(chunk1.as_slice(), b"Hello");
        let chunk2 = file.read_bytes(5).unwrap();
        assert_eq!(chunk2.as_slice(), b", wor");
        let chunk3 = file.read_bytes(5).unwrap();
        assert_eq!(chunk3.as_slice(), b"ld!");
        let chunk4 = file.read_bytes(5).unwrap();
        assert!(chunk4.is_empty());
    }

    #[test]
    fn test_buffered_stream_reader_free_functions() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        let mut file = read_buffered_zipfile_from_stream(&mut reader).unwrap();
        assert!(!file.is_none());
        assert_eq!(file.get_file_name().as_slice(), b"hello.txt");
        assert_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[test]
    fn test_buffered_stream_reader_with_compressed_size() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        // File 1 is uncompressed "Hello, world!" which is 13 bytes stored
        let mut file = reader.read_next_file_with_compressed_size(13).unwrap();
        assert!(!file.is_none());
        assert_eq!(file.get_file_name().as_slice(), b"hello.txt");
        assert_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[test]
    fn test_buffered_stream_reader_invalid_data() {
        let mut reader =
            BufferedZipStreamReader::new_from_data(VecU8::from(b"not a zip".to_vec())).unwrap();
        let res = reader.read_next_file();
        assert!(res.is_err());
    }
}
