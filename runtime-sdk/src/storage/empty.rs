use oasis_core_runtime::storage::mkvs;

/// A key-value store that is always empty. Useful for testing,
/// and when there is no better store is not available.
#[derive(Default)]
pub struct EmptyStore;

impl EmptyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl super::Store for EmptyStore {
    fn get(&self, _key: &[u8]) -> Option<Vec<u8>> {
        None
    }

    fn insert(&mut self, _key: &[u8], _value: &[u8]) {}

    fn remove(&mut self, _key: &[u8]) {}

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(EmptyStoreIter)
    }
}

#[derive(Clone, Copy, Debug)]
struct EmptyStoreIter;

impl std::iter::Iterator for EmptyStoreIter {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

impl mkvs::Iterator for EmptyStoreIter {
    fn set_prefetch(&mut self, _prefetch: usize) {}

    fn is_valid(&self) -> bool {
        true
    }

    fn error(&self) -> &Option<anyhow::Error> {
        &None
    }

    fn rewind(&mut self) {}

    fn seek(&mut self, _key: &[u8]) {}

    fn get_key(&self) -> &Option<mkvs::Key> {
        &None
    }

    fn get_value(&self) -> &Option<Vec<u8>> {
        &None
    }

    fn next(&mut self) {}
}
