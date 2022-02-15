//! Mock key manager implementation.
pub use oasis_core_keymanager_api_common::{
    KeyManagerError, KeyPair, KeyPairId, PrivateKey, PublicKey, SignedPublicKey, StateKey,
    TrustedPolicySigners,
};

use crate::keymanager::KeyManager;

#[derive(Clone, Default)]
pub struct MockKeyManagerClient {}

impl MockKeyManagerClient {
    pub fn new() -> Self {
        MockKeyManagerClient {}
    }
}

impl KeyManager for MockKeyManagerClient {
    fn clear_cache(&self) {
        // Nothing to do here, no cache.
    }

    fn get_or_create_keys(&self, _key_pair_id: KeyPairId) -> Result<KeyPair, KeyManagerError> {
        let mut kp = KeyPair::generate_mock();
        kp.state_key.0.fill(0x33);
        Ok(kp)
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
