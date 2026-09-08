// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use crate::{BufferedZipFile, FsZipFile, VecU8, ZipError};
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipArchive` from data.
    ///
    /// Returns an error if `data` is not a valid or supported zip archive.
    pub fn new_from_data(data: VecU8) -> Result<BufferedZipArchive, ZipError> {
        let mut archive = Self::default();
        archive.open(data)?;
        Ok(archive)
    }

    /// Opens a zip archive from data.
    ///
    /// Returns an error if the archive is already open or if `data` is not a
    /// valid or supported zip archive.
    fn open(&mut self, data: VecU8) -> Result<(), ZipError> {
        if self.reader.is_some() {
            return Err(ZipError::failed_precondition("Zip archive is already open"));
        }
        let cursor = Cursor::new(data.into_vec());
        let reader = WrappedZipArchive::new(cursor)
            .map_err(|e| ZipError::from_zip_error(e).with_context("Failed to open zip buffer"))?;
        self.reader = Some(Box::new(reader));
        Ok(())
    }

    /// Returns whether the zip archive is none (due to being
    /// default-constructed or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns the number of files in the archive.
    #[must_use]
    pub fn get_length(&self) -> usize {
        get_length_impl(&self.reader)
    }

    /// Returns the comment of the zip archive.
    #[must_use]
    pub fn get_comment(&self) -> VecU8 {
        get_comment_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an error if the archive is not open (`is_none()`), if `index` is
    /// out of bounds, or if the entry header is invalid or unsupported.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<BufferedZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, false).map(BufferedZipFile::new)
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an error if the archive is not open (`is_none()`), if `index` is
    /// out of bounds, or if the entry's local file header is invalid.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<BufferedZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, true).map(BufferedZipFile::new)
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `FsZipArchive` from a path.
    ///
    /// Propagates OS file-open errors (such as `NotFound` and
    /// `PermissionDenied`) and returns `InvalidArgument` if `path` is empty,
    /// contains NUL bytes, not valid UTF-8, or not a valid zip archive.
    pub fn new_from_path(path: &[u8]) -> Result<FsZipArchive, ZipError> {
        let mut archive = Self::default();
        archive.open(path)?;
        Ok(archive)
    }

    /// Opens a zip archive from a path.
    ///
    /// Returns an error if the archive is already open, if `path` cannot be
    /// opened, or if `path` does not point to a valid or supported zip archive.
    fn open(&mut self, path: &[u8]) -> Result<(), ZipError> {
        if self.reader.is_some() {
            return Err(ZipError::failed_precondition("Zip archive is already open"));
        }
        let path_str = ZipError::validate_non_empty_utf8_path(path, "Failed to open zip archive")?;
        let file = File::open(path_str)
            .map_err(|e| ZipError::from_io_error(e).with_context("Failed to open zip archive"))?;
        let reader = WrappedZipArchive::new(file)
            .map_err(|e| ZipError::from_zip_error(e).with_context("Failed to open zip archive"))?;
        self.reader = Some(Box::new(reader));
        Ok(())
    }

    /// Returns whether the zip archive is none (due to being
    /// default-constructed or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns the number of files in the archive.
    #[must_use]
    pub fn get_length(&self) -> usize {
        get_length_impl(&self.reader)
    }

    /// Returns the comment of the zip archive.
    #[must_use]
    pub fn get_comment(&self) -> VecU8 {
        get_comment_impl(&self.reader)
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an error if the archive is not open (`is_none()`), if `index` is
    /// out of bounds, or if the entry header is invalid or unsupported.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<FsZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, false).map(FsZipFile::new)
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an error if the archive is not open (`is_none()`), if `index` is
    /// out of bounds, or if the entry's local file header is invalid.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<FsZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, true).map(FsZipFile::new)
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

fn get_file_by_index_impl<'a, R: Read + Seek>(
    reader: &'a mut Option<Box<WrappedZipArchive<R>>>,
    index: usize,
    raw: bool,
) -> Result<WrappedZipFile<'a, R>, ZipError> {
    let reader =
        reader.as_mut().ok_or_else(|| ZipError::failed_precondition("Zip archive is not open"))?;
    let result = if raw { reader.by_index_raw(index) } else { reader.by_index(index) };
    result.map_err(ZipError::from_zip_error)
}

/// Shared body of `read_next_file` / `read_next_file_with_compressed_size` for
/// both stream readers.
///
/// Returns `Ok(None)` if the stream is already finished or once the start of
/// the central directory is reached (which also sets `finished`). Returns a
/// `FailedPrecondition` error if `reader` is `None` or if `poisoned` is set.
/// When `compressed_size` is `Some`, it is used for entries whose local header
/// carries no size (general purpose bit 3 / data descriptor).
///
/// Errors are sticky: when zip-rs rejects an entry it has already consumed the
/// local file header, leaving the stream positioned at the entry's untrusted
/// payload. Parsing that payload as the next header would let a crafted
/// archive smuggle entries past the error, so a failed read sets `poisoned`
/// and every later call fails with `FailedPrecondition`. `finished` is left
/// untouched (false) in that state.
fn read_next_file_impl<'a, R: Read>(
    reader: &'a mut Option<Box<R>>,
    finished: &mut bool,
    poisoned: &mut bool,
    compressed_size: Option<u64>,
) -> Result<Option<WrappedZipFile<'a, R>>, ZipError> {
    if *finished {
        return Ok(None);
    }
    if *poisoned {
        return Err(ZipError::failed_precondition(
            "Zip stream reader is unusable after an earlier read error",
        ));
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
        Err(e) => {
            *poisoned = true;
            Err(ZipError::from_zip_error(e))
        }
    }
}

/// A streaming zip reader that reads from an in-memory buffer without seeking.
#[derive(Default)]
pub struct BufferedZipStreamReader {
    reader: Option<Box<Cursor<Vec<u8>>>>,
    finished: bool,
    /// Set once a `read_next_file*` call has failed; see `read_next_file_impl`.
    poisoned: bool,
}

impl Debug for BufferedZipStreamReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferedZipStreamReader")
            .field("reader", &if self.reader.is_some() { "Some(Cursor<Vec<u8>>)" } else { "None" })
            .field("finished", &self.finished)
            .field("poisoned", &self.poisoned)
            .finish()
    }
}

impl BufferedZipStreamReader {
    /// Initializes an empty `BufferedZipStreamReader`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipStreamReader` from data.
    pub fn new_from_data(data: VecU8) -> Result<Self, ZipError> {
        Ok(Self {
            reader: Some(Box::new(Cursor::new(data.into_vec()))),
            finished: false,
            poisoned: false,
        })
    }

    /// Returns whether the stream reader is none (due to being default-constructed
    /// or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached. Stays
    /// false after a failed `read_next_file*` call.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Reads the next zip file from the stream.
    ///
    /// When there are no more files in the archive (start of central directory
    /// is reached), returns a `BufferedZipFile` where `is_none()` returns true.
    /// After any failed `read_next_file*` call, every later call returns a
    /// `FailedPrecondition` error (see `read_next_file_impl`).
    pub fn read_next_file(&mut self) -> Result<BufferedZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, &mut self.poisoned, None)?
            .map(BufferedZipFile::new)
            .unwrap_or_default())
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `BufferedZipFile`
    /// where `is_none()` returns true. After any failed `read_next_file*` call,
    /// every later call returns a `FailedPrecondition` error.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<BufferedZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(
            &mut self.reader,
            &mut self.finished,
            &mut self.poisoned,
            Some(compressed_size),
        )?
        .map(BufferedZipFile::new)
        .unwrap_or_default())
    }
}

