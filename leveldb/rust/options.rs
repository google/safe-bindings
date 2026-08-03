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
    /// The implementation of `cmp` **must be thread-safe** (safe to call `Compare`
    /// simultaneously from multiple threads), as LevelDB will use it
    /// from background compaction threads.
    pub fn set_cmp(&mut self, cmp: virtual_unique_ptr<Comparator>) {
        self.inner.cmp =
            std::sync::Arc::new(Box::new(crate::cmp_wrapper::RustCmpWrapper::new(cmp)));
    }

    /// Sets the environment for the database.
    pub fn set_env(&mut self, env: virtual_unique_ptr<cpp_env::leveldb_rs::Env>) {
        self.inner.env =
            std::sync::Arc::new(Box::new(crate::env_wrapper::RustEnvWrapper::new(env)));
    }

    /// Sets the filter policy for the database.
    pub fn set_filter_policy(&mut self, filter: virtual_unique_ptr<FilterPolicy>) {
        self.inner.filter_policy =
            std::sync::Arc::new(Box::new(crate::filter_wrapper::RustFilterWrapper::new(filter)));
    }

    /// Sets the logger for the database.
    pub fn set_info_log(&mut self, logger: virtual_unique_ptr<Logger>) {
        let writer = crate::logger_wrapper::RustLoggerWriter::new(logger);
        let logger = rusty_leveldb::infolog::Logger(Box::new(writer));
        self.inner.log = Some(rusty_leveldb::share(logger));
    }

    pub(crate) fn into_rusty_options(self) -> rusty_leveldb::Options {
        self.inner
    }
}
