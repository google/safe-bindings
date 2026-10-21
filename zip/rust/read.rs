// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use crate::{BufferedZipFile, FsZipFile, VecU8, ZipEntryMetadata, ZipError};
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use zip::read::ZipFile as WrappedZipFile;
use zip::ZipArchive as WrappedZipArchive;

#[derive(Default)]
pub struct BufferedZipArchive {
    reader: Option<Box<WrappedZipArchive<Cursor<Vec<u8>>>>>,
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
                self.reader = Some(Box::new(reader));
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
        get_comment_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an error if the archive is not open (`is_none()`).
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
    /// Returns an error if the archive is not open (`is_none()`).
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

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry.
    ///
    /// Returns an error if the archive is not open or `index` is out of
    /// bounds.
    pub fn get_entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        get_entry_metadata_impl(&self.reader, index)
    }
}

#[derive(Default)]
pub struct FsZipArchive {
    reader: Option<Box<WrappedZipArchive<File>>>,
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
                    self.reader = Some(Box::new(reader));
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
        get_comment_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an error if the archive is not open (`is_none()`).
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
    /// Returns an error if the archive is not open (`is_none()`).
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

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry.
    ///
    /// Returns an error if the archive is not open or `index` is out of
    /// bounds.
    pub fn get_entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        get_entry_metadata_impl(&self.reader, index)
    }
}

fn get_length_impl<R: Read + Seek>(reader: &Option<Box<WrappedZipArchive<R>>>) -> usize {
    match reader.as_ref() {
        Some(r) => r.len(),
        None => 0,
    }
}

fn get_comment_impl<R: Read + Seek>(reader: &Option<Box<WrappedZipArchive<R>>>) -> VecU8 {
    reader.as_ref().map(|r| VecU8::copy_from_slice(r.comment())).unwrap_or_default()
}

fn get_entry_metadata_impl<R: Read + Seek>(
    reader: &Option<Box<WrappedZipArchive<R>>>,
    index: usize,
) -> Result<ZipEntryMetadata, ZipError> {
    let Some(r) = reader.as_ref() else {
        return Err(ZipError::failed_precondition("Zip archive is not open"));
    };
    // The central directory was parsed when the archive was opened, so this
    // reads nothing.
    match ZipEntryMetadata::from_central_directory(&r.metadata(), index) {
        Some(metadata) => Ok(metadata),
        None => Err(ZipError::out_of_range(format!(
            "Index {index} is out of bounds for an archive of {} entries",
            r.len()
        ))),
    }
}

/// Shared body of `read_next_file` / `read_next_file_with_compressed_size` for
/// both stream readers.
///
/// Returns `Ok(None)` if the stream is already finished or once the start of
/// the central directory is reached (which also sets `finished`). Returns a
/// `FailedPrecondition` error if `reader` is `None`. When `compressed_size` is
/// `Some`, it is used for entries whose local header carries no size (general
/// purpose bit 3 / data descriptor).
fn read_next_file_impl<'a, R: Read>(
    reader: &'a mut Option<Box<R>>,
    finished: &mut bool,
    compressed_size: Option<u64>,
) -> Result<Option<WrappedZipFile<'a, R>>, ZipError> {
    if *finished {
        return Ok(None);
    }
    let reader = match reader.as_deref_mut() {
        Some(reader) => reader,
        None => return Err(ZipError::failed_precondition("Zip stream reader is not open")),
    };
    let result = match compressed_size {
        Some(size) => zip::read::read_zipfile_from_stream_with_compressed_size(reader, size),
        None => zip::read::read_zipfile_from_stream(reader),
    };
    match result {
        Ok(Some(file)) => Ok(Some(file)),
        Ok(None) => {
            *finished = true;
            Ok(None)
        }
        Err(e) => Err(ZipError::from_zip_error(e)),
    }
}

