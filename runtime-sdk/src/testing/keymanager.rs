//! Mock key manager implementation.

use std::{collections::HashMap, sync::Mutex};

pub use crate::keymanager::{
    KeyManagerError, KeyPair, KeyPairId, PrivateKey, PublicKey, SignedPublicKey, StateKey,
    TrustedPolicySigners,
};
use crate::{core::consensus::beacon::EpochTime, keymanager::KeyManager};

#[derive(Default)]
pub struct MockKeyManagerClient {
    keys: Mutex<HashMap<KeyPairId, KeyPair>>,
    ephemeral_keys: Mutex<HashMap<KeyPairId, KeyPair>>,
}

impl Clone for MockKeyManagerClient {
    fn clone(&self) -> Self {
        let keys = self.keys.lock().unwrap();
        let ephemeral_keys = self.ephemeral_keys.lock().unwrap();
        Self {
            keys: Mutex::new(keys.clone()),
            ephemeral_keys: Mutex::new(ephemeral_keys.clone()),
        }
    }
}

impl MockKeyManagerClient {
    pub fn new() -> Self {
        Default::default()
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

    fn get_or_create_ephemeral_keys(
        &self,
        key_pair_id: KeyPairId,
        _epoch: EpochTime,
    ) -> Result<KeyPair, KeyManagerError> {
        let mut ephemeral_keys = self.ephemeral_keys.lock().unwrap();
        Ok(ephemeral_keys
            .entry(key_pair_id)
            .or_insert_with(|| {
                let mut kp = KeyPair::generate_mock();
                kp.state_key.0.fill(0x33);
                kp
            })
            .clone())
    }

    fn get_public_ephemeral_key(
        &self,
        _key_pair_id: KeyPairId,
        _epoch: EpochTime,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError> {
        Err(KeyManagerError::NotInitialized)
    }

    fn box_clone(&self) -> Box<dyn KeyManager> {
        Box::new(self.clone())
    }
}
