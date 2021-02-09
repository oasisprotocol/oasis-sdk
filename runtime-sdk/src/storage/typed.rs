use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};

use oasis_core_runtime::{common::cbor, storage::mkvs};

use super::{DecodableStoreKey, Store, StoreKey};

/// A key-value store that transparently handles serialization/deserialization.
pub struct TypedStore<S: Store> {
    parent: S,
}

impl<S: Store> TypedStore<S> {
    /// Create a new typed store.
    pub fn new(parent: S) -> Self {
        Self { parent }
    }

    /// Fetch entry with given key.
    pub fn get<K: StoreKey, T: DeserializeOwned>(&self, key: K) -> Option<T> {
        self.parent
            .get(key)
            .map(|data| cbor::from_slice(&data).unwrap())
    }

    /// Update entry with given key to the given value.
    pub fn insert<K: StoreKey, T: Serialize>(&mut self, key: K, value: &T) {
        self.parent.insert(key, &cbor::to_vec(value))
    }

    /// Remove entry with given key.
    pub fn remove<K: StoreKey>(&mut self, key: K) {
        self.parent.remove(key)
    }

    pub fn iter<K: DecodableStoreKey, V: DeserializeOwned>(&self) -> TypedStoreIterator<K, V> {
        TypedStoreIterator::new(self.parent.iter())
    }
}

/// An iterator over the `TypedStore`.
pub struct TypedStoreIterator<'store, K: DecodableStoreKey, V: DeserializeOwned> {
    inner: Box<dyn mkvs::Iterator + 'store>,

    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<'store, K: DecodableStoreKey, V: DeserializeOwned> TypedStoreIterator<'store, K, V> {
    fn new(inner: Box<dyn mkvs::Iterator + 'store>) -> Self {
        Self {
            inner,
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<'store, K: DecodableStoreKey, V: DeserializeOwned> Iterator
    for TypedStoreIterator<'store, K, V>
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        Iterator::next(&mut self.inner).map(|(k, v)| {
            (
                DecodableStoreKey::from_bytes(&k).unwrap(),
                cbor::from_slice(&v).unwrap(),
            )
        })
    }
}
