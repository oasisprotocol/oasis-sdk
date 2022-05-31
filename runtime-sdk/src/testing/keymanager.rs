//! Mock key manager implementation.

use std::{collections::HashMap, sync::Mutex};

pub use oasis_core_keymanager_api_common::{
    KeyManagerError, KeyPair, KeyPairId, PrivateKey, PublicKey, SignedPublicKey, StateKey,
    TrustedPolicySigners,
};

use crate::keymanager::KeyManager;

#[derive(Default)]
pub struct MockKeyManagerClient {
    keys: Mutex<HashMap<KeyPairId, KeyPair>>,
}

impl Clone for MockKeyManagerClient {
    fn clone(&self) -> Self {
        let keys = self.keys.lock().unwrap();
        Self {
            keys: Mutex::new(keys.clone()),
        }
    }
}

impl MockKeyManagerClient {
    pub fn new() -> Self {
        MockKeyManagerClient {
            keys: Default::default(),
        }
    }
}

impl KeyManager for MockKeyManagerClient {
    fn clear_cache(&self) {
        // Nothing to do here, no cache.
    }

    fn get_or_create_keys(&self, key_pair_id: KeyPairId) -> Result<KeyPair, KeyManagerError> {
        let mut keys = self.keys.lock().unwrap();
        Ok(keys
            .entry(key_pair_id)
            .or_insert_with(|| {
                let mut kp = KeyPair::generate_mock();
                kp.state_key.0.fill(0x33);
                kp
            })
            .clone())
    }

    fn get_public_key(
        &self,
        _key_pair_id: KeyPairId,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError> {
        Err(KeyManagerError::NotInitialized)
    }

    fn box_clone(&self) -> Box<dyn KeyManager> {
        Box::new(self.clone())
    }
}
