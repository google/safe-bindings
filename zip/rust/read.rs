// Crubit does not support generic type parameters, so we need to implement
// BufferedZipArchive and FsZipArchive manually.

use crate::callback_reader::CallbackReader;
use crate::{BufferedZipFile, CallbackZipFile, FsZipFile, VecU8, ZipEntryMetadata, ZipError};
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Cursor, IoSliceMut, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex, MutexGuard};
use zip::read::{ArchiveOffset, Config, ZipArchiveMetadata, ZipFile as WrappedZipFile};
use zip::ZipArchive as WrappedZipArchive;

/// An open archive's central directory, kept beside the reader rather than
/// read through it.
///
/// A `*ZipFile` returned by `get_file_by_index*` holds a `&mut` into the boxed
/// `WrappedZipArchive` for as long as it lives. Crubit cannot make C++ respect
/// that borrow, and C++ callers legitimately call the `&self` accessors
/// (`get_length`, `get_comment`, `get_entry_metadata`) while an entry is open;
/// dereferencing the box there would create a `&WrappedZipArchive` aliasing
/// that `&mut`, which Rust's aliasing rules forbid even though nothing is
/// written. The parsed central directory is immutable and reference-counted
/// inside the zip crate, so holding our own `Arc` to it and a copy of the
/// comment lets those accessors never touch the box.
#[derive(Clone, Default)]
struct CentralDirectory {
    metadata: Option<Arc<ZipArchiveMetadata>>,
    comment: Vec<u8>,
}

impl CentralDirectory {
    fn new<R: Read + Seek>(archive: &WrappedZipArchive<R>) -> Self {
        Self { metadata: Some(archive.metadata()), comment: archive.comment().to_vec() }
    }

    /// Returns the number of entries, or 0 if the archive is not open.
    fn len(&self) -> usize {
        self.metadata.as_ref().map_or(0, |metadata| metadata.len())
    }

    /// Returns the archive comment, or an empty vector if the archive is not
    /// open.
    fn comment(&self) -> VecU8 {
        VecU8::copy_from_slice(&self.comment)
    }

    /// Returns what the central directory records about the entry at `index`.
    ///
    /// Returns `FailedPrecondition` if the archive is not open and `OutOfRange`
    /// if `index` is out of bounds, the same codes `get_file_by_index` uses.
    fn entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        let Some(metadata) = self.metadata.as_ref() else {
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        let entry = metadata.entry(index).map_err(ZipError::from_zip_error)?;
        Ok(ZipEntryMetadata::from_entry(&entry))
    }
}

/// Hands a staged replacement to the next `Clone::clone` of the reader an
/// archive was opened on. That is how `zip::ZipArchive::clone` is made to
/// clone its parsed central directory (`Arc<ZipArchiveMetadata>`) onto a
/// reader of the caller's choosing, in safe Rust and without reparsing.
///
/// `T` is what gets staged: a `File` for `FsReader`, a `CallbackReader` for
/// itself. Callers hold `lock()` across staging, cloning and the check that
/// the staged value was taken, so that two concurrent clones cannot adopt each
/// other's replacement.
///
/// The mutexes are `expect`ed rather than recovered from poisoning: a panic
/// while a replacement is staged must not let a later clone on another thread
/// silently adopt it.
#[derive(Debug)]
pub(crate) struct CloneSlot<T> {
    lock: Mutex<()>,
    staged: Mutex<Option<T>>,
}

impl<T> Default for CloneSlot<T> {
    fn default() -> Self {
        Self { lock: Mutex::new(()), staged: Mutex::new(None) }
    }
}

impl<T> CloneSlot<T> {
    /// Serializes stage / clone / take sequences against each other.
    pub(crate) fn lock(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().expect("CloneSlot clone mutex poisoned")
    }

    /// Stages `value` for the next clone to adopt.
    pub(crate) fn stage(&self, value: T) {
        *self.staged.lock().expect("CloneSlot staged mutex poisoned") = Some(value);
    }

    /// Removes and returns whatever is staged, if anything.
    pub(crate) fn take_staged(&self) -> Option<T> {
        self.staged.lock().expect("CloneSlot staged mutex poisoned").take()
    }
}

