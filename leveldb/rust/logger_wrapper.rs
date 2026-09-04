use cc_std::std::virtual_unique_ptr;
use cpp_logger::leveldb_rs::Logger;
use std::io::Write;

pub(crate) struct RustLoggerWriter {
    inner: virtual_unique_ptr<Logger>,
}

impl RustLoggerWriter {
    pub fn new(inner: virtual_unique_ptr<Logger>) -> Self {
        assert!(!virtual_unique_ptr::is_null(&inner), "Logger pointer must not be null");
        Self { inner }
    }
}

impl Write for RustLoggerWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `Logger`
        // instance. Crubit guarantees that this pointer remains valid and non-null
        // as long as the `RustLoggerWriter` exists.
        unsafe {
            Logger::Log(
                virtual_unique_ptr::as_mut_ptr(&mut self.inner).as_mut().unwrap(),
                buf.into(),
            );
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
