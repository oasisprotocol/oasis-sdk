//! Storage.
use oasis_core_runtime::storage::mkvs::Iterator;

pub mod confidential;
mod hashed;
pub mod host;
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

    /// Populate the in-memory tree with nodes for keys starting with given prefixes.
    fn prefetch_prefixes(&mut self, prefixes: Vec<Prefix>, limit: u16);
}

/// A key-value store that supports the commit operation.
pub trait NestedStore: Store {
    /// Type of the inner store.
    type Inner;

    /// Commit any changes to the underlying store.
    ///
    /// If this method is not called the changes may be discarded by the store.
    fn commit(self) -> Self::Inner;

    /// Rollback any changes.
    fn rollback(self) -> Self::Inner;

    /// Whether there are any store updates pending to be committed.
    fn has_pending_updates(&self) -> bool;

    /// Size (in bytes) of any pending updates.
    fn pending_update_byte_size(&self) -> usize;
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

    fn prefetch_prefixes(&mut self, prefixes: Vec<Prefix>, limit: u16) {
        S::prefetch_prefixes(self, prefixes, limit)
    }
}

impl<S: Store + ?Sized> Store for Box<S> {
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

    fn prefetch_prefixes(&mut self, prefixes: Vec<Prefix>, limit: u16) {
        S::prefetch_prefixes(self, prefixes, limit)
    }
}

pub use confidential::{ConfidentialStore, Error as ConfidentialStoreError};
pub use hashed::HashedStore;
pub use host::HostStore;
pub use mkvs::MKVSStore;
pub use overlay::OverlayStore;
pub use prefix::PrefixStore;
pub use typed::TypedStore;

// Re-export the mkvs storage prefix.
pub use oasis_core_runtime::storage::mkvs::Prefix;