/// What identifies the file an `FsZipArchive` was opened from, so that
/// `try_clone` can tell whether the path still names that same file.
///
/// Length always participates. On Unix the device and inode numbers do too,
/// which catches the file being replaced by another of the same length
/// (atomic rename, remove and recreate). An in-place rewrite that keeps the
/// inode and the length is not detectable this way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FileIdentity {
    len: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl FileIdentity {
    fn of(file: &File) -> std::io::Result<Self> {
        let metadata = file.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self { len: metadata.len(), dev: metadata.dev(), ino: metadata.ino() })
        }
        #[cfg(not(unix))]
        {
            Ok(Self { len: metadata.len() })
        }
    }
}

/// A `File` wrapper that implements `Clone` by adopting a freshly opened
/// descriptor staged in its `CloneSlot`, allowing `zip::ZipArchive<FsReader>`
/// to clone its parsed central directory (`Arc<ZipArchiveMetadata>`) in safe
/// Rust without reparsing.
#[derive(Debug)]
pub(crate) struct FsReader {
    file: File,
    clone_slot: Option<Arc<CloneSlot<File>>>,
}

impl FsReader {
    fn with_clone_slot(file: File, clone_slot: Arc<CloneSlot<File>>) -> Self {
        Self { file, clone_slot: Some(clone_slot) }
    }

    fn uncloneable(file: File) -> Self {
        Self { file, clone_slot: None }
    }
}

impl Clone for FsReader {
    fn clone(&self) -> Self {
        let slot = self.clone_slot.as_ref().expect("FsReader::clone requires a clone slot");
        let file = slot.take_staged().expect("FsReader::clone requires a staged File");
        Self { file, clone_slot: Some(Arc::clone(slot)) }
    }
}

impl Read for FsReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
        self.file.read_vectored(bufs)
    }
}

impl Seek for FsReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.file.stream_position()
    }
}

#[derive(Default)]
pub struct BufferedZipArchive {
    reader: Option<Box<WrappedZipArchive<Cursor<Vec<u8>>>>>,
    central_directory: CentralDirectory,
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
        self.central_directory = CentralDirectory::new(&reader);
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
        self.central_directory.len()
    }

    /// Returns the comment of the zip archive.
    #[must_use]
    pub fn get_comment(&self) -> VecU8 {
        self.central_directory.comment()
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

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything. Usable while an entry is open.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry.
    ///
    /// Returns `FailedPrecondition` if the archive is not open and `OutOfRange`
    /// if `index` is out of bounds.
    pub fn get_entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        self.central_directory.entry_metadata(index)
    }
}

