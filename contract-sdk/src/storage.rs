//! Smart contract storage interface.

/// Key/value store trait.
pub trait Store {
    /// Fetch a given key from contract storage.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Insert a given key/value pair into contract storage.
    fn insert(&mut self, key: &[u8], value: &[u8]);

    /// Remove a given key from contract storage.
    fn remove(&mut self, key: &[u8]);
}

/// Marker trait for stores backed by public storage.
pub trait PublicStore: Store {}

/// Marker trait for stores backed by confidential storage.
pub trait ConfidentialStore: Store {}
