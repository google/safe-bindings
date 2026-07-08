use crate::DBValue;
use rusty_leveldb::DBIterator as LevelDB_DBIterator;
use rusty_leveldb::LdbIterator;
use std::fmt;
use std::fmt::Debug;

/// A LevelDB database iterator.
///
/// A Default DBIterator (which has the iterator set to None) won't raise errors,
/// but will always return false/None.
#[derive(Default)]
pub struct DBIterator {
    pub iterator: Option<LevelDB_DBIterator>,
}

#[derive(Debug, Default, Clone)]
pub struct DBKeyValueTuple {
    pub key: DBValue,
    pub value: DBValue,
}

impl DBKeyValueTuple {
    pub fn key(&self) -> DBValue {
        self.key.clone()
    }
    pub fn value(&self) -> DBValue {
        self.value.clone()
    }
}

impl DBIterator {
    pub fn advance(&mut self) -> bool {
        match self.iterator {
            Some(ref mut iter) => iter.advance(),
            None => false,
        }
    }
    pub fn prev(&mut self) -> bool {
        match self.iterator {
            Some(ref mut iter) => iter.prev(),
            None => false,
        }
    }
    pub fn valid(&self) -> bool {
        match self.iterator {
            Some(ref iter) => iter.valid(),
            None => false,
        }
    }

    pub fn current(&self) -> Option<DBKeyValueTuple> {
        self.iterator.as_ref().and_then(|iter| {
            iter.current().map(|(k, v)| DBKeyValueTuple {
                key: DBValue { inner: k },
                value: DBValue { inner: v },
            })
        })
    }

    pub fn seek(&mut self, key: &[u8]) {
        if let Some(ref mut iter) = self.iterator {
            iter.seek(key)
        }
    }

    pub fn seek_to_first(&mut self) {
        if let Some(ref mut iter) = self.iterator {
            iter.seek_to_first()
        }
    }

    pub fn reset(&mut self) {
        if let Some(ref mut iter) = self.iterator {
            iter.reset()
        }
    }
}

impl Debug for DBIterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DBIterator").finish()
    }
}
