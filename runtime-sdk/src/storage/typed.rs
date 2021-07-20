use std::{convert::TryFrom, marker::PhantomData};

use oasis_core_runtime::storage::mkvs;

use super::Store;

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
    pub fn get<K: AsRef<[u8]>, T: cbor::Decode>(&self, key: K) -> Option<T> {
        self.parent
            .get(key.as_ref())
            .map(|data| cbor::from_slice(&data).unwrap())
    }

    /// Update entry with given key to the given value.
    pub fn insert<K: AsRef<[u8]>, T: cbor::Encode>(&mut self, key: K, value: T) {
        self.parent.insert(key.as_ref(), &cbor::to_vec(value))
    }

    /// Remove entry with given key.
    pub fn remove<K: AsRef<[u8]>>(&mut self, key: K) {
        self.parent.remove(key.as_ref())
    }

    pub fn iter<'store, K, V>(&'store self) -> TypedStoreIterator<'store, K, V>
    where
        K: for<'k> TryFrom<&'k [u8]>,
        V: cbor::Decode,
    {
        TypedStoreIterator::new(self.parent.iter())
    }
}

/// An iterator over the `TypedStore`.
pub struct TypedStoreIterator<'store, K, V>
where
    K: for<'k> TryFrom<&'k [u8]>,
    V: cbor::Decode,
{
    inner: Box<dyn mkvs::Iterator + 'store>,

    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<'store, K, V> TypedStoreIterator<'store, K, V>
where
    K: for<'k> TryFrom<&'k [u8]>,
    V: cbor::Decode,
{
    fn new(inner: Box<dyn mkvs::Iterator + 'store>) -> Self {
        Self {
            inner,
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<'store, K, V, E> Iterator for TypedStoreIterator<'store, K, V>
where
    K: for<'k> TryFrom<&'k [u8], Error = E>,
    E: std::error::Error,
    V: cbor::Decode,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        Iterator::next(&mut self.inner).map(|(k, v)| {
            let key = K::try_from(&k).unwrap_or_else(|e| panic!("corrupted storage key: {}", e));
            let value = cbor::from_slice(&v).unwrap();
            (key, value)
        })
    }
}
