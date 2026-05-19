// Crubit does not support generic type parameters, so we need to implement
// BufferedZipWriter and FsZipWriter manually.

use cc_std::std::string_view;
use rust_vec_u8::VecU8;
use std::fmt::{Debug, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{copy, Cursor, Read, Seek, Write};
use zip::{
    write::FileOptions, CompressionMethod as ZipCrateCompressionMethod,
    ZipWriter as WrappedZipWriter,
};

use crate::{
    BufferedZipFile, FsZipFile, ResultBufferedZipWriter, ResultFsZipWriter, ResultUnit, ResultVecU8,
};

// Expose some of the options for writing files to the zip archive.

/// Mirror some of zip::CompressionMethod for Crubit, based on build features.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMethod {
    #[default]
    Deflated,
    Stored,
    Bzip2,
    Zstd,
    Lzma,
    Xz,
    Unsupported,
}

impl TryFrom<i32> for CompressionMethod {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CompressionMethod::Stored),
            1 => Ok(CompressionMethod::Deflated),
            2 => Ok(CompressionMethod::Bzip2),
            3 => Ok(CompressionMethod::Zstd),
            4 => Ok(CompressionMethod::Lzma),
            5 => Ok(CompressionMethod::Xz),
            _ => Err(format!("Invalid discriminant for CompressionMethod value: {}", value)),
        }
    }
}

impl CompressionMethod {
    pub fn from_i32(value: i32) -> CompressionMethod {
        match CompressionMethod::try_from(value) {
            Ok(method) => method,
            Err(_) => CompressionMethod::Unsupported,
        }
    }
}

