//! Storage.
use oasis_core_runtime::storage::mkvs::Iterator;

mod mkvs;
mod overlay;
mod prefix;
mod typed;

/// A key-value store.
pub trait Store {
    /// Fetch entry with given key.
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<Vec<u8>>;

    /// Update entry with given key to the given value.
    fn insert<K: AsRef<[u8]>>(&mut self, key: K, value: &[u8]);

    /// Remove entry with given key.
    fn remove<K: AsRef<[u8]>>(&mut self, key: K);

    /// Returns an iterator over the tree.
    fn iter(&self) -> Box<dyn Iterator + '_>;
}

impl<S: Store + ?Sized> Store for &mut S {
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<Vec<u8>> {
        S::get(self, key)
    }

    fn insert<K: AsRef<[u8]>>(&mut self, key: K, value: &[u8]) {
        S::insert(self, key, value)
    }

    fn remove<K: AsRef<[u8]>>(&mut self, key: K) {
        S::remove(self, key)
    }

    fn iter(&self) -> Box<dyn Iterator + '_> {
        S::iter(self)
    }
}

pub use mkvs::MKVSStore;
pub use overlay::OverlayStore;
pub use prefix::PrefixStore;
pub use typed::TypedStore;

// Re-export the mkvs storage prefix.
pub use oasis_core_runtime::storage::mkvs::Prefix;
