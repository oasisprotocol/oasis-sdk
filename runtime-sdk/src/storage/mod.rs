//! Storage.
use oasis_core_runtime::storage::mkvs::Iterator;

mod mkvs;
mod overlay;
mod prefix;
mod typed;

/// A key-value store.
pub trait Store {
    /// Fetch entry with given key.
    fn get<K: StoreKey>(&self, key: K) -> Option<Vec<u8>>;

    /// Update entry with given key to the given value.
    fn insert<K: StoreKey>(&mut self, key: K, value: &[u8]);

    /// Remove entry with given key.
    fn remove<K: StoreKey>(&mut self, key: K);

    /// Returns an iterator over the tree.
    fn iter(&self) -> Box<dyn Iterator + '_>;
}

impl<S: Store + ?Sized> Store for &mut S {
    fn get<K: StoreKey>(&self, key: K) -> Option<Vec<u8>> {
        S::get(self, key)
    }

    fn insert<K: StoreKey>(&mut self, key: K, value: &[u8]) {
        S::insert(self, key, value)
    }

    fn remove<K: StoreKey>(&mut self, key: K) {
        S::remove(self, key)
    }

    fn iter(&self) -> Box<dyn Iterator + '_> {
        S::iter(self)
    }
}

/// Store key trait.
pub trait StoreKey {
    /// Return the type's byte representation for a store prefix key.
    fn as_store_key(&self) -> &[u8];
}

/// Decodable store key trait for store keys which can also be decoded.
pub trait DecodableStoreKey {
    /// Decode the store key from raw bytes.
    ///
    /// Returns `None` in case the key cannot be decoded.
    fn from_bytes(v: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

macro_rules! impl_storekey_fwd {
    ($t:ty) => {
        impl StoreKey for $t {
            fn as_store_key(&self) -> &[u8] {
                &self[..]
            }
        }
    };
}

impl_storekey_fwd!(&[u8]);
impl_storekey_fwd!(Vec<u8>);
impl_storekey_fwd!(&Vec<u8>);
impl_storekey_fwd!([u8; 1]);
impl_storekey_fwd!([u8; 2]);
impl_storekey_fwd!([u8; 3]);
impl_storekey_fwd!([u8; 4]);
impl_storekey_fwd!([u8; 5]);
impl_storekey_fwd!([u8; 6]);
impl_storekey_fwd!([u8; 7]);
impl_storekey_fwd!([u8; 8]);
impl_storekey_fwd!([u8; 9]);
impl_storekey_fwd!([u8; 10]);
impl_storekey_fwd!([u8; 11]);
impl_storekey_fwd!([u8; 12]);
impl_storekey_fwd!([u8; 13]);
impl_storekey_fwd!([u8; 14]);
impl_storekey_fwd!([u8; 15]);
impl_storekey_fwd!([u8; 16]);

impl StoreKey for &str {
    fn as_store_key(&self) -> &[u8] {
        self.as_bytes()
    }
}

pub struct OwnedStoreKey(Vec<u8>);

impl StoreKey for &OwnedStoreKey {
    fn as_store_key(&self) -> &[u8] {
        &self.0
    }
}

impl From<u64> for OwnedStoreKey {
    fn from(v: u64) -> OwnedStoreKey {
        OwnedStoreKey(v.to_be_bytes().to_vec())
    }
}

pub use mkvs::MKVSStore;
pub use overlay::OverlayStore;
pub use prefix::PrefixStore;
pub use typed::TypedStore;