#[derive(Default)]
pub struct FsZipArchive {
    reader: Option<Box<WrappedZipArchive<FsReader>>>,
    central_directory: CentralDirectory,
    /// Path the archive was opened from, retained so that `try_clone` can open
    /// an independent file descriptor onto the same archive. Empty until
    /// `open` succeeds.
    path: String,
    /// Identity of the file at `path` when it was opened, checked by
    /// `try_clone` against the reopened file. Zero until `open` succeeds.
    identity: FileIdentity,
    clone_slot: Option<Arc<CloneSlot<File>>>,
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
        let identity = FileIdentity::of(&file)
            .map_err(|e| ZipError::from_io_error(e).with_context("Failed to open zip archive"))?;
        let clone_slot = Arc::new(CloneSlot::default());
        let fs_reader = FsReader::with_clone_slot(file, Arc::clone(&clone_slot));
        let reader = WrappedZipArchive::new(fs_reader)
            .map_err(|e| ZipError::from_zip_error(e).with_context("Failed to open zip archive"))?;
        self.central_directory = CentralDirectory::new(&reader);
        self.reader = Some(Box::new(reader));
        self.path = path_str.to_string();
        self.identity = identity;
        self.clone_slot = Some(clone_slot);
        Ok(())
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
    /// The path must still resolve to the same, unmodified file the archive was
    /// opened from (for a relative path, the working directory must not have
    /// changed either), since the shared central directory would otherwise
    /// describe byte ranges that no longer hold the entries it names. The
    /// reopened file's identity (length, plus device and inode on Unix) is
    /// compared with what `new_from_path` saw, so a file that was replaced,
    /// truncated or appended to in between is detected and reported as
    /// `ZipErrorCode::FailedPrecondition`. An in-place rewrite that keeps the
    /// inode and the length is not detected.
    ///
    /// Also returns `ZipErrorCode::FailedPrecondition` if the archive is not
    /// open (default-constructed or moved-from), and propagates OS file-open
    /// errors (such as `ZipErrorCode::NotFound` and
    /// `ZipErrorCode::PermissionDenied`) if the path can no longer be opened.
    pub fn try_clone(&self) -> Result<FsZipArchive, ZipError> {
        let (Some(reader), Some(clone_slot)) = (self.reader.as_ref(), self.clone_slot.as_ref())
        else {
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        let file = File::open(&self.path)
            .map_err(|e| ZipError::from_io_error(e).with_context("Failed to reopen zip archive"))?;
        let identity = FileIdentity::of(&file)
            .map_err(|e| ZipError::from_io_error(e).with_context("Failed to reopen zip archive"))?;
        if identity != self.identity {
            return Err(ZipError::failed_precondition(format!(
                "Zip archive at {} was replaced or modified since it was opened",
                self.path
            )));
        }
        let archive = {
            let _guard = clone_slot.lock();
            clone_slot.stage(file);
            let archive = (**reader).clone();
            // `zip::ZipArchive::clone` clones its reader exactly once, which
            // adopts the staged file. Anything still staged means it did not,
            // and `archive` would share a descriptor rather than own one.
            if clone_slot.take_staged().is_some() {
                return Err(ZipError::internal(
                    "zip::ZipArchive::clone did not take the staged file",
                ));
            }
            archive
        };
        Ok(FsZipArchive {
            reader: Some(Box::new(archive)),
            central_directory: self.central_directory.clone(),
            path: self.path.clone(),
            identity: self.identity,
            clone_slot: Some(Arc::clone(clone_slot)),
        })
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
        self.central_directory.len()
    }

    /// Returns the comment of the zip archive.
    #[must_use]
    pub fn get_comment(&self) -> VecU8 {
        self.central_directory.comment()
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

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything. Usable while an entry is open.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry.
    ///
    /// Returns `FailedPrecondition` if the archive is not open and `OutOfRange`
    /// if `index` is out of bounds.
    pub fn get_entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        self.central_directory.entry_metadata(index)
    }
}

/// A zip archive whose bytes are read through caller-supplied callbacks, so
/// that only the central directory and the entries that are read are ever
/// fetched. See `new_from_callbacks` for what the callbacks must do.
#[derive(Default)]
pub struct CallbackZipArchive {
    reader: Option<Box<WrappedZipArchive<CallbackReader>>>,
    central_directory: CentralDirectory,
    clone_slot: Option<Arc<CloneSlot<CallbackReader>>>,
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
    /// `offset`, stage exactly the bytes it read with a single call to
    /// [`crate::stage_callback_chunk`] (which replaces anything staged
    /// before), and return their count, or a negative value if the read
    /// failed. It may return fewer bytes than requested, but not 0: it is only
    /// asked for bytes that `size` said are there, so a count of 0 is reported
    /// as truncated input. `size` must return the total number of bytes in the
    /// archive, or a negative value on failure. `release` is called with
    /// `context` exactly once, once this archive and all of its clones are
    /// gone; it is called even if this function fails.
    ///
    /// Handles obtained through `try_clone` share `context` and may be used
    /// from different threads at once, so `read_at` must be safe to call
    /// concurrently with itself from any thread. `size` is called once, before
    /// any clone exists, and `release` runs after the last clone is dropped, so
    /// neither overlaps another callback. None of the callbacks may unwind
    /// across FFI.
    ///
    /// Returns an error if the size cannot be determined or the bytes are not a
    /// valid zip archive that starts at offset 0. The first `read_at` call that
    /// fails or breaks the contract above fails the open as well, also where
    /// the `zip` crate would have carried on from it, and `read_at` is not
    /// called again: a failure fails the open as a reader callback failure
    /// (`ZipError::is_reader_callback_failure`), a count of 0 with
    /// `ZipErrorCode::InvalidArgument`, and any other breach with
    /// `ZipErrorCode::Internal`.
    pub fn new_from_callbacks(
        read_at: extern "C" fn(usize, u64, usize) -> i64,
        size: extern "C" fn(usize) -> i64,
        release: extern "C" fn(usize),
        context: usize,
    ) -> Result<CallbackZipArchive, ZipError> {
        let mut reader = CallbackReader::try_new(read_at, size, release, context)?;
        let len = reader.len();
        let clone_slot = reader.clone_slot();
        // With the default `ArchiveOffset::Detect`, zip 9.x locates the central
        // directory by scanning the archive *forwards from byte 0*, i.e. it
        // reads the whole archive just to open it. That defeats the purpose of
        // a lazy, callback-backed reader, so require the archive to start at
        // offset 0 (no prepended data), which only needs the end of the file.
        let config = Config { archive_offset: ArchiveOffset::Known(0) };
        // zip 9.x does not propagate every error of a read. While it locates
        // the central directory it treats a read of a signature or of the
        // ZIP64 records that goes wrong like a signature or record that is not
        // there, and carries on. A valid archive is then reported as corrupt,
        // or, when only its ZIP64 records could not be read, opened from its
        // 32-bit record with no more than the first 65535 of its entries.
        // Nothing `zip` concluded after a read went wrong can be relied on, so
        // the open fails with what went wrong, whatever `zip` returned.
        //
        // Nor is anything `zip` reads after that of any use. Left to itself it
        // would go through a whole central directory, or scan back over the
        // last 64 KiB of the archive, for a result that is discarded. While
        // the reader is watched, the first read that goes wrong therefore
        // fails every read after it without calling back.
        let open_watch = reader.watch();
        let opened = WrappedZipArchive::with_config(config, reader);
        open_watch.check()?;
        let reader = opened.map_err(ZipError::from_zip_error)?;
        // zip 9.x does not hold an archive without entries to the known
        // offset: with no central directory header to look for there, it
        // accepts the archive wherever its end-of-central-directory record
        // places it. Check the offset here, so that data in front of the
        // archive is rejected whether or not the archive has entries.
        if reader.offset() != 0 {
            return Err(ZipError::invalid_argument(
                "The zip archive does not start at offset 0 of the reader",
            ));
        }
        // The watch was for the open only: a read of the open archive that
        // goes wrong must not fail the reads after it, or it could not be
        // retried. A clone of a reader is never watched, so keep a clone of
        // the archive, which shares the parsed central directory and reads
        // nothing, in place of the one that was opened.
        let reader = reader.clone();
        Ok(Self {
            central_directory: CentralDirectory::new(&reader),
            reader: Some(Box::new(reader)),
            clone_slot: Some(clone_slot),
            len,
        })
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
        let (Some(reader), Some(clone_slot)) = (self.reader.as_ref(), self.clone_slot.as_ref())
        else {
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        let archive = {
            let _guard = clone_slot.lock();
            (**reader).clone()
        };
        Ok(CallbackZipArchive {
            reader: Some(Box::new(archive)),
            central_directory: self.central_directory.clone(),
            clone_slot: Some(Arc::clone(clone_slot)),
            len: self.len,
        })
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
    /// The new callbacks must uphold the same contract as `new_from_callbacks`
    /// and serve the exact bytes the original ones did. The shared central
    /// directory would otherwise describe byte ranges that do not hold the
    /// entries it names. That cannot be checked in full, but a reader whose
    /// `size` differs from the one this archive was opened from certainly does
    /// not qualify and is rejected with `ZipErrorCode::InvalidArgument`.
    ///
    /// Returns `ZipErrorCode::FailedPrecondition` if the archive is not open,
    /// `ZipErrorCode::Internal` if `size` fails, and
    /// `ZipErrorCode::InvalidArgument` if `size` disagrees with the original.
    /// `release` is called on `context` in every case, including failure.
    pub fn try_clone_with_callbacks(
        &self,
        read_at: extern "C" fn(usize, u64, usize) -> i64,
        size: extern "C" fn(usize) -> i64,
        release: extern "C" fn(usize),
        context: usize,
    ) -> Result<CallbackZipArchive, ZipError> {
        let (Some(original), Some(clone_slot)) = (self.reader.as_ref(), self.clone_slot.as_ref())
        else {
            // Ownership of `context` transferred on call, so release it before
            // failing without invoking `size`.
            release(context);
            return Err(ZipError::failed_precondition("Zip archive is not open"));
        };
        let reader = CallbackReader::try_new(read_at, size, release, context)?;
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
        let next_clone_slot = reader.clone_slot();
        let archive = {
            let _guard = clone_slot.lock();
            clone_slot.stage(reader);
            let archive = (**original).clone();
            // `zip::ZipArchive::clone` clones its reader exactly once, which
            // adopts the staged reader. Anything still staged means it did not,
            // and `archive` would share the original's context after all.
            if clone_slot.take_staged().is_some() {
                return Err(ZipError::internal(
                    "zip::ZipArchive::clone did not take the staged reader",
                ));
            }
            archive
        };
        Ok(CallbackZipArchive {
            reader: Some(Box::new(archive)),
            central_directory: self.central_directory.clone(),
            clone_slot: Some(next_clone_slot),
            len: self.len,
        })
    }

    /// Returns whether the zip archive is none (due to being
    /// default-constructed or moved-from).
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.reader.is_none()
    }

    /// Returns the number of files in the archive.
    ///
    /// The result is only valid if `is_none()` returns false.
    #[must_use]
    pub fn get_length(&self) -> usize {
        self.central_directory.len()
    }

    /// Returns the comment of the zip archive.
    ///
    /// The result is only valid if `is_none()` returns false.
    #[must_use]
    pub fn get_comment(&self) -> VecU8 {
        self.central_directory.comment()
    }

    /// Returns a zip file by its index.
    ///
    /// Returns an error if the archive is not open, `index` is out of bounds,
    /// or the entry's local file header is invalid or unsupported. Opening an
    /// entry reads its local file header through the callbacks, so a failing
    /// `read_at` is also reported here.
    pub fn get_file_by_index(&mut self, index: usize) -> Result<CallbackZipFile<'_>, ZipError> {
        get_file_by_index_impl(&mut self.reader, index, false).map(CallbackZipFile::new)
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
        get_file_by_index_impl(&mut self.reader, index, true).map(CallbackZipFile::new)
    }

