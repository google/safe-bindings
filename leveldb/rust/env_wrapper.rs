use cc_std::std::virtual_unique_ptr;
use cpp_env::leveldb_rs::{ChildNameList, Env, RandomAccessFile, SequentialFile, WritableFile};
use rusty_leveldb::env::{Env as RustyEnv, FileLock, Logger, RandomAccess};
use rusty_leveldb::Result;

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(crate) struct RustEnvWrapper {
    inner: virtual_unique_ptr<Env>,
    fallback: std::sync::Arc<Box<dyn RustyEnv>>,
}

impl std::fmt::Debug for RustEnvWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RustEnvWrapper").finish()
    }
}

impl RustEnvWrapper {
    pub fn new(inner: virtual_unique_ptr<Env>) -> Self {
        assert!(!inner.get().is_null(), "Env pointer must not be null");
        Self { inner, fallback: rusty_leveldb::Options::default().env }
    }

    fn path_to_bytes(path: &Path) -> &[u8] {
        path.as_os_str().as_bytes()
    }
}

impl RustyEnv for RustEnvWrapper {
    fn open_sequential_file(&self, path: &Path) -> Result<Box<dyn Read>> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::NewSequentialFile(
                self.inner.get().as_ref().unwrap(),
                Self::path_to_bytes(path).into(),
            )
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.open_sequential_file(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(file) => Ok(Box::new(RustSequentialFile { inner: file })),
        }
    }

    fn open_random_access_file(&self, path: &Path) -> Result<Box<dyn RandomAccess>> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::NewRandomAccessFile(
                self.inner.get().as_ref().unwrap(),
                Self::path_to_bytes(path).into(),
            )
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.open_random_access_file(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(file) => Ok(Box::new(RustRandomAccessFile { inner: file })),
        }
    }

    fn open_writable_file(&self, path: &Path) -> Result<Box<dyn Write + Send + Sync>> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::NewWritableFile(
                self.inner.get().as_ref().unwrap(),
                Self::path_to_bytes(path).into(),
            )
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.open_writable_file(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(file) => Ok(Box::new(RustWritableFile { inner: file })),
        }
    }

    fn open_appendable_file(&self, path: &Path) -> Result<Box<dyn Write + Send + Sync>> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::NewAppendableFile(
                self.inner.get().as_ref().unwrap(),
                Self::path_to_bytes(path).into(),
            )
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.open_appendable_file(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(file) => Ok(Box::new(RustWritableFile { inner: file })),
        }
    }

    fn exists(&self, path: &Path) -> Result<bool> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::FileExists(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.exists(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(val) => Ok(val),
        }
    }

    fn size_of(&self, path: &Path) -> Result<usize> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::GetFileSize(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.size_of(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            // `std::size_t` from C++ is mapped to `u64` by Crubit in this context.
            // In Rust, `u64` and `usize` are distinct types, so an explicit
            // cast is required to match the trait signature.
            Ok(val) => val.try_into().map_err(|e| {
                rusty_leveldb::Status::new(
                    rusty_leveldb::StatusCode::IOError,
                    &format!("File size too large for usize: {:?}", e),
                )
            }),
        }
    }

    fn children(&self, path: &Path) -> Result<Vec<PathBuf>> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::ListChildren(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        let list = match res {
            Err(e) if e.code().is_unimplemented() => return self.fallback.children(path),
            Err(e) => {
                return Err(rusty_leveldb::Status::new(
                    rusty_leveldb::StatusCode::IOError,
                    &format!("{:?}", e),
                ))
            }
            Ok(list) => list,
        };

        let mut result = Vec::new();
        let list_ptr = list.get();
        // SAFETY: `list` is a `virtual_unique_ptr` to a `ChildNameList` returned by C++.
        // We assume the C++ side correctly implements `get_count` and `get_item`.
        unsafe {
            for i in 0..ChildNameList::get_count(list_ptr) {
                let s = ChildNameList::get_item(list_ptr, i);
                result.push(PathBuf::from(OsStr::from_bytes(s.as_slice()).to_owned()));
            }
        }
        Ok(result)
    }

    fn delete(&self, path: &Path) -> Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::DeleteFile(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.delete(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(_) => Ok(()),
        }
    }

    fn mkdir(&self, path: &Path) -> Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::CreateDir(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.mkdir(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(_) => Ok(()),
        }
    }

    fn rmdir(&self, path: &Path) -> Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::DeleteDir(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.rmdir(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(_) => Ok(()),
        }
    }

    fn rename(&self, old: &Path, new: &Path) -> Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::RenameFile(
                self.inner.get().as_ref().unwrap(),
                Self::path_to_bytes(old).into(),
                Self::path_to_bytes(new).into(),
            )
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.rename(old, new),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(_) => Ok(()),
        }
    }

    fn lock(&self, path: &Path) -> Result<FileLock> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::LockFile(self.inner.get().as_ref().unwrap(), Self::path_to_bytes(path).into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.lock(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(s) => Ok(FileLock { id: String::from_utf8_lossy(s.as_slice()).into_owned() }),
        }
    }

    fn unlock(&self, l: FileLock) -> Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res =
            unsafe { Env::UnlockFile(self.inner.get().as_ref().unwrap(), l.id.as_bytes().into()) };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.unlock(l),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(_) => Ok(()),
        }
    }

    fn new_logger(&self, path: &Path) -> Result<Logger> {
        let path_str = path.to_str().ok_or_else(|| {
            rusty_leveldb::Status::new(rusty_leveldb::StatusCode::InvalidArgument, "invalid path")
        })?;
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        let res = unsafe {
            Env::NewLogger(self.inner.get().as_ref().unwrap(), path_str.as_bytes().into())
        };
        match res {
            Err(e) if e.code().is_unimplemented() => self.fallback.new_logger(path),
            Err(e) => Err(rusty_leveldb::Status::new(
                rusty_leveldb::StatusCode::IOError,
                &format!("{:?}", e),
            )),
            Ok(logger) => {
                let writer = crate::logger_wrapper::RustLoggerWriter::new(logger);
                Ok(Logger::new(Box::new(writer)))
            }
        }
    }

    fn micros(&self) -> u64 {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        unsafe { Env::NowMicros(self.inner.get().as_ref().unwrap()) }
    }

    fn sleep_for(&self, micros: u32) {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Env` instance.
        // Crubit guarantees that this pointer remains valid and non-null as long as
        // the `RustEnvWrapper` exists.
        unsafe { Env::SleepForMicroseconds(self.inner.get().as_ref().unwrap(), micros) }
    }
}

struct RustSequentialFile {
    inner: virtual_unique_ptr<SequentialFile>,
}

impl Read for RustSequentialFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `SequentialFile`.
        // This object was successfully created via `Env::NewSequentialFile`, which
        // checks the `absl::Status` to ensure a valid, non-null pointer is returned.
        // The pointer remains valid as long as this `RustSequentialFile` exists.
        let res = unsafe { SequentialFile::Read(self.inner.get().as_mut().unwrap(), buf.into()) };
        res.map(|s| s as usize).map_err(|e| std::io::Error::other(format!("C++ Error: {:?}", e)))
    }
}

struct RustRandomAccessFile {
    inner: virtual_unique_ptr<RandomAccessFile>,
}

impl RandomAccess for RustRandomAccessFile {
    fn read_at(&self, off: usize, dst: &mut [u8]) -> Result<usize> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `RandomAccessFile`.
        // It was created and verified via `Env::NewRandomAccessFile`. Since we own
        // the unique pointer, the underlying C++ object is guaranteed to be valid.
        let res = unsafe {
            RandomAccessFile::Read(self.inner.get().as_ref().unwrap(), off as u64, dst.into())
        };
        res.map(|s| s as usize).map_err(|e| {
            rusty_leveldb::Status::new(rusty_leveldb::StatusCode::IOError, &format!("{:?}", e))
        })
    }
}

struct RustWritableFile {
    inner: virtual_unique_ptr<WritableFile>,
}

impl Write for RustWritableFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `WritableFile`.
        // This was created via `Env::NewWritableFile` or `Env::NewAppendableFile`.
        // Ownership and validity are maintained for the lifetime of this object.
        let res = unsafe { WritableFile::Append(self.inner.get().as_mut().unwrap(), buf.into()) };
        res.map(|_| buf.len()).map_err(|e| std::io::Error::other(format!("C++ Error: {:?}", e)))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `WritableFile`.
        // Validity is guaranteed as described in the `write` implementation.
        let res = unsafe { WritableFile::Flush(self.inner.get().as_mut().unwrap()) };
        res.map_err(|e| std::io::Error::other(format!("C++ Error: {:?}", e)))
    }
}