impl From<CompressionMethod> for ZipCrateCompressionMethod {
    fn from(val: CompressionMethod) -> Self {
        match val {
            CompressionMethod::Deflated => ZipCrateCompressionMethod::Deflated,
            CompressionMethod::Stored => ZipCrateCompressionMethod::Stored,
            CompressionMethod::Bzip2 => ZipCrateCompressionMethod::Bzip2,
            CompressionMethod::Zstd => ZipCrateCompressionMethod::Zstd,
            CompressionMethod::Lzma => ZipCrateCompressionMethod::Lzma,
            CompressionMethod::Xz => ZipCrateCompressionMethod::Xz,
            CompressionMethod::Unsupported => {
                panic!("cannot convert CompressionMethod::Unsupported to ZipCrateCompressionMethod")
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct ZipWriterFileOptions {
    compression_method: Option<CompressionMethod>,
    compression_level: Option<i64>,
    permissions: Option<u32>,
    large_file: Option<bool>,
}

impl TryFrom<&ZipWriterFileOptions> for FileOptions<'static, ()> {
    type Error = anyhow::Error;

    fn try_from(val: &ZipWriterFileOptions) -> anyhow::Result<Self> {
        let mut options = FileOptions::<()>::default();
        if let Some(method) = val.compression_method {
            options = options.compression_method(method.into());
        }
        // third_party/rust/zip/v6/src/write.rs
        //
        // `None` value specifies default compression level.
        //
        // Range of values depends on compression method:
        // * `Deflated (None is treated as Deflated)`: 10 - 264 for Zopfli, 0 - 9 for other
        //   encoders. Default is 24 if Zopfli is the only encoder, or 6 otherwise.
        // * `Bzip2`: 0 - 9. Default is 6
        // * `Zstd`: -7 - 22, with zero being mapped to default level. Default is 3
        // * others: only `None` is allowed
        if let Some(level) = val.compression_level {
            let method = val.compression_method.unwrap_or_default();
            let in_range = match method {
                CompressionMethod::Deflated => {
                    #[cfg(feature = "deflate-zopfli")]
                    let range = 0..=264;
                    #[cfg(not(feature = "deflate-zopfli"))]
                    let range = 0..=9;
                    range.contains(&level)
                }
                CompressionMethod::Bzip2 => (0..=9).contains(&level),
                CompressionMethod::Zstd => (-7..=22).contains(&level),
                _ => false,
            };
            if !in_range {
                return Err(anyhow::anyhow!(
                    "Compression level {} is out of range for method {:?}",
                    level,
                    method
                ));
            }
            options = options.compression_level(Some(level));
        }
        if let Some(permissions) = val.permissions {
            options = options.unix_permissions(permissions);
        }
        if let Some(large_file) = val.large_file {
            options = options.large_file(large_file);
        }
        Ok(options)
    }
}

impl ZipWriterFileOptions {
    /// Creates a new set of options for writing files to the zip archive.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the compression method for the new file.
    pub fn compression_method(mut self, method: CompressionMethod) -> Self {
        self.compression_method = Some(method);
        self
    }

    /// Sets the compression level for the new file.
    ///
    /// This option is only effective if `compression_method` is also set to
    /// a method that supports compression levels (e.g., `Deflated`, `Bzip2`, `Zstd`).
    pub fn compression_level(mut self, level: i64) -> Self {
        self.compression_level = Some(level);
        self
    }

    /// Sets the Unix permissions for the new file.
    pub fn unix_permissions(mut self, permissions: u32) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Sets whether to use large file support for the new file.
    pub fn large_file(mut self, large_file: bool) -> Self {
        self.large_file = Some(large_file);
        self
    }
}

#[derive(Default)]
/// A zip writer that writes to an in-memory buffer.
pub struct BufferedZipWriter {
    writer: Option<WrappedZipWriter<Cursor<Vec<u8>>>>,
}

impl Debug for BufferedZipWriter {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("BufferedZipWriter")
            .field("writer", &if self.writer.is_some() { "Some(WrappedZipWriter)" } else { "None" })
            .finish()
    }
}

impl BufferedZipWriter {
    /// Creates a new `BufferedZipWriter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BufferedZipWriter` from a byte vector.
    pub fn new_from_data(data: VecU8, append: bool) -> ResultBufferedZipWriter {
        let mut writer = Self::default();
        if let Err(e) = writer.open(data, append) {
            return ResultBufferedZipWriter::from(anyhow::anyhow!(
                "Failed to open zip buffer: {}",
                e
            ));
        }
        ResultBufferedZipWriter::from_ok(writer)
    }

    /// Opens a zip archive from a byte vector.
    fn open(&mut self, data: VecU8, append: bool) -> anyhow::Result<()> {
        let cursor = Cursor::new(data.into_vec());
        if append {
            match WrappedZipWriter::new_append(cursor) {
                Ok(writer) => {
                    self.writer = Some(writer);
                    Ok(())
                }
                Err(e) => Err(e.into()),
            }
        } else {
            self.writer = Some(WrappedZipWriter::new(cursor));
            Ok(())
        }
    }

    /// Returns whether the zip writer is none (due to being
    /// default-constructed or moved-from).
    pub fn is_none(&self) -> bool {
        self.writer.is_none()
    }

    /// Finishes writing the zip archive and returns the buffered data.
    pub fn finish(&mut self) -> ResultVecU8 {
        if let Some(writer) = self.writer.take() {
            match writer.finish() {
                Ok(cursor) => ResultVecU8::from_ok(cursor.into_inner().into()),
                Err(e) => Err(anyhow::Error::from(e)).into(),
            }
        } else {
            ResultVecU8::from(anyhow::anyhow!("Zip writer is not open"))
        }
    }

    /// Creates a new file in the zip archive and start writing to it.
    ///
    /// # Safety
    ///
    /// The memory viewed by `name` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `name` must not modify it via
    /// other aliases.
    pub unsafe fn start_file(
        &mut self,
        name: string_view,
        options: ZipWriterFileOptions,
    ) -> ResultUnit {
        start_file_impl(&mut self.writer, name, options)
    }

    /// Adds a directory to the zip archive.
    ///
    /// # Safety
    ///
    /// The memory viewed by `name` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `name` must not modify it via
    /// other aliases.
    pub unsafe fn add_directory(
        &mut self,
        name: string_view,
        options: ZipWriterFileOptions,
    ) -> ResultUnit {
        add_directory_impl(&mut self.writer, name, options)
    }

    /// Writes data to the current file in the zip archive.
    pub fn write_data(&mut self, data: VecU8) -> ResultUnit {
        write_data_impl(&mut self.writer, data)
    }

    // Crubit does not support generic parameters, so we implement `io::copy`
    // helpers for a set of types instead of a generic method.

    /// Writes file content from a `BufferedZipFile` to the current file in the zip archive.
    pub fn write_buffered_zip_file_content(&mut self, file: &mut BufferedZipFile) -> ResultUnit {
        do_copy_impl(&mut self.writer, file)
    }

    /// Writes file content from a `FsZipFile` to the current file in the zip archive.
    pub fn write_fs_zip_file_content(&mut self, file: &mut FsZipFile) -> ResultUnit {
        do_copy_impl(&mut self.writer, file)
    }

    /// Writes file content from a path to the current file in the zip archive.
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    pub unsafe fn write_file_content(&mut self, path: string_view) -> ResultUnit {
        write_file_content_impl(&mut self.writer, path)
    }
}

#[derive(Default)]
/// A zip writer that writes to a file on the filesystem.
pub struct FsZipWriter {
    writer: Option<WrappedZipWriter<File>>,
}

impl Debug for FsZipWriter {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("FsZipWriter")
            .field("writer", &if self.writer.is_some() { "Some(WrappedZipWriter)" } else { "None" })
            .finish()
    }
}

impl FsZipWriter {
    /// Creates a new `FsZipWriter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `FsZipWriter` from a path.
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    pub unsafe fn new_from_path(path: string_view, append: bool) -> ResultFsZipWriter {
        let mut writer = Self::default();
        if let Err(e) = writer.open(path, append) {
            return ResultFsZipWriter::from(anyhow::anyhow!("Failed to open zip archive: {}", e));
        }
        ResultFsZipWriter::from_ok(writer)
    }

