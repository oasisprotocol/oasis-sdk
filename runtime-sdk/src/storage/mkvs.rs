use std::sync::Arc;

use io_context::Context;

use oasis_core_runtime::storage::mkvs;

use super::{Store, StoreKey};

/// A key-value store backed by MKVS.
pub struct MkvsStore<M: mkvs::MKVS> {
    ctx: Arc<Context>,
    parent: M,
}

impl<M: mkvs::MKVS> MkvsStore<M> {
    pub fn new(ctx: Arc<Context>, parent: M) -> Self {
        Self { ctx, parent }
    }

    #[inline]
    fn create_ctx(&self) -> Context {
        Context::create_child(&self.ctx)
    }
}

impl<M: mkvs::MKVS> Store for MkvsStore<M> {
    fn get<K: StoreKey>(&self, key: K) -> Option<Vec<u8>> {
        self.parent.get(self.create_ctx(), key.as_store_key())
    }

    fn insert<K: StoreKey>(&mut self, key: K, value: &[u8]) {
        self.parent
            .insert(self.create_ctx(), key.as_store_key(), value);
    }

    fn remove<K: StoreKey>(&mut self, key: K) {
        self.parent.remove(self.create_ctx(), key.as_store_key());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        self.parent.iter(self.create_ctx())
    }
}
