use rusty_leveldb::Snapshot as LevelDB_Snapshot;
use std::fmt;

#[derive(Default, Clone)]
pub struct Snapshot {
    pub(crate) snapshot: Option<LevelDB_Snapshot>,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Snapshot").finish()
    }
}
