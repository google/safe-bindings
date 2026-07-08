use rusty_leveldb::WriteBatch as LevelDB_WriteBatch;

#[derive(Default)]
pub struct WriteBatch {
    pub(crate) batch: LevelDB_WriteBatch,
}

impl WriteBatch {
    pub fn set_contents(&mut self, contents: &[u8]) {
        self.batch.set_contents(contents);
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) {
        self.batch.put(key, value);
    }

    pub fn delete(&mut self, key: &[u8]) {
        self.batch.delete(key);
    }

    pub fn clear(&mut self) {
        self.batch.clear();
    }

    pub fn count(&self) -> u32 {
        self.batch.count()
    }

    pub fn sequence(&self) -> u64 {
        self.batch.sequence()
    }
}

impl Clone for WriteBatch {
    fn clone(&self) -> Self {
        let mut new_batch = LevelDB_WriteBatch::default();
        for (k, v) in self.batch.iter() {
            match v {
                Some(v_) => new_batch.put(k, v_),
                None => new_batch.delete(k),
            }
        }
        WriteBatch { batch: new_batch }
    }
}
