use cc_std::std::virtual_unique_ptr;
use cpp_filter::leveldb_rs::FilterPolicy;
use rusty_leveldb::FilterPolicy as RustyFilterPolicy;

pub(crate) struct RustFilterWrapper {
    pub inner: virtual_unique_ptr<FilterPolicy>,
}

impl std::fmt::Debug for RustFilterWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RustFilterWrapper").finish()
    }
}

impl RustFilterWrapper {
    pub fn new(inner: virtual_unique_ptr<FilterPolicy>) -> Self {
        assert!(!inner.get().is_null(), "FilterPolicy pointer must not be null");
        Self { inner }
    }
}

impl RustyFilterPolicy for RustFilterWrapper {
    fn name(&self) -> &'static str {
        // SAFETY: The C++ interface documents that `Name()` must return a
        // string_view to a static string. We rely on this invariant to provide
        // the `'static` lifetime required by the Rust `FilterPolicy` trait.
        // `self.inner` is a `virtual_unique_ptr` that owns the C++ `FilterPolicy`
        // instance; Crubit guarantees that this pointer remains valid and non-null
        // as long as the `RustFilterWrapper` exists.
        unsafe {
            let name = FilterPolicy::Name(self.inner.get().as_ref().unwrap());
            let bytes: &'static [u8] = &*<*const [u8]>::from(name);
            std::str::from_utf8(bytes).expect("FilterPolicy name must be valid UTF-8")
        }
    }

    fn create_filter(&self, keys: &[u8], key_offsets: &[usize]) -> Vec<u8> {
        let offsets_u64: Vec<u64> = key_offsets.iter().map(|&o| o as u64).collect();
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `FilterPolicy`
        // instance. Crubit guarantees that this pointer remains valid and non-null
        // as long as the `RustFilterWrapper` exists.
        let s = unsafe {
            FilterPolicy::CreateFilter(
                self.inner.get().as_ref().unwrap(),
                keys.into(),
                offsets_u64.as_slice().into(),
            )
        };
        s.as_slice().to_vec()
    }

    fn key_may_match(&self, key: &[u8], filter: &[u8]) -> bool {
        // SAFETY: `self.inner` is a `virtual_unique_ptr` that owns the C++ `FilterPolicy`
        // instance. Crubit guarantees that this pointer remains valid and non-null
        // as long as the `RustFilterWrapper` exists.
        unsafe {
            FilterPolicy::KeyMayMatch(self.inner.get().as_ref().unwrap(), key.into(), filter.into())
        }
    }
}
