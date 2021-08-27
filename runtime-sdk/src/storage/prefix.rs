use oasis_core_runtime::storage::mkvs;

use super::Store;

/// A key-value store that prefixes all keys with the given prefix.
pub struct PrefixStore<S: Store, P: AsRef<[u8]>> {
    parent: S,
    prefix: P,
}

impl<S: Store, P: AsRef<[u8]>> PrefixStore<S, P> {
    /// Create a new prefix store with the given prefix.
    pub fn new(parent: S, prefix: P) -> Self {
        Self { parent, prefix }
    }
}

impl<S: Store, P: AsRef<[u8]>> Store for PrefixStore<S, P> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.parent.get(&[self.prefix.as_ref(), key].concat())
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.parent
            .insert(&[self.prefix.as_ref(), key].concat(), value);
    }

    fn remove(&mut self, key: &[u8]) {
        self.parent.remove(&[self.prefix.as_ref(), key].concat());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(PrefixStoreIterator::new(
            self.parent.iter(),
            self.prefix.as_ref(),
        ))
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