    /// Returns what the central directory records about the entry at `index`,
    /// without reading anything.
    ///
    /// Prefer this to `get_file_by_index` when the entry's contents are not
    /// wanted: that one also reads the entry's local file header, so listing
    /// an archive with it costs one read per entry. For this archive the
    /// reads go through the caller's callbacks, which is typically a network,
    /// so listing with `get_file_by_index` costs one round trip per entry.
    /// Usable while an entry is open.
    ///
    /// Returns `FailedPrecondition` if the archive is not open and `OutOfRange`
    /// if `index` is out of bounds.
    pub fn get_entry_metadata(&self, index: usize) -> Result<ZipEntryMetadata, ZipError> {
        self.central_directory.entry_metadata(index)
    }
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
    reader: Option<Box<FsReader>>,
    finished: bool,
    /// Set once a `read_next_file*` call has failed; see `read_next_file_impl`.
    poisoned: bool,
}

impl Debug for FsZipStreamReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsZipStreamReader")
            .field("reader", &if self.reader.is_some() { "Some(FsReader)" } else { "None" })
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
        Ok(Self {
            reader: Some(Box::new(FsReader::uncloneable(file))),
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
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::LazyLock;

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
        /// The offset at which reads fail, if any.
        fail_read_at: Option<u64>,
        /// The offset at which reads return no bytes, if any.
        empty_read_at: Option<u64>,
        /// Whether the next read fails. See `fail_next_read`.
        fail_next_read: bool,
        /// The `offset` of every `read_at` call, in order. Shared, so that a
        /// test can still look at it once the context has been released.
        read_offsets: Arc<Mutex<Vec<u64>>>,
    }

    impl BytesContext {
        /// A context that serves `data`.
        fn new(data: Vec<u8>) -> Self {
            Self {
                data,
                fail_read_at: None,
                empty_read_at: None,
                fail_next_read: false,
                read_offsets: Arc::default(),
            }
        }
    }

    /// The contexts that test callbacks find by token, and the token that the
    /// next one gets.
    struct ContextsByToken<T> {
        next_token: usize,
        contexts: HashMap<usize, T>,
    }

    impl<T> Default for ContextsByToken<T> {
        fn default() -> Self {
            Self { next_token: 1, contexts: HashMap::new() }
        }
    }

    impl<T> ContextsByToken<T> {
        /// Adds `context` and returns its token.
        fn register(&mut self, context: T) -> usize {
            let token = self.next_token;
            self.next_token += 1;
            self.contexts.insert(token, context);
            token
        }
    }

    static BYTES_CONTEXTS: LazyLock<Mutex<ContextsByToken<BytesContext>>> =
        LazyLock::new(|| Mutex::new(ContextsByToken::default()));

    extern "C" fn bytes_read_at(token: usize, offset: u64, len: usize) -> i64 {
        let mut bytes_contexts = BYTES_CONTEXTS.lock().unwrap();
        let context = bytes_contexts.contexts.get_mut(&token).expect("live BytesContext");
        context.read_offsets.lock().unwrap().push(offset);
        if std::mem::take(&mut context.fail_next_read) {
            return -1;
        }
        if context.fail_read_at == Some(offset) {
            return -1;
        }
        if context.empty_read_at == Some(offset) {
            return 0;
        }
        let data = &context.data;
        let offset = offset as usize;
        if offset >= data.len() {
            return 0;
        }
        let len = len.min(data.len() - offset);
        if len > 0 {
            crate::stage_callback_chunk(&data[offset..offset + len]);
        }
        len as i64
    }

    extern "C" fn bytes_size(token: usize) -> i64 {
        let bytes_contexts = BYTES_CONTEXTS.lock().unwrap();
        bytes_contexts.contexts.get(&token).expect("live BytesContext").data.len() as i64
    }

    extern "C" fn bytes_release(token: usize) {
        BYTES_CONTEXTS.lock().unwrap().contexts.remove(&token).expect("BytesContext released once");
    }

    fn bytes_context(data: Vec<u8>) -> usize {
        register_bytes_context(BytesContext::new(data))
    }

    /// Like `bytes_context`, but reads that start at `offset` fail. Also
    /// returns the offsets that `read_at` gets called with.
    fn bytes_context_failing_read_at(data: Vec<u8>, offset: u64) -> (usize, Arc<Mutex<Vec<u64>>>) {
        let context = BytesContext { fail_read_at: Some(offset), ..BytesContext::new(data) };
        let read_offsets = Arc::clone(&context.read_offsets);
        (register_bytes_context(context), read_offsets)
    }

    /// Like `bytes_context`, but reads that start at `offset` return no bytes.
    /// Also returns the offsets that `read_at` gets called with.
    fn bytes_context_with_empty_read_at(
        data: Vec<u8>,
        offset: u64,
    ) -> (usize, Arc<Mutex<Vec<u64>>>) {
        let context = BytesContext { empty_read_at: Some(offset), ..BytesContext::new(data) };
        let read_offsets = Arc::clone(&context.read_offsets);
        (register_bytes_context(context), read_offsets)
    }

    fn register_bytes_context(context: BytesContext) -> usize {
        BYTES_CONTEXTS.lock().unwrap().register(context)
    }

    /// Makes the next read through the context of `token` fail. The reads
    /// after it are served again.
    fn fail_next_read(token: usize) {
        let mut bytes_contexts = BYTES_CONTEXTS.lock().unwrap();
        bytes_contexts.contexts.get_mut(&token).expect("live BytesContext").fail_next_read = true;
    }

    /// Builds a valid archive that is longer than the 1 KiB that `zip` looks
    /// through at a time while it searches backwards for the
    /// end-of-central-directory record, so that the search is not over yet
    /// when a read at the central directory goes wrong. Returns the archive
    /// and the offset of its central directory.
    fn create_zip_longer_than_a_search_window() -> (Vec<u8>, u64) {
        let mut writer = BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap();
        let options = ZipWriterFileOptions::new().compression_method(CompressionMethod::Stored);
        writer.start_file(b"filler.bin", options).unwrap();
        writer.write_data(VecU8::from(vec![0u8; 4096])).unwrap();
        let data = writer.finish().unwrap().as_slice().to_vec();
        let central_directory = data.windows(4).position(|w| w == b"PK\x01\x02").unwrap() as u64;
        (data, central_directory)
    }

    #[gtest]
    fn test_callback_archive_reports_a_read_failure_that_zip_carries_on_from() {
        let (data, central_directory) = create_zip_longer_than_a_search_window();
        // `zip` checks for the central directory signature with a read of its
        // own, and treats a failure of that read like a missing signature: on
        // its own it searches on and then reports this valid archive as
        // invalid.
        let (token, read_offsets) = bytes_context_failing_read_at(data, central_directory);

        let err =
            CallbackZipArchive::new_from_callbacks(bytes_read_at, bytes_size, bytes_release, token)
                .unwrap_err();
        expect_eq!(err.code, ZipErrorCode::Internal);
        expect_true!(err.is_reader_callback_failure());
        // The search does not get to read on: the failed read is the last one.
        expect_eq!(read_offsets.lock().unwrap().last(), Some(&central_directory));
        // The context is released all the same.
        expect_false!(BYTES_CONTEXTS.lock().unwrap().contexts.contains_key(&token));
    }

    #[gtest]
    fn test_callback_archive_reports_an_empty_read_that_zip_carries_on_from() {
        let (data, central_directory) = create_zip_longer_than_a_search_window();
        // `zip` treats a signature that ends before it is complete like a
        // missing one as well, and would search on in the same way.
        let (token, read_offsets) = bytes_context_with_empty_read_at(data, central_directory);

        let err =
            CallbackZipArchive::new_from_callbacks(bytes_read_at, bytes_size, bytes_release, token)
                .unwrap_err();
        // Input that ends early is not a valid archive, and no callback
        // reported a failure.
        expect_eq!(err.code, ZipErrorCode::InvalidArgument);
        expect_false!(err.is_reader_callback_failure());
        // The empty read is the last one.
        expect_eq!(read_offsets.lock().unwrap().last(), Some(&central_directory));
    }

    #[gtest]
    fn test_callback_archive_read_that_failed_can_be_retried() {
        let token = bytes_context(create_test_zip().as_slice().to_vec());
        let mut archive =
            CallbackZipArchive::new_from_callbacks(bytes_read_at, bytes_size, bytes_release, token)
                .unwrap();

        // Opening the entry reads its local file header, and that read fails.
        fail_next_read(token);
        let err = archive.get_file_by_index(0).err();
        expect_true!(err.is_some_and(|err| err.is_reader_callback_failure()));

        // Only while the archive was being opened did a read that went wrong
        // fail the reads after it. Now the same read goes through.
        let mut file = archive.get_file_by_index(0).unwrap();
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
    }

    #[gtest]
    fn test_callback_archive_accepts_a_clone_reader_of_the_same_length() {
        let data = create_test_zip().as_slice().to_vec();
        let archive = CallbackZipArchive::new_from_callbacks(
            bytes_read_at,
            bytes_size,
            bytes_release,
            bytes_context(data.clone()),
        )
        .unwrap();

        // A reader over the same bytes is accepted, serves the entries, and can
        // itself be cloned via both `try_clone` and `try_clone_with_callbacks`.
        let mut same = archive
            .try_clone_with_callbacks(
                bytes_read_at,
                bytes_size,
                bytes_release,
                bytes_context(data.clone()),
            )
            .unwrap();
        let mut same_clone = same.try_clone().unwrap();
        let mut same_replaced = same
            .try_clone_with_callbacks(bytes_read_at, bytes_size, bytes_release, bytes_context(data))
            .unwrap();
        {
            let mut file = same.get_file_by_index(0).unwrap();
            expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
        }
        {
            let mut file = same_clone.get_file_by_index(1).unwrap();
            expect_eq!(file.get_file_data().unwrap().as_slice(), b"Second file content");
        }
        {
            let mut file = same_replaced.get_file_by_index(0).unwrap();
            expect_eq!(file.get_file_data().unwrap().as_slice(), b"Hello, world!");
        }
    }

    #[gtest]
    fn test_callback_archive_rejects_a_clone_reader_of_a_different_length() {
        let data = create_test_zip().as_slice().to_vec();
        // One byte more and the new reader cannot be serving the archive the
        // shared central directory describes, so it is rejected up front rather
        // than yielding garbled entries later.
        let mut longer = data.clone();
        longer.push(b'x');
        let longer_token = bytes_context(longer);
        let archive = CallbackZipArchive::new_from_callbacks(
            bytes_read_at,
            bytes_size,
            bytes_release,
            bytes_context(data),
        )
        .unwrap();

        let err = archive
            .try_clone_with_callbacks(bytes_read_at, bytes_size, bytes_release, longer_token)
            .unwrap_err();

        expect_eq!(err.code, ZipErrorCode::InvalidArgument);
        // The rejected reader is released all the same.
        expect_false!(BYTES_CONTEXTS.lock().unwrap().contexts.contains_key(&longer_token));
    }

    #[gtest]
    fn test_callback_archive_rejects_data_in_front_of_an_archive_without_entries() {
        let empty =
            BufferedZipWriter::new_from_data(VecU8::default(), false).unwrap().finish().unwrap();
        let empty = empty.as_slice().to_vec();
        let archive = CallbackZipArchive::new_from_callbacks(
            bytes_read_at,
            bytes_size,
            bytes_release,
            bytes_context(empty.clone()),
        )
        .unwrap();
        expect_eq!(archive.get_length(), 0);

        // An archive without entries has no central directory header that
        // could be found missing at offset 0, so `zip` accepts it wherever it
        // starts; the offset has to be checked on top.
        let mut prefixed = b"self-extracting stub ".to_vec();
        prefixed.extend_from_slice(&empty);
        let token = bytes_context(prefixed);
        let err =
            CallbackZipArchive::new_from_callbacks(bytes_read_at, bytes_size, bytes_release, token)
                .unwrap_err();
        expect_eq!(err.code, ZipErrorCode::InvalidArgument);
        expect_false!(err.is_reader_callback_failure());
        // The rejected context is released all the same.
        expect_false!(BYTES_CONTEXTS.lock().unwrap().contexts.contains_key(&token));
    }

    #[gtest]
    fn test_callback_archive_try_clone_fails_when_not_open() {
        let archive = CallbackZipArchive::new();
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::FailedPrecondition);
    }

