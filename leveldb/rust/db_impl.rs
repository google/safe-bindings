use crate::*;
use cc_std::std::string;
use rusty_leveldb::DB as LevelDB_DB;
use std::option::Option;

#[derive(Default)]
pub struct DB {
    db: std::sync::Mutex<Option<LevelDB_DB>>,
}

#[derive(Debug, Default, Clone)]
pub struct DBValue {
    pub(crate) inner: bytes::Bytes,
}

impl DBValue {
    pub fn to_string(&self) -> string {
        string::from(self.inner.as_ref())
    }
}

impl std::fmt::Debug for DB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_open = self.db.lock().ok().and_then(|g| g.as_ref().map(|_| "...")).is_some();
        f.debug_struct("DB").field("db", &if is_open { Some("...") } else { None }).finish()
    }
}

mod db_guard {
    use rusty_leveldb::DB as LevelDB_DB;

    /// A wrapper around MutexGuard that provides a projection to the underlying `LevelDB_DB`.
    ///
    /// This avoids unsafe-looking `unwrap()` calls at all database call sites, while upholding
    /// thread safety. Since `DB::get_db()` validates that the DB is open (producing a `Some` variant),
    /// obtaining a `DBGuard` guarantees that the `Option` is `Some` for the duration of the lock.
    pub struct DBGuard<'a> {
        guard: std::sync::MutexGuard<'a, Option<LevelDB_DB>>,
    }

    impl<'a> DBGuard<'a> {
        /// Creates a new `DBGuard`.
        ///
        /// # Panics
        ///
        /// Panics if the underlying `Option<LevelDB_DB>` is `None`.
        pub fn new(guard: std::sync::MutexGuard<'a, Option<LevelDB_DB>>) -> Self {
            assert!(guard.is_some(), "the db must be Some");
            Self { guard }
        }
    }

    impl<'a> std::ops::Deref for DBGuard<'a> {
        type Target = LevelDB_DB;
        fn deref(&self) -> &Self::Target {
            self.guard.as_ref().unwrap()
        }
    }

    impl<'a> std::ops::DerefMut for DBGuard<'a> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.guard.as_mut().unwrap()
        }
    }
}
pub use db_guard::DBGuard;

impl DB {
    fn get_db(&self) -> Result<DBGuard<'_>, LevelDBError> {
        let guard =
            self.db.lock().map_err(|e| LevelDBError::from(format!("poisoned lock: {}", e)))?;
        if guard.is_none() {
            return Err(LevelDBError::from("DB is not open"));
        }
        Ok(DBGuard::new(guard))
    }

    pub fn open(path: &str, options: crate::Options) -> Result<DB, LevelDBError> {
        let rusty_opts = options.into_rusty_options();
        LevelDB_DB::open(path, rusty_opts)
            .map(|db| DB { db: std::sync::Mutex::new(Some(db)) })
            .map_err(|e| e.into())
    }

    pub fn close(&self) -> Result<u8, LevelDBError> {
        let mut guard = match self.db.lock() {
            Ok(g) => g,
            Err(e) => return Err(LevelDBError::from(format!("poisoned lock: {}", e))),
        };
        guard
            .take()
            .ok_or_else(|| LevelDBError::from("DB is not open"))
            .and_then(|mut db| db.close().map_err(|e| e.into()))
            .map(|_| 0)
            .map_err(|e| e.into())
    }

    pub fn put(&self, k: &[u8], v: &[u8]) -> Result<u8, LevelDBError> {
        // put performs a deep copy, no references are leaked.
        let mut guard = self.get_db()?;
        guard.put(k, v).map(|_| 0).map_err(|e| e.into())
    }

    pub fn get(&self, k: &[u8]) -> Result<Option<DBValue>, LevelDBError> {
        let mut guard = self.get_db()?;
        Ok(guard.get(k).map(|v| DBValue { inner: v }))
    }

    pub fn delete(&self, k: &[u8]) -> Result<u8, LevelDBError> {
        let mut guard = self.get_db()?;
        guard.delete(k).map(|_| 0).map_err(|e| e.into())
    }

    pub fn flush(&self) -> Result<u8, LevelDBError> {
        let mut guard = self.get_db()?;
        guard.flush().map(|_| 0).map_err(|e| e.into())
    }

    pub fn write(&self, batch: WriteBatch, sync: bool) -> Result<u8, LevelDBError> {
        let mut guard = self.get_db()?;
        guard.write(batch.batch, sync).map(|_| 0).map_err(|e| e.into())
    }

    pub fn get_snapshot(&self) -> Result<Snapshot, LevelDBError> {
        let mut guard = self.get_db()?;
        let snap = guard.get_snapshot();
        Ok(Snapshot { snapshot: Some(snap) })
    }

    pub fn get_at(&self, snapshot: &Snapshot, k: &[u8]) -> Result<Option<DBValue>, LevelDBError> {
        let mut guard = self.get_db()?;
        let Some(s) = &snapshot.snapshot else {
            return Err(LevelDBError::from("Snapshot is empty"));
        };
        guard.get_at(s, k).map(|opt| opt.map(|v| DBValue { inner: v })).map_err(|e| e.into())
    }

    pub fn new_iter(&self) -> Result<DBIterator, LevelDBError> {
        let mut guard = self.get_db()?;
        guard.new_iter().map(|iter| DBIterator { iterator: Some(iter) }).map_err(|e| e.into())
    }

    pub fn new_iter_at(&self, snapshot: &Snapshot) -> Result<DBIterator, LevelDBError> {
        let mut guard = self.get_db()?;
        let Some(s) = snapshot.snapshot.clone() else {
            return Err(LevelDBError::from("Snapshot is empty"));
        };
        guard.new_iter_at(s).map(|iter| DBIterator { iterator: Some(iter) }).map_err(|e| e.into())
    }

    pub fn compact_range(&self, start: &[u8], end: &[u8]) -> Result<u8, LevelDBError> {
        let mut guard = self.get_db()?;
        guard.compact_range(start, end).map(|_| 0).map_err(|e| e.into())
    }
}
