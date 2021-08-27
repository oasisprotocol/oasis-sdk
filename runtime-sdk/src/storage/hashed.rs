use std::marker::PhantomData;

use oasis_core_runtime::storage::mkvs;

use super::Store;

/// A key-value store that hashes all keys and stores them as `H(k) || k`.
pub struct HashedStore<S: Store, D: digest::Digest> {
    parent: S,
    _digest: PhantomData<D>,
}

impl<S: Store, D: digest::Digest> HashedStore<S, D> {
    /// Create a new hashed store.
    pub fn new(parent: S) -> Self {
        Self {
            parent,
            _digest: PhantomData,
        }
    }
}

impl<S: Store, D: digest::Digest> Store for HashedStore<S, D> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.parent.get(&[&D::digest(key), key].concat())
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.parent.insert(&[&D::digest(key), key].concat(), value);
    }

    fn remove(&mut self, key: &[u8]) {
        self.parent.remove(&[&D::digest(key), key].concat());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        self.parent.iter()
    }
}
