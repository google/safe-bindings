// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use crate::callback_reader::CallbackReader;
use crate::{BufferedZipFile, CallbackZipFile, FsZipFile, VecU8, ZipEntryMetadata, ZipError};
use std::ffi::c_void;
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
        get_file_by_index_impl(&mut self.reader, index, false).map(BufferedZipFile::new)
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an error if the archive is not open (`is_none()`).
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<BufferedZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, true).map(BufferedZipFile::new)
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
    /// Path the archive was opened from, retained so that `try_clone` can open
    /// an independent file descriptor onto the same archive. Empty until
    /// `open` succeeds.
    path: String,
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
                    self.path = path_str.to_string();
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            },
            Err(e) => Err(e.to_string()),
        }
    }

    /// Returns an independent handle onto the same archive.
    ///
    /// The new handle shares the central directory that `new_from_path` already
    /// parsed, so this costs one `open(2)` and is O(1) in the number of entries,
    /// whereas `new_from_path` is O(entries). Use it when several entries must
    /// be read at once: `get_file_by_index` borrows the archive mutably, so a
    /// caller that wants N concurrently open entries needs N handles, and
    /// reopening by path for each one is quadratic on large archives.
    ///
    /// Each handle owns its own file descriptor and therefore its own file
    /// position, so handles may be read independently.
    ///
    /// Returns `ZipErrorCode::FailedPrecondition` if the archive is not open
    /// (default-constructed or moved-from) or if the path can no longer be
    /// opened. The caller must ensure the path still resolves to the same,
    /// unmodified file it was opened from: not renamed, replaced or rewritten
    /// in between (for a relative path, the working directory must not have
    /// changed either). The shared central directory would otherwise describe
    /// byte ranges that no longer hold the entries it names.
    pub fn try_clone(&self) -> Result<FsZipArchive, ZipError> {
        let Some(reader) = self.reader.as_ref() else {
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) => {
                return Err(ZipError::failed_precondition(format!(
                    "Failed to reopen zip archive: {}",
                    e
                )))
            }
        };
        // SAFETY: `unsafe_new_with_metadata` requires `reader.metadata()` to
        // describe the bytes `file` will yield. `file` is a freshly opened
        // descriptor for the same path `reader` parsed its central directory
        // from, and this function's contract (see the doc comment) makes the
        // caller responsible for that path still resolving to the same,
        // unmodified file. Should it not anyway, every subsequent read goes
        // through zip-rs's bounds, signature and CRC checks, so a mismatch
        // surfaces as an error rather than as memory unsafety.
        let archive =
            unsafe { WrappedZipArchive::unsafe_new_with_metadata(file, reader.metadata()) };
        Ok(FsZipArchive { reader: Some(Box::new(archive)), path: self.path.clone() })
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
        get_file_by_index_impl(&mut self.reader, index, false).map(FsZipFile::new)
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an error if the archive is not open (`is_none()`).
    /// Returns an error if `index` is out of bounds.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<FsZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, true).map(FsZipFile::new)
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
pub struct CallbackZipArchive {
    reader: Option<Box<WrappedZipArchive<CallbackReader>>>,
    /// Total number of bytes the archive was opened from, kept so that
    /// `try_clone_with_callbacks` can reject a reader of a different length.
    len: u64,
}

impl Debug for CallbackZipArchive {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("CallbackZipArchive")
            .field(
                "reader",
                &if self.reader.is_some() { "Some(CallbackZipArchive)" } else { "None" },
            )
            .finish()
    }
}

