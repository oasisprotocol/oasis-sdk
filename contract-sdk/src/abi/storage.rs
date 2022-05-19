//! Storage ABI.
use crate::{
    memory::{HostRegion, HostRegionRef},
    storage::{ConfidentialStore, PublicStore, Store},
    types::storage::StoreKind,
};

#[link(wasm_import_module = "storage")]
extern "C" {
    #[link_name = "get"]
    fn storage_get(store: u32, key_ptr: u32, key_len: u32) -> *const HostRegion;

    #[link_name = "insert"]
    fn storage_insert(store: u32, key_ptr: u32, key_len: u32, value_ptr: u32, value_len: u32);

    #[link_name = "remove"]
    fn storage_remove(store: u32, key_ptr: u32, key_len: u32);
}

/// Fetches a given key from contract storage.
pub fn get(store: StoreKind, key: &[u8]) -> Option<Vec<u8>> {
    let key_region = HostRegionRef::from_slice(key);
    let rsp_ptr = unsafe { storage_get(store as u32, key_region.offset, key_region.length) };

    // Special value of 0 is treated as if the key doesn't exist.
    if rsp_ptr as u32 == 0 {
        return None;
    }

    Some(unsafe { HostRegion::deref(rsp_ptr) }.into_vec())
}

/// Inserts a given key/value pair into contract storage.
pub fn insert(store: StoreKind, key: &[u8], value: &[u8]) {
    let key_region = HostRegionRef::from_slice(key);
    let value_region = HostRegionRef::from_slice(value);

    unsafe {
        storage_insert(
            store as u32,
            key_region.offset,
            key_region.length,
            value_region.offset,
            value_region.length,
        );
    }
}

/// Removes a given key from contract storage.
pub fn remove(store: StoreKind, key: &[u8]) {
    let key_region = HostRegionRef::from_slice(key);

    unsafe {
        storage_remove(store as u32, key_region.offset, key_region.length);
    }
}

/// Public store backed by the host through the Oasis WASM ABI.
pub struct PublicHostStore;

impl Store for PublicHostStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        get(StoreKind::Public, key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        insert(StoreKind::Public, key, value)
    }

    fn remove(&mut self, key: &[u8]) {
        remove(StoreKind::Public, key)
    }
}

impl PublicStore for PublicHostStore {}

/// Confidential store backed by the host through the Oasis WASM ABI.
pub struct ConfidentialHostStore;

impl Store for ConfidentialHostStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        get(StoreKind::Confidential, key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        insert(StoreKind::Confidential, key, value)
    }

    fn remove(&mut self, key: &[u8]) {
        remove(StoreKind::Confidential, key)
    }
}

impl ConfidentialStore for ConfidentialHostStore {}
