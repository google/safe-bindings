use cc_std::std::virtual_unique_ptr;
use cpp_cmp::leveldb_rs::Comparator;
use cpp_filter::leveldb_rs::FilterPolicy;
use cpp_logger::leveldb_rs::Logger;

// Our FFI-safe representation of LevelDB options.
#[derive(Default, Clone)]
pub struct Options {
    pub(crate) inner: rusty_leveldb::Options,
}

impl Options {
    pub fn create() -> Self {
        Self::default()
    }

    pub fn set_create_if_missing(&mut self, val: bool) {
        self.inner.create_if_missing = val;
    }

    pub fn set_error_if_exists(&mut self, val: bool) {
        self.inner.error_if_exists = val;
    }

    pub fn set_paranoid_checks(&mut self, val: bool) {
        self.inner.paranoid_checks = val;
    }

    pub fn set_write_buffer_size(&mut self, val: usize) {
        self.inner.write_buffer_size = val;
    }

    pub fn set_max_open_files(&mut self, val: usize) {
        self.inner.max_open_files = val;
    }

    pub fn set_max_file_size(&mut self, val: usize) {
        self.inner.max_file_size = val;
    }

    pub fn set_block_cache_capacity_bytes(&mut self, val: usize) {
        self.inner.block_cache_capacity_bytes = val;
    }

    pub fn set_block_size(&mut self, val: usize) {
        self.inner.block_size = val;
    }

    pub fn set_block_restart_interval(&mut self, val: usize) {
        self.inner.block_restart_interval = val;
    }

    pub fn set_compressor(&mut self, val: u8) {
        self.inner.compressor = val;
    }

    pub fn set_reuse_logs(&mut self, val: bool) {
        self.inner.reuse_logs = val;
    }

    pub fn set_reuse_manifest(&mut self, val: bool) {
        self.inner.reuse_manifest = val;
    }

    /// Sets the comparator for the database.
    ///
    /// # Safety
    ///
    /// * `cmp` must be a valid, heap-allocated C++ `Comparator`. This function
    ///   takes ownership and will delete the object when dropped.
    /// * The implementation of `cmp` **must be thread-safe** (safe to call `Compare`
    ///   simultaneously from multiple threads), as LevelDB will use it
    ///   from background compaction threads.
    // NOTE: b/505082018 - Change the parameter type to cc_std::std::virtual_unique_ptr once mapping from virtual_unique_ptr to cc_std::unique_ptr is implemented.
    pub unsafe fn set_cmp(&mut self, cmp: *mut Comparator) {
        assert!(!cmp.is_null(), "Comparator pointer must not be null");
        let wrapped_cmp = unsafe { virtual_unique_ptr::new(cmp) };
        self.inner.cmp =
            std::sync::Arc::new(Box::new(crate::cmp_wrapper::RustCmpWrapper::new(wrapped_cmp)));
    }

    /// Sets the environment for the database.
    ///
    /// # Safety
    ///
    /// `env` must be a valid, heap-allocated C++ `Env`. This function
    /// takes ownership and will delete the object when dropped.
    // NOTE: b/505082018 - Change the parameter type to cc_std::std::virtual_unique_ptr once mapping from virtual_unique_ptr to cc_std::unique_ptr is implemented.
    pub unsafe fn set_env(&mut self, env: *mut cpp_env::leveldb_rs::Env) {
        assert!(!env.is_null(), "Env pointer must not be null");
        let wrapped_env = unsafe { virtual_unique_ptr::new(env) };
        self.inner.env =
            std::sync::Arc::new(Box::new(crate::env_wrapper::RustEnvWrapper::new(wrapped_env)));
    }

    /// Sets the filter policy for the database.
    ///
    /// # Safety
    ///
    /// `filter` must be a valid, heap-allocated C++ `FilterPolicy`. This function
    /// takes ownership and will delete the object when dropped.
    // NOTE: b/505082018 - Change the parameter type to cc_std::std::virtual_unique_ptr once mapping from virtual_unique_ptr to cc_std::unique_ptr is implemented.
    pub unsafe fn set_filter_policy(&mut self, filter: *mut FilterPolicy) {
        assert!(!filter.is_null(), "FilterPolicy pointer must not be null");
        let wrapped_filter = unsafe { virtual_unique_ptr::new(filter) };
        self.inner.filter_policy = std::sync::Arc::new(Box::new(
            crate::filter_wrapper::RustFilterWrapper::new(wrapped_filter),
        ));
    }

    /// Sets the logger for the database.
    ///
    /// # Safety
    ///
    /// `logger` must be a valid, heap-allocated C++ `Logger`. This function
    /// takes ownership and will delete the object when dropped.
    // NOTE: b/505082018 - Change the parameter type to cc_std::std::virtual_unique_ptr once mapping from virtual_unique_ptr to cc_std::unique_ptr is implemented.
    pub unsafe fn set_info_log(&mut self, logger: *mut Logger) {
        assert!(!logger.is_null(), "Logger pointer must not be null");
        let wrapped_logger = unsafe { virtual_unique_ptr::new(logger) };
        let writer = crate::logger_wrapper::RustLoggerWriter::new(wrapped_logger);
        let logger = rusty_leveldb::infolog::Logger(Box::new(writer));
        self.inner.log = Some(rusty_leveldb::share(logger));
    }

    pub(crate) fn into_rusty_options(self) -> rusty_leveldb::Options {
        self.inner
    }
}
