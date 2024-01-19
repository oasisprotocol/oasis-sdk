use oasis_core_runtime::storage::mkvs;

use super::{NestedStore, Store};

/// A key-value store backed by MKVS.
pub struct MKVSStore<M: mkvs::MKVS> {
    parent: M,
}

impl<M: mkvs::MKVS> MKVSStore<M> {
    pub fn new(parent: M) -> Self {
        Self { parent }
    }
}

impl<M: mkvs::MKVS> Store for MKVSStore<M> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.parent.get(key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.parent.insert(key, value);
    }

    fn remove(&mut self, key: &[u8]) {
        self.parent.remove(key);
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        self.parent.iter()
    }

    fn prefetch_prefixes(&mut self, prefixes: Vec<mkvs::Prefix>, limit: u16) {
        self.parent.prefetch_prefixes(&prefixes, limit);
    }
}

impl<M: mkvs::MKVS> NestedStore for MKVSStore<M> {
    type Inner = M;

    fn commit(self) -> Self::Inner {
        // Commit is not needed.
        self.parent
    }

    fn rollback(self) -> Self::Inner {
        panic!("attempted to rollback a non-transactional store");
    }

    fn has_pending_updates(&self) -> bool {
        true
    }

    fn pending_update_byte_size(&self) -> usize {
        0
    }
}
