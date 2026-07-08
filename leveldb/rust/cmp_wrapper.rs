use cc_std::std::virtual_unique_ptr;
use cpp_cmp::leveldb_rs::Comparator;
use rusty_leveldb::Cmp;
use std::cmp::Ordering;

pub(crate) struct RustCmpWrapper {
    pub inner: virtual_unique_ptr<Comparator>,
}

impl std::fmt::Debug for RustCmpWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RustCmpWrapper").finish()
    }
}

impl RustCmpWrapper {
    pub fn new(inner: virtual_unique_ptr<Comparator>) -> Self {
        assert!(!inner.get().is_null(), "Comparator pointer must not be null");
        Self { inner }
    }
}

impl Cmp for RustCmpWrapper {
    fn cmp(&self, a: &[u8], b: &[u8]) -> Ordering {
        // SAFETY: `self.inner` is only ever populated by `RustCmpWrapper::new`, which
        // guarantees that it points to a valid C++ `Comparator` instance that remains valid
        // during this call.
        let res =
            unsafe { Comparator::Compare(self.inner.get().as_ref().unwrap(), a.into(), b.into()) };
        match res {
            r if r < 0 => Ordering::Less,
            r if r > 0 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }

    fn id(&self) -> &'static str {
        "leveldb.CppComparator"
    }

    fn find_shortest_sep(&self, a: &[u8], b: &[u8]) -> Vec<u8> {
        // SAFETY: `self.inner` is only ever populated by `RustCmpWrapper::new`, which
        // guarantees that it points to a valid C++ `Comparator` instance that remains valid
        // during this call.
        let s = unsafe {
            Comparator::FindShortestSeparator(
                self.inner.get().as_ref().unwrap(),
                a.into(),
                b.into(),
            )
        };
        s.as_ref().to_vec()
    }

    fn find_short_succ(&self, a: &[u8]) -> Vec<u8> {
        // SAFETY: `self.inner` is only ever populated by `RustCmpWrapper::new`, which
        // guarantees that it points to a valid C++ `Comparator` instance that remains valid
        // during this call.
        let s =
            unsafe { Comparator::FindShortSuccessor(self.inner.get().as_ref().unwrap(), a.into()) };
        s.as_ref().to_vec()
    }
}
