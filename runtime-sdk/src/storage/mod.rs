//! Storage.
use oasis_core_runtime::storage::mkvs::Iterator;

pub mod confidential;
mod hashed;
mod mkvs;
mod overlay;
mod prefix;
mod typed;

/// A key-value store.
pub trait Store {
    /// Fetch entry with given key.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Update entry with given key to the given value.
    fn insert(&mut self, key: &[u8], value: &[u8]);

    /// Remove entry with given key.
    fn remove(&mut self, key: &[u8]);

    /// Returns an iterator over the tree.
    fn iter(&self) -> Box<dyn Iterator + '_>;
}

/// A key-value store that supports the commit operation.
pub trait NestedStore: Store {
    /// Type of the inner store.
    type Inner;

    /// Commit any changes to the underlying store.
    ///
    /// If this method is not called the changes may be discarded by the store.
    fn commit(self) -> Self::Inner;
}

impl<S: Store + ?Sized> Store for &mut S {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        S::get(self, key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        S::insert(self, key, value)
    }

    fn remove(&mut self, key: &[u8]) {
        S::remove(self, key)
    }

    fn iter(&self) -> Box<dyn Iterator + '_> {
        S::iter(self)
    }
}

pub use confidential::{ConfidentialStore, Error as ConfidentialStoreError};
pub use hashed::HashedStore;
pub use mkvs::MKVSStore;
pub use overlay::OverlayStore;
pub use prefix::PrefixStore;
pub use typed::TypedStore;

// Re-export the mkvs storage prefix.
pub use oasis_core_runtime::storage::mkvs::Prefix;
