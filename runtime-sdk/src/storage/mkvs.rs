use std::sync::Arc;

use io_context::Context;

use oasis_core_runtime::storage::mkvs;

use super::Store;

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
}

impl<M: mkvs::MKVS> Store for MKVSStore<M> {
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<Vec<u8>> {
        self.parent.get(self.create_ctx(), key.as_ref())
    }

    fn insert<K: AsRef<[u8]>>(&mut self, key: K, value: &[u8]) {
        self.parent.insert(self.create_ctx(), key.as_ref(), value);
    }

    fn remove<K: AsRef<[u8]>>(&mut self, key: K) {
        self.parent.remove(self.create_ctx(), key.as_ref());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        self.parent.iter(self.create_ctx())
    }
}