    /// Opens a zip archive from a path.
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    unsafe fn open(&mut self, path: string_view, append: bool) -> anyhow::Result<()> {
        // SAFETY: The caller of `open` must ensure that `path` is valid for
        // the duration of the call.
        let path_str = String::from_utf8_lossy(unsafe { path.as_bytes() });
        if append {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path_str.as_ref())
            {
                Ok(file) => match WrappedZipWriter::new_append(file) {
                    Ok(writer) => {
                        self.writer = Some(writer);
                        Ok(())
                    }
                    Err(e) => Err(anyhow::Error::from(e)),
                },
                Err(e) => Err(anyhow::Error::from(e)),
            }
        } else {
            match OpenOptions::new().write(true).create(true).truncate(true).open(path_str.as_ref())
            {
                Ok(file) => {
                    self.writer = Some(WrappedZipWriter::new(file));
                    Ok(())
                }
                Err(e) => Err(anyhow::Error::from(e)),
            }
        }
    }

    /// Returns whether the zip writer is none (due to being
    /// default-constructed or moved-from).
    pub fn is_none(&self) -> bool {
        self.writer.is_none()
    }

    /// Finishes writing the zip archive to file.
    pub fn finish(&mut self) -> ResultUnit {
        if let Some(writer) = self.writer.take() {
            match writer.finish() {
                Ok(_) => ResultUnit::from_ok(()),
                Err(e) => Err(anyhow::Error::from(e)).into(),
            }
        } else {
            ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
        }
    }

    /// Creates a new file in the zip archive and start writing to it.
    ///
    /// # Safety
    ///
    /// The memory viewed by `name` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `name` must not modify it via
    /// other aliases.
    pub unsafe fn start_file(
        &mut self,
        name: string_view,
        options: ZipWriterFileOptions,
    ) -> ResultUnit {
        start_file_impl(&mut self.writer, name, options)
    }

    /// Adds a directory to the zip archive.
    ///
    /// # Safety
    ///
    /// The memory viewed by `name` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `name` must not modify it via
    /// other aliases.
    pub unsafe fn add_directory(
        &mut self,
        name: string_view,
        options: ZipWriterFileOptions,
    ) -> ResultUnit {
        add_directory_impl(&mut self.writer, name, options)
    }

    /// Writes data to the current file in the zip archive.
    pub fn write_data(&mut self, data: VecU8) -> ResultUnit {
        write_data_impl(&mut self.writer, data)
    }

    // Crubit does not support generic parameters, so we implement `io::copy`
    // helpers for a set of types instead of a generic method.

    /// Writes file content from a `BufferedZipFile` to the current file in the zip archive.
    pub fn write_buffered_zip_file_content(&mut self, file: &mut BufferedZipFile) -> ResultUnit {
        do_copy_impl(&mut self.writer, file)
    }

    /// Writes file content from a `FsZipFile` to the current file in the zip archive.
    pub fn write_fs_zip_file_content(&mut self, file: &mut FsZipFile) -> ResultUnit {
        do_copy_impl(&mut self.writer, file)
    }

