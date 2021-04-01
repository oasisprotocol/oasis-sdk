use oasis_core_runtime::storage::mkvs;

use super::Store;

/// A key-value store that prefixes all keys with the given prefix.
pub struct PrefixStore<'store, S: Store> {
    parent: S,
    prefix: &'store [u8],
}

impl<'store, S: Store> PrefixStore<'store, S> {
    /// Create a new prefix store with the given prefix.
    pub fn new<K: 'store + AsRef<[u8]>>(parent: S, prefix: &'store K) -> Self {
        Self {
            parent,
            prefix: prefix.as_ref(),
        }
    }
}

impl<'store, S: Store> Store for PrefixStore<'store, S> {
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<Vec<u8>> {
        self.parent.get(&[self.prefix, key.as_ref()].concat())
    }

    fn insert<K: AsRef<[u8]>>(&mut self, key: K, value: &[u8]) {
        self.parent
            .insert(&[self.prefix, key.as_ref()].concat(), value);
    }

    fn remove<K: AsRef<[u8]>>(&mut self, key: K) {
        self.parent.remove(&[self.prefix, key.as_ref()].concat());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(PrefixStoreIterator::new(self.parent.iter(), &self.prefix))
    }
}

/// An iterator over the `PrefixStore`.
pub(crate) struct PrefixStoreIterator<'store> {
    inner: Box<dyn mkvs::Iterator + 'store>,
    prefix: &'store [u8],
}

impl<'store> PrefixStoreIterator<'store> {
    fn new(mut inner: Box<dyn mkvs::Iterator + 'store>, prefix: &'store [u8]) -> Self {
        inner.seek(&prefix);
        Self { inner, prefix }
    }
}

impl<'store> Iterator for PrefixStoreIterator<'store> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        Iterator::next(&mut self.inner).and_then(|(mut k, v)| {
            if k.starts_with(self.prefix) {
                Some((k.split_off(self.prefix.len()), v))
            } else {
                None
            }
        })
    }
}

impl<'store> mkvs::Iterator for PrefixStoreIterator<'store> {
    fn set_prefetch(&mut self, prefetch: usize) {
        self.inner.set_prefetch(prefetch)
    }

    fn is_valid(&self) -> bool {
        if !self
            .inner
            .get_key()
            .as_ref()
            .unwrap_or(&vec![])
            .starts_with(self.prefix)
        {
            return false;
        }
        self.inner.is_valid()
    }

    fn error(&self) -> &Option<anyhow::Error> {
        self.inner.error()
    }

    fn rewind(&mut self) {
        self.inner.seek(&self.prefix);
    }

    fn seek(&mut self, key: &[u8]) {
        self.inner.seek(&[self.prefix, key].concat());
    }

    fn get_key(&self) -> &Option<mkvs::Key> {
        self.inner.get_key()
    }

    fn get_value(&self) -> &Option<Vec<u8>> {
        self.inner.get_value()
    }

    fn next(&mut self) {
        if !self.is_valid() {
            // Could be invalid due to prefix mismatch.
            return;
        }
        mkvs::Iterator::next(&mut *self.inner)
    }
}