/// A streaming zip reader that reads sequentially from a file without seeking.
#[derive(Default)]
pub struct FsZipStreamReader {
    reader: Option<Box<File>>,
    finished: bool,
    /// Set once a `read_next_file*` call has failed; see `read_next_file_impl`.
    poisoned: bool,
}

impl Debug for FsZipStreamReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsZipStreamReader")
            .field("reader", &if self.reader.is_some() { "Some(File)" } else { "None" })
            .field("finished", &self.finished)
            .field("poisoned", &self.poisoned)
            .finish()
    }
}

impl FsZipStreamReader {
    /// Initializes an empty `FsZipStreamReader`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `FsZipStreamReader` from a path.
    ///
    /// Propagates OS file-open errors (such as `NotFound` and
    /// `PermissionDenied`) and returns `InvalidArgument` if `path` is empty,
    /// contains NUL bytes, or is not valid UTF-8.
    pub fn new_from_path(path: &[u8]) -> Result<Self, ZipError> {
        let path_str = ZipError::validate_non_empty_utf8_path(path, "Failed to open zip stream")?;
        let file = File::open(path_str)
            .map_err(|e| ZipError::from_io_error(e).with_context("Failed to open zip stream"))?;
        Ok(Self { reader: Some(Box::new(file)), finished: false, poisoned: false })
    }