    #[gtest]
    fn test_callback_archive_try_clone_with_callbacks_when_not_open_skips_size_and_releases() {
        /// How many times each callback was called on a context.
        #[derive(Default)]
        struct ProbeCounts {
            size_calls: usize,
            release_calls: usize,
        }

        static PROBE_CONTEXTS: LazyLock<Mutex<ContextsByToken<Arc<Mutex<ProbeCounts>>>>> =
            LazyLock::new(|| Mutex::new(ContextsByToken::default()));

        extern "C" fn probe_size(token: usize) -> i64 {
            let probes = PROBE_CONTEXTS.lock().unwrap();
            let counts = probes.contexts.get(&token).expect("live probe context");
            counts.lock().unwrap().size_calls += 1;
            -1
        }

        extern "C" fn probe_release(token: usize) {
            let counts = PROBE_CONTEXTS
                .lock()
                .unwrap()
                .contexts
                .remove(&token)
                .expect("probe context released once");
            counts.lock().unwrap().release_calls += 1;
        }

        let counts = Arc::new(Mutex::new(ProbeCounts::default()));
        let token = PROBE_CONTEXTS.lock().unwrap().register(Arc::clone(&counts));

        let archive = CallbackZipArchive::new();
        let err = archive
            .try_clone_with_callbacks(bytes_read_at, probe_size, probe_release, token)
            .unwrap_err();

        expect_eq!(err.code, ZipErrorCode::FailedPrecondition);
        let counts = counts.lock().unwrap();
        expect_eq!(counts.size_calls, 0);
        expect_eq!(counts.release_calls, 1);
    }

