use std::{
    collections::{btree_map, BTreeMap, HashSet},
    iter::{Iterator, Peekable},
};

use oasis_core_runtime::storage::mkvs;

use super::Store;

pub struct OverlayStore<S: Store> {
    parent: S,
    overlay: BTreeMap<Vec<u8>, Vec<u8>>,
    dirty: HashSet<Vec<u8>>,
}

impl<S: Store> OverlayStore<S> {
    pub fn new(parent: S) -> Self {
        Self {
            parent,
            overlay: BTreeMap::new(),
            dirty: HashSet::new(),
        }
    }

    pub fn commit(mut self) {
        // Insert all items present in the overlay.
        for (key, value) in self.overlay {
            self.dirty.remove(&key);
            self.parent.insert(key, &value);
        }

        // Any remaining dirty items must have been removed.
        for key in &self.dirty {
            self.parent.remove(key);
        }
    }
}

impl<S: Store> Store for OverlayStore<S> {
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<Vec<u8>> {
        // For dirty values, check the overlay.
        if self.dirty.contains(key.as_ref()) {
            return self.overlay.get(key.as_ref()).cloned();
        }

        // Otherwise fetch from parent store.
        self.parent.get(key)
    }

    fn insert<K: AsRef<[u8]>>(&mut self, key: K, value: &[u8]) {
        self.overlay
            .insert(key.as_ref().to_owned(), value.to_owned());
        self.dirty.insert(key.as_ref().to_owned());
    }

    fn remove<K: AsRef<[u8]>>(&mut self, key: K) {
        // For dirty values, remove from the overlay.
        if self.dirty.contains(key.as_ref()) {
            self.overlay.remove(key.as_ref());
            return;
        }

        // Since we don't care about the previous value, we can just record an update.
        self.dirty.insert(key.as_ref().to_owned());
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(OverlayStoreIterator::new(self))
    }
}

/// An iterator over the `OverlayStore`.
pub(crate) struct OverlayStoreIterator<'store, S: Store> {
    store: &'store OverlayStore<S>,

    parent: Box<dyn mkvs::Iterator + 'store>,

    overlay: Peekable<btree_map::Range<'store, Vec<u8>, Vec<u8>>>,
    overlay_valid: bool,

    key: Option<Vec<u8>>,
    value: Option<Vec<u8>>,
}

impl<'store, S: Store> OverlayStoreIterator<'store, S> {
    fn new(store: &'store OverlayStore<S>) -> Self {
        Self {
            store,
            parent: store.parent.iter(),
            overlay: store.overlay.range(vec![]..).peekable(),
            overlay_valid: true,
            key: None,
            value: None,
        }
    }

    fn update_iterator_position(&mut self) {
        // Skip over any dirty entries from the parent iterator.
        loop {
            if !self.parent.is_valid()
                || !self
                    .store
                    .dirty
                    .contains(self.parent.get_key().as_ref().expect("parent.is_valid"))
            {
                break;
            }
            self.parent.next();
        }

        let i_key = self.parent.get_key();
        let o_item = self.overlay.peek();
        self.overlay_valid = o_item.is_some();

        if self.parent.is_valid()
            && (!self.overlay_valid
                || i_key.as_ref().expect("parent.is_valid") < o_item.expect("overlay_valid").0)
        {
            // Key of parent iterator is smaller than the key of the overlay iterator.
            self.key = i_key.clone();
            self.value = self.parent.get_value().clone();
        } else if self.overlay_valid {
            // Key of overlay iterator is smaller than or equal to the key of the parent iterator.
            let (o_key, o_value) = o_item.expect("overlay_valid");
            self.key = Some(o_key.to_vec());
            self.value = Some(o_value.to_vec());
        } else {
            // Both iterators are invalid.
            self.key = None;
            self.value = None;
        }
    }

    fn next(&mut self) {
        if !self.overlay_valid
            || (self.parent.is_valid()
                && self.parent.get_key().as_ref().expect("parent.is_valid")
                    <= self.overlay.peek().expect("overlay_valid").0)
        {
            // Key of parent iterator is smaller or equal than the key of the overlay iterator.
            self.parent.next();
        } else {
            // Key of parent iterator is greater than the key of the overlay iterator.
            self.overlay.next();
        }

        self.update_iterator_position();
    }
}

impl<'store, S: Store> Iterator for OverlayStoreIterator<'store, S> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        use mkvs::Iterator;

        if !self.is_valid() {
            return None;
        }

        let key = self.key.as_ref().expect("iterator is valid").clone();
        let value = self.value.as_ref().expect("iterator is valid").clone();
        OverlayStoreIterator::next(self);

        Some((key, value))
    }
}

impl<'store, S: Store> mkvs::Iterator for OverlayStoreIterator<'store, S> {
    fn set_prefetch(&mut self, prefetch: usize) {
        self.parent.set_prefetch(prefetch)
    }

    fn is_valid(&self) -> bool {
        // If either iterator is valid, the merged iterator is valid.
        self.parent.is_valid() || self.overlay_valid
    }

    fn error(&self) -> &Option<anyhow::Error> {
        self.parent.error()
    }

    fn rewind(&mut self) {
        self.seek(&[]);
    }

    fn seek(&mut self, key: &[u8]) {
        self.parent.seek(key);
        self.overlay = self.store.overlay.range(key.to_vec()..).peekable();

        self.update_iterator_position();
    }

    fn get_key(&self) -> &Option<mkvs::Key> {
        &self.key
    }

    fn get_value(&self) -> &Option<Vec<u8>> {
        &self.value
    }

    fn next(&mut self) {
        OverlayStoreIterator::next(self)
    }
}