/// A streaming zip reader that reads from an in-memory buffer without seeking.
#[derive(Default)]
pub struct BufferedZipStreamReader {
    reader: Option<Box<Cursor<Vec<u8>>>>,
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
        Ok(Self { reader: Some(Box::new(Cursor::new(data.into_vec()))), finished: false })
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
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, None)?
            .map(BufferedZipFile::new)
            .unwrap_or_default())
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `BufferedZipFile`
    /// where `is_none()` returns true.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<BufferedZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, Some(compressed_size))?
            .map(BufferedZipFile::new)
            .unwrap_or_default())
    }
}

/// A streaming zip reader that reads sequentially from a file without seeking.
#[derive(Default)]
pub struct FsZipStreamReader {
    reader: Option<Box<File>>,
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
        let path_str = std::str::from_utf8(path)
            .map_err(|e| ZipError::invalid_argument(format!("Failed to open zip stream: {}", e)))?;
        let file = File::open(path_str)
            .map_err(|e| ZipError::invalid_argument(format!("Failed to open zip stream: {}", e)))?;
        Ok(Self { reader: Some(Box::new(file)), finished: false })
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
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, None)?
            .map(FsZipFile::new)
            .unwrap_or_default())
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `FsZipFile`
    /// where `is_none()` returns true.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<FsZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, Some(compressed_size))?
            .map(FsZipFile::new)
            .unwrap_or_default())
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
    use crate::{BufferedZipWriter, CompressionMethod, ZipErrorCode, ZipWriterFileOptions};
    use googletest::prelude::*;

    fn create_test_zip() -> VecU8 {
        let mut writer = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        let options = ZipWriterFileOptions::new().compression_method(CompressionMethod::Stored);
        writer.start_file(b"hello.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Hello, world!".to_vec())).unwrap();
        writer.start_file(b"second.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Second file content".to_vec())).unwrap();
        writer.finish().unwrap()
    }

    #[gtest]
    fn test_entry_offsets_delimit_the_stored_bytes() {
        let zip_data = create_test_zip();
        let raw = zip_data.as_slice().to_vec();
        let mut archive = BufferedZipArchive::new_from_data(zip_data).unwrap();
        let file = archive.get_file_by_index(0).unwrap();

        let header_start = file.get_header_start();
        let data_start = file.get_data_start();
        let compressed_size = file.get_compressed_size();

        // The first entry's local header sits at the very start of the archive,
        // and its data follows the 30-byte fixed header plus the file name.
        expect_eq!(header_start, 0);
        expect_eq!(data_start, 30 + "hello.txt".len() as u64);
        // The entry is Stored, so the raw bytes in that range are the contents.
        let begin = data_start as usize;
        let end = begin + compressed_size as usize;
        expect_eq!(&raw[begin..end], b"Hello, world!");
    }

    #[gtest]
    fn test_stream_entries_report_unknown_data_start() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        let file = reader.read_next_file().unwrap();

        // Stream readers never learn where the entry lives in the archive.
        // `data_start` reports that as 0, which no real entry can have;
        // `header_start` is simply 0 and not meaningful for stream entries (the
        // first entry of an archive genuinely starts at 0 too).
        expect_eq!(file.get_data_start(), 0);
        expect_eq!(file.get_header_start(), 0);
    }

    #[gtest]
    fn test_get_entry_metadata_lists_the_central_directory() {
        let zip_data = create_test_zip();
        let archive = BufferedZipArchive::new_from_data(zip_data).unwrap();

        let first = archive.get_entry_metadata(0).unwrap();
        expect_eq!(first.get_file_name().as_slice(), b"hello.txt");
        expect_eq!(first.get_uncompressed_size(), "Hello, world!".len() as u64);
        expect_eq!(first.get_compressed_size(), "Hello, world!".len() as u64);
        expect_eq!(first.get_compression_method(), CompressionMethod::Stored);
        expect_eq!(first.get_header_start(), 0);
        expect_true!(first.is_file());
        expect_false!(first.is_dir());
        expect_false!(first.is_encrypted());

        let second = archive.get_entry_metadata(1).unwrap();
        expect_eq!(second.get_file_name().as_slice(), b"second.txt");
        expect_eq!(second.get_uncompressed_size(), "Second file content".len() as u64);
        // The second entry's local header follows the first entry: its 30-byte
        // fixed header, its name and its stored contents.
        expect_eq!(
            second.get_header_start(),
            30 + "hello.txt".len() as u64 + "Hello, world!".len() as u64
        );

        let code = archive.get_entry_metadata(2).err().map(|e| e.code);
        expect_eq!(code, Some(ZipErrorCode::OutOfRange));
    }

    #[gtest]
    fn test_get_entry_metadata_on_an_unopened_archive_fails_precondition() {
        let archive = BufferedZipArchive::new();
        expect_true!(archive.is_none());
        let code = archive.get_entry_metadata(0).err().map(|e| e.code);
        expect_eq!(code, Some(ZipErrorCode::FailedPrecondition));
    }

    #[gtest]
    fn test_buffered_stream_reader_read_all_files() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        expect_false!(reader.is_none());
        expect_false!(reader.is_finished());

        {
            let mut file1 = reader.read_next_file().unwrap();
            expect_false!(file1.is_none());
            expect_eq!(file1.get_file_name().as_slice(), b"hello.txt");
            expect_true!(file1.is_file());
            expect_false!(file1.is_dir());
            expect_eq!(file1.get_file_data().unwrap().as_slice(), b"Hello, world!");
        }

        {
            let mut file2 = reader.read_next_file().unwrap();
            expect_false!(file2.is_none());
            expect_eq!(file2.get_file_name().as_slice(), b"second.txt");
            expect_eq!(file2.get_file_data().unwrap().as_slice(), b"Second file content");
        }

        // Third read should return None (end of files)
        {
            let file3 = reader.read_next_file().unwrap();
            expect_true!(file3.is_none());
        }
        expect_true!(reader.is_finished());

        // Repeated reads after finish should be idempotent
        {
            let file4 = reader.read_next_file().unwrap();
            expect_true!(file4.is_none());
        }
    }

    #[gtest]
    fn test_buffered_stream_reader_chunked_read() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        let mut file = reader.read_next_file().unwrap();
        expect_false!(file.is_none());

        // Read in chunks of 5 bytes
        let chunk1 = file.read_bytes(5).unwrap();
        expect_eq!(chunk1.as_slice(), b"Hello");
        let chunk2 = file.read_bytes(5).unwrap();
        expect_eq!(chunk2.as_slice(), b", wor");
        let chunk3 = file.read_bytes(5).unwrap();
        expect_eq!(chunk3.as_slice(), b"ld!");
        let chunk4 = file.read_bytes(5).unwrap();
        expect_true!(chunk4.is_empty());
    }

    #[gtest]
    fn test_buffered_stream_reader_free_functions() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        let mut file = read_buffered_zipfile_from_stream(&mut reader).unwrap();
        expect_false!(file.is_none());
        expect_eq!(file.get_file_name().as_slice(), b"hello.txt");
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_buffered_stream_reader_with_compressed_size() {
        let zip_data = create_test_zip();
        let mut reader = BufferedZipStreamReader::new_from_data(zip_data).unwrap();
        // File 1 is uncompressed "Hello, world!" which is 13 bytes stored
        let mut file = reader.read_next_file_with_compressed_size(13).unwrap();
        expect_false!(file.is_none());
        expect_eq!(file.get_file_name().as_slice(), b"hello.txt");
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_buffered_stream_reader_invalid_data() {
        let mut reader =
            BufferedZipStreamReader::new_from_data(VecU8::from(b"not a zip".to_vec())).unwrap();
        let res = reader.read_next_file();
        // A bad local file header signature maps to InvalidArchive.
        let code = res.err().map(|e| e.code);
        expect_eq!(code, Some(ZipErrorCode::InvalidArgument));
    }
}
