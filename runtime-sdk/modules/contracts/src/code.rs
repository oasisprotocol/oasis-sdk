//! Code caching and storage.
use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    sync::Mutex,
};

use once_cell::sync::Lazy;

use oasis_runtime_sdk::{
    context::Context,
    core::common::crypto::hash::Hash,
    storage::{self, Store},
};

use crate::{state, types, Config, Error, Module, MODULE_NAME};

/// A global in-memory LRU cache of code instances.
static CODE_CACHE: Lazy<Mutex<lru::LruCache<Hash, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(lru::LruCache::new(NonZeroUsize::new(128).unwrap())));

impl<Cfg: Config> Module<Cfg> {
    /// Loads code with the specified code identifier.
    pub fn load_code<C: Context>(ctx: &mut C, code_info: &types::Code) -> Result<Vec<u8>, Error> {
        let mut cache = CODE_CACHE.lock().unwrap();
        if let Some(code) = cache.get(&code_info.hash) {
            return Ok(code.clone());
        }

        // TODO: Support local untrusted cache to avoid storage queries.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let code_store = storage::PrefixStore::new(&mut store, &state::CODE);
        let code = code_store
            .get(&code_info.id.to_storage_key())
            .ok_or_else(|| Error::CodeNotFound(code_info.id.as_u64()))?;

        // Decompress code.
        let mut output = Vec::with_capacity(code.len());
        let mut decoder = snap::read::FrameDecoder::new(code.as_slice());
        decoder.read_to_end(&mut output).unwrap();

        // Cache uncompressed code for later use.
        cache.put(code_info.hash, output.clone());

        Ok(output)
    }

    /// Stores code with the specified code identifier.
    pub fn store_code<C: Context>(
        ctx: &mut C,
        code_info: &types::Code,
        code: &[u8],
    ) -> Result<(), Error> {
        // If the code is currently cached replace it, otherwise don't do anything.
        let mut cache = CODE_CACHE.lock().unwrap();
        if cache.contains(&code_info.hash) {
            cache.put(code_info.hash, code.to_vec());
        }

        // Compress code before storing it in storage.
        let mut output = Vec::with_capacity(code.len() << 3);
        let mut encoder = snap::write::FrameEncoder::new(&mut output);
        encoder.write_all(code).unwrap();
        drop(encoder); // Make sure data is flushed.

        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut code_store = storage::PrefixStore::new(&mut store, &state::CODE);
        code_store.insert(&code_info.id.to_storage_key(), &output);

        Ok(())
    }
}