    /// Returns whether the stream reader is none (due to being default-constructed
    /// or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached. Stays
    /// false after a failed `read_next_file*` call.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Reads the next zip file from the stream.
    ///
    /// When there are no more files in the archive (start of central directory
    /// is reached), returns a `FsZipFile` where `is_none()` returns true.
    /// After any failed `read_next_file*` call, every later call returns a
    /// `FailedPrecondition` error (see `read_next_file_impl`).
    pub fn read_next_file(&mut self) -> Result<FsZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(&mut self.reader, &mut self.finished, &mut self.poisoned, None)?
            .map(FsZipFile::new)
            .unwrap_or_default())
    }

    /// Reads the next zip file from the stream with an assumed compressed size.
    ///
    /// When there are no more files in the archive, returns a `FsZipFile`
    /// where `is_none()` returns true. After any failed `read_next_file*` call,
    /// every later call returns a `FailedPrecondition` error.
    pub fn read_next_file_with_compressed_size(
        &mut self,
        compressed_size: u64,
    ) -> Result<FsZipFile<'_>, ZipError> {
        Ok(read_next_file_impl(
            &mut self.reader,
            &mut self.finished,
            &mut self.poisoned,
            Some(compressed_size),
        )?
        .map(FsZipFile::new)
        .unwrap_or_default())
    }
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

    /// Builds a two-entry stored archive whose entry 0 is flagged as encrypted
    /// (general purpose bit 0) in its local header, and whose entry 0 payload
    /// is itself a complete local file entry ("smuggled.txt"). zip-rs rejects
    /// entry 0 only after consuming its local header, so a reader that kept
    /// going would parse the smuggled entry as if it were a top-level one.
    fn create_zip_with_rejected_first_entry() -> Vec<u8> {
        let options = ZipWriterFileOptions::new().compression_method(CompressionMethod::Stored);

        let mut inner = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        inner.start_file(b"smuggled.txt", options).unwrap();
        inner.write_data(VecU8::from(b"smuggled".to_vec())).unwrap();
        let inner = inner.finish().unwrap().into_vec();
        let cd_pos = inner.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        let smuggled_entry = &inner[..cd_pos];
        assert!(smuggled_entry.starts_with(b"PK\x03\x04"));

        let mut outer = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        outer.start_file(b"first.bin", options).unwrap();
        outer.write_data(VecU8::copy_from_slice(smuggled_entry)).unwrap();
        outer.start_file(b"second.txt", options).unwrap();
        outer.write_data(VecU8::from(b"second".to_vec())).unwrap();
        let mut outer = outer.finish().unwrap().into_vec();
        // Local header general purpose flags are at offset 6..8.
        assert_eq!(&outer[..4], b"PK\x03\x04");
        outer[6] = 0x01;
        outer[7] = 0x00;
        outer
    }

    #[gtest]
    fn test_stream_reader_is_poisoned_after_read_error() {
        let zip_data = create_zip_with_rejected_first_entry();
        let mut reader = BufferedZipStreamReader::new_from_data(VecU8::from(zip_data)).unwrap();

        // Entry 0 is encrypted -> FailedPrecondition, after which zip-rs has
        // left the stream at the start of entry 0's payload.
        expect_eq!(
            reader.read_next_file().err().map(|e| e.code),
            Some(ZipErrorCode::FailedPrecondition)
        );
        expect_false!(reader.is_finished());
        expect_false!(reader.is_none());

        // Without poisoning, this call would parse the smuggled local header
        // inside entry 0's payload and succeed with "smuggled.txt".
        let second = reader.read_next_file();
        expect_eq!(second.as_ref().err().map(|e| e.code), Some(ZipErrorCode::FailedPrecondition));
        expect_true!(String::from_utf8_lossy(second.err().unwrap().message.as_slice())
            .contains("unusable after an earlier read error"));
        expect_eq!(
            reader.read_next_file_with_compressed_size(8).err().map(|e| e.code),
            Some(ZipErrorCode::FailedPrecondition)
        );
        expect_false!(reader.is_finished());
        expect_false!(reader.is_none());
    }

    #[gtest]
    fn test_fs_stream_reader_is_poisoned_after_read_error() {
        let zip_data = create_zip_with_rejected_first_entry();
        let path = std::env::temp_dir()
            .join(format!("poisoned_stream_977770313_{}.zip", std::process::id()));
        std::fs::write(&path, &zip_data).unwrap();
        let mut reader =
            FsZipStreamReader::new_from_path(path.to_str().unwrap().as_bytes()).unwrap();
        expect_eq!(
            reader.read_next_file().err().map(|e| e.code),
            Some(ZipErrorCode::FailedPrecondition)
        );
        expect_eq!(
            reader.read_next_file().err().map(|e| e.code),
            Some(ZipErrorCode::FailedPrecondition)
        );
        expect_false!(reader.is_finished());
        let _ = std::fs::remove_file(&path);
    }

    #[gtest]
    fn test_fs_open_propagates_not_found_and_validates_path() {
        let dir = std::env::temp_dir();
        let missing = dir.join(format!("nonexistent_zip_977770313_{}.zip", std::process::id()));
        let _ = std::fs::remove_file(&missing);
        let missing_bytes = missing.to_str().unwrap().as_bytes();

        expect_eq!(
            FsZipArchive::new_from_path(missing_bytes).err().map(|e| e.code),
            Some(ZipErrorCode::NotFound)
        );
        expect_eq!(
            FsZipStreamReader::new_from_path(missing_bytes).err().map(|e| e.code),
            Some(ZipErrorCode::NotFound)
        );

        expect_eq!(
            FsZipArchive::new_from_path(b"").err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
        expect_eq!(
            FsZipStreamReader::new_from_path(b"").err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );

        expect_eq!(
            FsZipArchive::new_from_path(&[0xff, 0xfe]).err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
        expect_eq!(
            FsZipStreamReader::new_from_path(&[0xff, 0xfe]).err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
    }

    #[gtest]
    fn test_fs_open_rejects_path_with_embedded_nul() {
        expect_eq!(
            FsZipArchive::new_from_path(b"a\0b.zip").err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
        expect_eq!(
            FsZipStreamReader::new_from_path(b"a\0b.zip").err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
    }

    #[gtest]
    fn test_archive_get_file_by_index_propagates_zip_error() {
        let zip_data = create_test_zip();

        let mut archive = BufferedZipArchive::new_from_data(zip_data.clone()).unwrap();
        expect_eq!(
            archive.get_file_by_index(99).err().map(|e| e.code),
            Some(ZipErrorCode::OutOfRange)
        );

        let mut corrupt = zip_data.into_vec();
        corrupt[..4].copy_from_slice(b"XXXX");
        let mut corrupt_archive = BufferedZipArchive::new_from_data(VecU8::from(corrupt)).unwrap();
        expect_eq!(
            corrupt_archive.get_file_by_index(0).err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
        expect_eq!(
            corrupt_archive.get_file_by_index_raw(0).err().map(|e| e.code),
            Some(ZipErrorCode::InvalidArgument)
        );
    }
}