    #[gtest]
    fn test_fs_archive_try_clone_reopen_failure() {
        let path = create_test_zip_file("try_clone_reopen_failure");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();
        std::fs::remove_file(&path).unwrap();
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::NotFound);
    }

    // Only Unix `FileIdentity` includes device and inode numbers; elsewhere it
    // compares the length alone, which this replacement keeps equal.
    #[cfg(unix)]
    #[gtest]
    fn test_fs_archive_try_clone_detects_replaced_file() {
        let path = create_test_zip_file("try_clone_replaced");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();

        // Replace the archive with a different file of the same contents (and
        // therefore the same length) via rename, as an atomic rewrite would.
        let replacement = format!("{path}.new");
        std::fs::write(&replacement, create_test_zip().as_slice()).unwrap();
        std::fs::rename(&replacement, &path).unwrap();

        let err = archive.try_clone().unwrap_err();
        expect_eq!(err.code, ZipErrorCode::FailedPrecondition);
        expect_that!(
            String::from_utf8_lossy(err.message.as_slice()).into_owned(),
            contains_substring("replaced or modified")
        );
    }

    #[gtest]
    fn test_fs_archive_try_clone_detects_truncated_or_appended_file() {
        let path = create_test_zip_file("try_clone_resized");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();

        // Appending keeps the inode, so only the length gives it away.
        {
            let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(b"trailing garbage").unwrap();
        }
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::FailedPrecondition);

        // Truncating in place likewise keeps the inode.
        {
            let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.set_len(4).unwrap();
        }
        expect_eq!(archive.try_clone().unwrap_err().code, ZipErrorCode::FailedPrecondition);
    }

    #[gtest]
    fn test_fs_archive_try_clone_of_clone() {
        let path = create_test_zip_file("try_clone_of_clone");
        let archive = FsZipArchive::new_from_path(path.as_bytes()).unwrap();
        let clone1 = archive.try_clone().unwrap();
        let mut clone2 = clone1.try_clone().unwrap();

        let mut file = clone2.get_file_by_index(1).unwrap();
        expect_eq!(file.get_file_name().as_slice(), b"second.txt");
        expect_eq!(file.get_file_data().unwrap().as_slice(), b"Second file content");
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
