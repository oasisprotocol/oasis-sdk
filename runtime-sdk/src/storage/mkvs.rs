use std::sync::Arc;

use io_context::Context;

use oasis_core_runtime::storage::mkvs;

use super::{NestedStore, Store};

/// A key-value store backed by MKVS.
pub struct MKVSStore<M: mkvs::MKVS> {
    ctx: Arc<Context>,
    parent: M,
}

impl<M: mkvs::MKVS> MKVSStore<M> {
    pub fn new(ctx: Arc<Context>, parent: M) -> Self {
        Self { ctx, parent }
    }

    #[inline]
    fn create_ctx(&self) -> Context {
        Context::create_child(&self.ctx)
    }

    /// Populate the in-memory tree with nodes for keys starting with given prefixes.
    pub fn prefetch_prefixes(&mut self, prefixes: Vec<mkvs::Prefix>, limit: u16) {
        self.parent
            .prefetch_prefixes(self.create_ctx(), &prefixes, limit)
    }
}

impl<M: mkvs::MKVS> Store for MKVSStore<M> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.parent.get(self.create_ctx(), key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.parent.insert(self.create_ctx(), key, value);
    }

    fn remove(&mut self, key: &[u8]) {
        self.parent.remove(self.create_ctx(), key);
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        self.parent.iter(self.create_ctx())
    }
}

impl<M: mkvs::MKVS> NestedStore for MKVSStore<M> {
    type Inner = M;

    fn commit(self) -> Self::Inner {
        // Commit is not needed.
        self.parent
    }

    fn has_pending_updates(&self) -> bool {
        true
    }
}