impl CallbackZipArchive {
    /// Initializes an empty `CallbackZipArchive`.
    ///
    /// An archive created this way will be in an empty state. Use
    /// `new_from_callbacks` to read an archive through callbacks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an archive that reads its bytes by calling back into the caller.
    ///
    /// Opening only reads the end-of-central-directory record and the central
    /// directory, so an archive held in remote storage does not have to be
    /// downloaded in full; individual entries are fetched when they are opened.
    ///
    /// `read_at` must read up to `len` bytes from `context` starting at
    /// `offset` into `buf` and return the number of bytes read; it must return
    /// 0 only at end of input, and a negative value if the read failed. It may
    /// return fewer bytes than requested. `size` must return the total number
    /// of bytes in the archive, or a negative value on failure. `release` is
    /// called exactly once, once this archive and all of its clones are gone;
    /// it is called even if this function fails.
    ///
    /// Returns an error if the size cannot be determined or the bytes are not a
    /// valid zip archive.
    ///
    /// # Safety
    ///
    /// The caller must guarantee all of the following.
    ///
    /// * `context` is valid until `release` is called with it.
    /// * All three callbacks accept `context` and may be invoked from any thread. Handles obtained
    ///   through `try_clone` share `context` and are meant to be used from different threads at
    ///   once, so `read_at` must be safe to call concurrently with itself. `size` is called once,
    ///   before any clone exists, and `release` runs after the last clone is dropped, so neither is
    ///   ever concurrent with another callback.
    /// * `read_at` writes no more than `len` bytes to `buf` and returns at most `len`.
    /// * None of the callbacks unwind. A C++ exception crossing into Rust is undefined behavior.
    pub unsafe fn new_from_callbacks(
        read_at: unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i64,
        size: unsafe extern "C" fn(*mut c_void) -> i64,
        release: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> Result<CallbackZipArchive, ZipError> {
        // SAFETY: forwarding this function's own contract, which is identical.
        let reader = unsafe { CallbackReader::try_new(read_at, size, release, context) }?;
        let len = reader.len();
        // With the default `ArchiveOffset::Detect`, zip 9.x locates the central
        // directory by scanning the archive *forwards from byte 0*, i.e. it
        // reads the whole archive just to open it. That defeats the purpose of
        // a lazy, callback-backed reader, so require the archive to start at
        // offset 0 (no prepended data), which only needs the end of the file.
        let config = zip::read::Config { archive_offset: zip::read::ArchiveOffset::Known(0) };
        match WrappedZipArchive::with_config(config, reader) {
            Ok(reader) => Ok(Self { reader: Some(Box::new(reader)), len }),
            Err(e) => Err(ZipError::from_zip_error(e)),
        }
    }

    /// Returns an independent handle onto the same archive.
    ///
    /// Unlike `FsZipArchive::try_clone` this performs no I/O whatsoever: the
    /// clone shares both the already parsed central directory and the callback
    /// context, and only tracks its own read position. Use it when several
    /// entries must be read at once, since `get_file_by_index` borrows the
    /// archive mutably and so each concurrently open entry needs its own
    /// handle.
    ///
    /// Returns an error if the archive is not open.
    pub fn try_clone(&self) -> Result<CallbackZipArchive, ZipError> {
        let Some(reader) = self.reader.as_ref() else {
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        Ok(CallbackZipArchive { reader: Some(reader.clone()), len: self.len })
    }

    /// Returns an independent handle onto the same archive that reads through a
    /// fresh set of callbacks.
    ///
    /// Like `try_clone` this reuses the central directory that
    /// `new_from_callbacks` already parsed and so is O(1) in the number of
    /// entries, but the new handle does not share the original's context. That
    /// is what a caller wants when the context carries per-handle state such as
    /// a read cache, which two handles reading two different entries would
    /// otherwise thrash, or when the context is not thread-safe and the handles
    /// are to be used from different threads.
    ///
    /// The new callbacks must serve exactly the bytes the original ones did.
    /// The shared central directory would otherwise describe byte ranges that
    /// do not hold the entries it names. That cannot be checked in full, but a
    /// reader whose `size` differs from the one this archive was opened from
    /// certainly does not qualify and is rejected with
    /// `ZipErrorCode::InvalidArgument`.
    ///
    /// Returns `ZipErrorCode::FailedPrecondition` if the archive is not open,
    /// `ZipErrorCode::Internal` if `size` fails, and
    /// `ZipErrorCode::InvalidArgument` if `size` disagrees with the original.
    /// `release` is called on `context` in every case, including failure.
    ///
    /// # Safety
    ///
    /// The caller must uphold the same guarantees as `new_from_callbacks`.
    pub unsafe fn try_clone_with_callbacks(
        &self,
        read_at: unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i64,
        size: unsafe extern "C" fn(*mut c_void) -> i64,
        release: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> Result<CallbackZipArchive, ZipError> {
        let Some(original) = self.reader.as_ref() else {
            // Ownership of `context` transferred on call, so release it before
            // failing without invoking `size`.
            // SAFETY: by this function's contract, `release` accepts `context`.
            unsafe { release(context) };
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        // SAFETY: forwarding this function's own contract, which is identical.
        let reader = unsafe { CallbackReader::try_new(read_at, size, release, context) }?;
        // The contract above cannot be checked in full, but a reader of a
        // different length certainly does not serve the same bytes, and this
        // catches the common mistake of handing over the wrong object.
        if reader.len() != self.len {
            return Err(ZipError::invalid_argument(format!(
                "The new reader has {} bytes but the archive was opened from {} bytes",
                reader.len(),
                self.len
            )));
        }
        // SAFETY: by this function's contract the new reader serves the same
        // bytes as the one whose central directory `original` parsed, which is
        // the correspondence `unsafe_new_with_metadata` requires.
        let archive =
            unsafe { WrappedZipArchive::unsafe_new_with_metadata(reader, original.metadata()) };
        Ok(CallbackZipArchive { reader: Some(Box::new(archive)), len: self.len })
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
    /// Returns an error if the archive is not open, `index` is out of bounds,
    /// or the entry's local file header is invalid or unsupported. Opening an
    /// entry reads its local file header through the callbacks, so a failing
    /// `read_at` is also reported here.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<CallbackZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index(index) {
                Ok(file) => Ok(CallbackZipFile::new(file)),
                Err(e) => Err(ZipError::from_zip_error(e)),
            },
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
        }
    }

    /// Returns a raw (compressed) zip file by its index.
    ///
    /// Warning: Writers in zip-rs do not support writing compressed data as-is.
    /// This data will be compressed again when writing.
    ///
    /// Returns an error if the archive is not open, `index` is out of bounds,
    /// or the entry's local file header is invalid. Opening an entry reads its
    /// local file header through the callbacks, so a failing `read_at` is also
    /// reported here.
    pub fn get_file_by_index_raw(&mut self, index: usize) -> Result<CallbackZipFile<'_>, ZipError> {
        match self.reader.as_mut() {
            Some(reader) => match reader.by_index_raw(index) {
                Ok(file) => Ok(CallbackZipFile::new(file)),
                Err(e) => Err(ZipError::from_zip_error(e)),
            },
            None => Err(ZipError::failed_precondition("Zip archive is not open")),
        }
    }

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry. For this archive the
    /// reads go through the caller's callbacks, which is typically a network,
    /// so listing with `get_file_by_index` costs one round trip per entry.
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

fn get_file_by_index_impl<'a, R: Read + Seek>(
    reader: &'a mut Option<Box<WrappedZipArchive<R>>>,
    index: usize,
    raw: bool,
) -> Result<WrappedZipFile<'a, R>, ZipError> {
    let reader =
        reader.as_mut().ok_or_else(|| ZipError::failed_precondition("Zip archive is not open"))?;
    let result = if raw { reader.by_index_raw(index) } else { reader.by_index(index) };
    result.map_err(|e| ZipError::out_of_range(e.to_string()))
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipStreamReader` from data.
    pub fn new_from_data(data: VecU8) -> Result<Self, ZipError> {
        Ok(Self { reader: Some(Box::new(Cursor::new(data.into_vec()))), finished: false })
    }

    /// Returns whether the stream reader is none (due to being default-constructed
    /// or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns whether the end of the archive stream has been reached.
    #[must_use]
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
    use std::io::Write;

    fn create_test_zip() -> VecU8 {
        let mut writer = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        let options = ZipWriterFileOptions::new().compression_method(CompressionMethod::Stored);
        writer.start_file(b"hello.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Hello, world!".to_vec())).unwrap();
        writer.start_file(b"second.txt", options).unwrap();
        writer.write_data(VecU8::from(b"Second file content".to_vec())).unwrap();
        writer.finish().unwrap()
    }

    /// Writes `create_test_zip()` to a uniquely named file under the test's
    /// temporary directory and returns the path.
    fn create_test_zip_file(name: &str) -> String {
        let dir = std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = format!("{}/{}.zip", dir, name);
        let mut file = File::create(&path).unwrap();
        file.write_all(create_test_zip().as_slice()).unwrap();
        file.flush().unwrap();
        path
    }

    #[gtest]
    fn test_fs_archive_try_clone_reads_the_same_entries() {
        let path = create_test_zip_file("try_clone_same_entries");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();
        let mut clone = archive.try_clone().unwrap();

        expect_false!(clone.is_none());
        expect_eq!(clone.get_length(), archive.get_length());
        let mut file = clone.get_file_by_index(0).unwrap();
        expect_eq!(file.get_file_name().as_slice(), b"hello.txt");
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_fs_archive_clones_have_independent_positions() {
        let path = create_test_zip_file("try_clone_independent");
        let mut archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();
        let mut clone = archive.try_clone().unwrap();

        // Each handle owns its own descriptor, so reading one entry to
        // completion must not disturb the other handle's entry. This is the
        // whole point of `try_clone`: `get_file_by_index` borrows the archive
        // mutably, so two entries can only be open at once via two handles.
        // Open both before reading either, then read them in the opposite
        // order, so that a shared file position would be detected.
        let mut first = archive.get_file_by_index(0).unwrap();
        let mut second = clone.get_file_by_index(1).unwrap();
        expect_eq!(second.get_file_data().unwrap().as_slice(), b"Second file content");
        expect_eq!(first.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_fs_archive_try_clone_fails_when_not_open() {
        let archive = FsZipArchive::new();
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::FailedPrecondition);
    }

    /// Callback context over an owned byte buffer, for `CallbackZipArchive`
    /// tests. The `CallbackReader` tests cover the reader itself; here the
    /// callbacks only need to serve bytes.
    struct BytesContext {
        data: Vec<u8>,
    }

    unsafe extern "C" fn bytes_read_at(
        context: *mut c_void,
        offset: u64,
        buf: *mut u8,
        len: usize,
    ) -> i64 {
        // SAFETY: tests only ever pass a pointer to a live `BytesContext`.
        let data = unsafe { &(*(context as *const BytesContext)).data };
        let offset = offset as usize;
        if offset >= data.len() {
            return 0;
        }
        let len = len.min(data.len() - offset);
        // SAFETY: the caller guarantees `buf` is valid for `len` bytes, and
        // `len` was clamped to what is available.
        unsafe { std::ptr::copy_nonoverlapping(data[offset..].as_ptr(), buf, len) };
        len as i64
    }

    unsafe extern "C" fn bytes_size(context: *mut c_void) -> i64 {
        // SAFETY: tests only ever pass a pointer to a live `BytesContext`.
        let data = unsafe { &(*(context as *const BytesContext)).data };
        data.len() as i64
    }

    unsafe extern "C" fn bytes_release(context: *mut c_void) {
        // SAFETY: the pointer came from `Box::into_raw` in `bytes_context`, and
        // `release` is called at most once.
        drop(unsafe { Box::from_raw(context as *mut BytesContext) });
    }

    fn bytes_context(data: Vec<u8>) -> *mut c_void {
        Box::into_raw(Box::new(BytesContext { data })) as *mut c_void
    }

    #[gtest]
    fn test_callback_archive_rejects_a_clone_reader_of_a_different_length() {
        let data = create_test_zip().as_slice().to_vec();
        // SAFETY: the callbacks all accept the boxed `BytesContext`, none of
        // them unwind, and `bytes_release` frees it exactly once.
        let archive = unsafe {
            CallbackZipArchive::new_from_callbacks(
                bytes_read_at,
                bytes_size,
                bytes_release,
                bytes_context(data.clone()),
            )
        }
        .unwrap();

        // A reader over the same bytes is accepted and serves the entries.
        // SAFETY: as above.
        let mut same = unsafe {
            archive.try_clone_with_callbacks(
                bytes_read_at,
                bytes_size,
                bytes_release,
                bytes_context(data.clone()),
            )
        }
        .unwrap();
        let mut file = same.get_file_by_index(0).unwrap();
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");

        // One byte more and the new reader cannot be serving the archive the
        // shared central directory describes, so it is rejected up front rather
        // than yielding garbled entries later.
        let mut longer = data.clone();
        longer.push(b'x');
        // SAFETY: as above.
        let err = unsafe {
            archive.try_clone_with_callbacks(
                bytes_read_at,
                bytes_size,
                bytes_release,
                bytes_context(longer),
            )
        }
        .unwrap_err();
        expect_eq!(err.code, ZipErrorCode::InvalidArgument);
    }

    #[gtest]
    fn test_callback_archive_try_clone_with_callbacks_when_not_open_skips_size_and_releases() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct ProbeContext {
            size_calls: Arc<AtomicUsize>,
            release_calls: Arc<AtomicUsize>,
        }

        unsafe extern "C" fn probe_size(context: *mut c_void) -> i64 {
            // SAFETY: `context` points to a live `ProbeContext`.
            let probe = unsafe { &*(context as *const ProbeContext) };
            probe.size_calls.fetch_add(1, Ordering::Relaxed);
            -1
        }

        unsafe extern "C" fn probe_release(context: *mut c_void) {
            // SAFETY: `context` came from `Box::into_raw` below and is freed once.
            let probe = unsafe { Box::from_raw(context as *mut ProbeContext) };
            probe.release_calls.fetch_add(1, Ordering::Relaxed);
        }

        let size_calls = Arc::new(AtomicUsize::new(0));
        let release_calls = Arc::new(AtomicUsize::new(0));
        let context = Box::into_raw(Box::new(ProbeContext {
            size_calls: size_calls.clone(),
            release_calls: release_calls.clone(),
        })) as *mut c_void;

        let archive = CallbackZipArchive::new();
        // SAFETY: callbacks accept `ProbeContext`, do not unwind, and release it once.
        let err = unsafe {
            archive.try_clone_with_callbacks(bytes_read_at, probe_size, probe_release, context)
        }
        .unwrap_err();

        expect_eq!(err.code, ZipErrorCode::FailedPrecondition);
        expect_eq!(size_calls.load(Ordering::Relaxed), 0);
        expect_eq!(release_calls.load(Ordering::Relaxed), 1);
    }

    #[gtest]
    fn test_fs_archive_try_clone_reopen_failure() {
        let path = create_test_zip_file("try_clone_reopen_failure");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();
        std::fs::remove_file(&path).unwrap();
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::FailedPrecondition);
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
        expect_false!(first.is_none());
        expect_eq!(first.clone(), first);
        expect_true!(ZipEntryMetadata::default().is_none());
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