    /// Writes file content from a path to the current file in the zip archive.
    ///
    /// # Safety
    ///
    /// The memory viewed by `path` must not be mutated by any C++ code during
    /// the execution of this function. While C++ `std::string_view` provides
    /// read-only access, the C++ code owning `path` must not modify it via
    /// other aliases.
    pub unsafe fn write_file_content(&mut self, path: string_view) -> ResultUnit {
        write_file_content_impl(&mut self.writer, path)
    }
}

/// # Safety
///
/// The memory viewed by `name` must not be mutated by any C++ code during
/// the execution of this function. While C++ `std::string_view` provides
/// read-only access, the C++ code owning `name` must not modify it via
/// other aliases.
unsafe fn start_file_impl<W: Write + Seek>(
    writer: &mut Option<WrappedZipWriter<W>>,
    name: string_view,
    options: ZipWriterFileOptions,
) -> ResultUnit {
    if let Some(writer) = writer.as_mut() {
        // SAFETY: The caller of `start_file` must ensure that `name` is valid for
        // the duration of the call.
        let name_str = String::from_utf8_lossy(unsafe { name.as_bytes() });
        match FileOptions::try_from(&options) {
            Ok(file_options) => match writer.start_file(name_str.as_ref(), file_options) {
                Ok(_) => ResultUnit::from_ok(()),
                Err(e) => Err(anyhow::Error::from(e)).into(),
            },
            Err(e) => Err(anyhow::Error::from(e)).into(),
        }
    } else {
        ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
    }
}

/// # Safety
///
/// The memory viewed by `name` must not be mutated by any C++ code during
/// the execution of this function. While C++ `std::string_view` provides
/// read-only access, the C++ code owning `name` must not modify it via
/// other aliases.
unsafe fn add_directory_impl<W: Write + Seek>(
    writer: &mut Option<WrappedZipWriter<W>>,
    name: string_view,
    options: ZipWriterFileOptions,
) -> ResultUnit {
    if let Some(writer) = writer.as_mut() {
        // SAFETY: The caller of `add_directory` must ensure that `name` is valid for
        // the duration of the call.
        let name_str = String::from_utf8_lossy(unsafe { name.as_bytes() });
        match FileOptions::try_from(&options) {
            Ok(file_options) => match writer.add_directory(name_str.as_ref(), file_options) {
                Ok(_) => ResultUnit::from_ok(()),
                Err(e) => Err(anyhow::Error::from(e)).into(),
            },
            Err(e) => Err(anyhow::Error::from(e)).into(),
        }
    } else {
        ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
    }
}

fn write_data_impl<W: Write + Seek>(
    writer: &mut Option<WrappedZipWriter<W>>,
    data: VecU8,
) -> ResultUnit {
    if let Some(writer) = writer.as_mut() {
        match writer.write_all(data.as_slice()) {
            Ok(_) => ResultUnit::from_ok(()),
            Err(e) => Err(anyhow::Error::from(e)).into(),
        }
    } else {
        ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
    }
}

fn do_copy_impl<W: Write + Seek, R: Read>(
    writer: &mut Option<WrappedZipWriter<W>>,
    reader: &mut R,
) -> ResultUnit {
    if let Some(writer) = writer.as_mut() {
        match copy(reader, writer) {
            Ok(_) => ResultUnit::from_ok(()),
            Err(e) => Err(anyhow::Error::from(e)).into(),
        }
    } else {
        ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
    }
}

/// # Safety
///
/// The memory viewed by `path` must not be mutated by any C++ code during
/// the execution of this function. While C++ `std::string_view` provides
/// read-only access, the C++ code owning `path` must not modify it via
/// other aliases.
unsafe fn write_file_content_impl<W: Write + Seek>(
    writer: &mut Option<WrappedZipWriter<W>>,
    path: string_view,
) -> ResultUnit {
    if let Some(writer) = writer.as_mut() {
        // SAFETY: The caller of `write_file_content` must ensure that `path` is valid for
        // the duration of the call.
        let path_str = String::from_utf8_lossy(unsafe { path.as_bytes() });
        match File::open(path_str.as_ref()) {
            Ok(mut file) => match copy(&mut file, writer) {
                Ok(_) => ResultUnit::from_ok(()),
                Err(e) => Err(anyhow::Error::from(e)).into(),
            },
            Err(e) => Err(anyhow::Error::from(e)).into(),
        }
    } else {
        ResultUnit::from(anyhow::anyhow!("Zip writer is not open"))
    }
}
