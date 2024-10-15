//! Keymanager interface.
use std::sync::Arc;

use tiny_keccak::{Hasher, TupleHash};

use oasis_core_keymanager::client::{KeyManagerClient as CoreKeyManagerClient, RemoteClient};
pub use oasis_core_keymanager::{
    api::KeyManagerError,
    crypto::{KeyPair, KeyPairId, SignedPublicKey, StateKey},
    policy::TrustedSigners,
};
use oasis_core_runtime::{
    common::{crypto::signature::PublicKey, namespace::Namespace},
    consensus::{beacon::EpochTime, verifier::Verifier},
    future::block_on,
    identity::Identity,
    protocol::Protocol,
    RpcDispatcher,
};

/// Key manager interface. This is a runtime context-resident convenience
/// wrapper to the keymanager configured for the runtime.
pub(crate) struct KeyManagerClient {
    inner: Arc<dyn CoreKeyManagerClient>,
}

impl KeyManagerClient {
    /// Create a new key manager client using the default remote client from oasis-core.
    pub(crate) fn new(
        runtime_id: Namespace,
        protocol: Arc<Protocol>,
        consensus_verifier: Arc<dyn Verifier>,
        identity: Arc<Identity>,
        rpc: &mut RpcDispatcher,
        key_cache_sizes: usize,
        signers: TrustedSigners,
    ) -> Self {
        let remote_client = Arc::new(RemoteClient::new_runtime(
            runtime_id,
            protocol,
            consensus_verifier,
            identity,
            key_cache_sizes,
            signers,
        ));

        // Setup the quote policy update handler.
        let handler_remote_client = remote_client.clone();
        rpc.set_keymanager_quote_policy_update_handler(Some(Box::new(move |policy| {
            block_on(handler_remote_client.set_quote_policy(policy));
        })));

        // Setup the status update handler.
        let handler_remote_client = remote_client.clone();
        rpc.set_keymanager_status_update_handler(Some(Box::new(move |status| {
            block_on(handler_remote_client.set_status(status))
                .expect("failed to update km client status");
        })));

        KeyManagerClient {
            inner: remote_client,
        }
    }

    /// Create a client proxy which will forward calls to the inner client using the given context.
    /// Only public key queries will be allowed.
    pub(crate) fn with_context(self: &Arc<Self>) -> Box<dyn KeyManager> {
        Box::new(KeyManagerClientWithContext::new(self.clone(), false)) as Box<dyn KeyManager>
    }

    /// Create a client proxy which will forward calls to the inner client using the given context.
    /// Public and private key queries will be allowed.
    pub(crate) fn with_private_context(self: &Arc<Self>) -> Box<dyn KeyManager> {
        Box::new(KeyManagerClientWithContext::new(self.clone(), true)) as Box<dyn KeyManager>
    }

    /// Key manager runtime identifier this client is connected to. It may be `None` in case the
    /// identifier is not known yet (e.g. the client has not yet been initialized).
    ///
    /// See the oasis-core documentation for details.
    pub(crate) fn runtime_id(&self) -> Option<Namespace> {
        self.inner.runtime_id()
    }

    /// Key manager runtime signing key used to sign messages from the key manager.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) fn runtime_signing_key(&self) -> Option<PublicKey> {
        self.inner.runtime_signing_key()
    }

    /// Clear local key cache.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) fn clear_cache(&self) {
        self.inner.clear_cache()
    }

    /// Get or create named key pair.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) async fn get_or_create_keys(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<KeyPair, KeyManagerError> {
        retryable(|| self.inner.get_or_create_keys(key_pair_id, 0)).await
    }

    /// Get public key for a key pair id.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) async fn get_public_key(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<SignedPublicKey, KeyManagerError> {
        retryable(|| self.inner.get_public_key(key_pair_id, 0)).await
    }

    /// Get or create named ephemeral key pair for given epoch.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) async fn get_or_create_ephemeral_keys(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<KeyPair, KeyManagerError> {
        retryable(|| self.inner.get_or_create_ephemeral_keys(key_pair_id, epoch)).await
    }

    /// Get ephemeral public key for an epoch and a key pair id.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) async fn get_public_ephemeral_key(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<SignedPublicKey, KeyManagerError> {
        retryable(|| self.inner.get_public_ephemeral_key(key_pair_id, epoch)).await
    }
}

/// Decorator for remote method calls that can be safely retried.
async fn retryable<A>(action: A) -> Result<A::Item, A::Error>
where
    A: tokio_retry::Action,
{
    let retry_strategy = tokio_retry::strategy::ExponentialBackoff::from_millis(4)
        .max_delay(std::time::Duration::from_millis(250))
        .map(tokio_retry::strategy::jitter)
        .take(5);

    tokio_retry::Retry::spawn(retry_strategy, action).await
}

/// Key manager interface.
pub trait KeyManager {
    /// Key manager runtime identifier this client is connected to. It may be `None` in case the
    /// identifier is not known yet (e.g. the client has not yet been initialized).
    ///
    /// See the oasis-core documentation for details.
    fn runtime_id(&self) -> Option<Namespace>;

    /// Key manager runtime signing key used to sign messages from the key manager.
    ///
    /// See the oasis-core documentation for details.
    fn runtime_signing_key(&self) -> Option<PublicKey>;

    /// Clear local key cache.
    ///
    /// See the oasis-core documentation for details.
    fn clear_cache(&self);

    /// Get or create named key pair.
    ///
    /// See the oasis-core documentation for details. This variant of the method
    /// synchronously blocks for the result.
    fn get_or_create_keys(&self, key_pair_id: KeyPairId) -> Result<KeyPair, KeyManagerError>;

    /// Get public key for a key pair id.
    ///
    /// See the oasis-core documentation for details. This variant of the method
    /// synchronously blocks for the result.
    fn get_public_key(&self, key_pair_id: KeyPairId) -> Result<SignedPublicKey, KeyManagerError>;

    /// Get or create named ephemeral key pair for given epoch.
    ///
    /// See the oasis-core documentation for details. This variant of the method
    /// synchronously blocks for the result.
    fn get_or_create_ephemeral_keys(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<KeyPair, KeyManagerError>;

    /// Get ephemeral public key for an epoch and a key pair id.
    ///
    /// See the oasis-core documentation for details. This variant of the method
    /// synchronously blocks for the result.
    fn get_public_ephemeral_key(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<SignedPublicKey, KeyManagerError>;

    fn box_clone(&self) -> Box<dyn KeyManager>;
}

impl Clone for Box<dyn KeyManager> {
    fn clone(&self) -> Box<dyn KeyManager> {
        self.box_clone()
    }
}

/// Convenience wrapper around an existing KeyManagerClient instance which uses
/// a default io context for all calls.
#[derive(Clone)]
pub struct KeyManagerClientWithContext {
    parent: Arc<KeyManagerClient>,
    allow_private: bool,
}

impl KeyManagerClientWithContext {
    fn new(parent: Arc<KeyManagerClient>, allow_private: bool) -> KeyManagerClientWithContext {
        KeyManagerClientWithContext {
            parent,
            allow_private,
        }
    }

    /// Get or create named key pair.
    ///
    /// See the oasis-core documentation for details.
    async fn get_or_create_keys_async(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<KeyPair, KeyManagerError> {
        if !self.allow_private {
            return Err(KeyManagerError::Other(anyhow::anyhow!(
                "not allowed by local runtime policy"
            )));
        }

        self.parent.get_or_create_keys(key_pair_id).await
    }

    /// Get public key for a key pair id.
    ///
    /// See the oasis-core documentation for details.
    async fn get_public_key_async(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<SignedPublicKey, KeyManagerError> {
        self.parent.get_public_key(key_pair_id).await
    }

    /// Get ephemeral public key for an epoch and a key pair id.
    ///
    /// See the oasis-core documentation for details.
    async fn get_or_create_ephemeral_keys_async(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<KeyPair, KeyManagerError> {
        if !self.allow_private {
            return Err(KeyManagerError::Other(anyhow::anyhow!(
                "not allowed by local runtime policy"
            )));
        }

        self.parent
            .get_or_create_ephemeral_keys(key_pair_id, epoch)
            .await
    }

    /// Get ephemeral public key for an epoch and a key pair id.
    ///
    /// See the oasis-core documentation for details.
    async fn get_public_ephemeral_key_async(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<SignedPublicKey, KeyManagerError> {
        self.parent
            .get_public_ephemeral_key(key_pair_id, epoch)
            .await
    }
}

impl KeyManager for KeyManagerClientWithContext {
    fn runtime_id(&self) -> Option<Namespace> {
        self.parent.runtime_id()
    }

    fn runtime_signing_key(&self) -> Option<PublicKey> {
        self.parent.runtime_signing_key()
    }

    fn clear_cache(&self) {
        self.parent.clear_cache();
    }

    fn get_or_create_keys(&self, key_pair_id: KeyPairId) -> Result<KeyPair, KeyManagerError> {
        block_on(self.get_or_create_keys_async(key_pair_id))
    }

    fn get_public_key(&self, key_pair_id: KeyPairId) -> Result<SignedPublicKey, KeyManagerError> {
        block_on(self.get_public_key_async(key_pair_id))
    }

    fn get_or_create_ephemeral_keys(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<KeyPair, KeyManagerError> {
        block_on(self.get_or_create_ephemeral_keys_async(key_pair_id, epoch))
    }

    fn get_public_ephemeral_key(
        &self,
        key_pair_id: KeyPairId,
        epoch: EpochTime,
    ) -> Result<SignedPublicKey, KeyManagerError> {
        block_on(self.get_public_ephemeral_key_async(key_pair_id, epoch))
    }

    fn box_clone(&self) -> Box<dyn KeyManager> {
        Box::new(self.clone())
    }
}

/// Key pair ID domain separation context.
pub const KEY_PAIR_ID_CONTEXT: &[u8] = b"oasis-runtime-sdk/keymanager: key pair id";

/// Derive a `KeyPairId` for use with the key manager functions.
pub fn get_key_pair_id<'a, C>(context: C) -> KeyPairId
where
    C: IntoIterator<Item = &'a [u8]> + 'a,
{
    let mut h = TupleHash::v256(KEY_PAIR_ID_CONTEXT);
    for item in context.into_iter() {
        h.update(item);
    }
    let mut key_pair_id = [0u8; 32];
    h.finalize(&mut key_pair_id);

    KeyPairId(key_pair_id)
}
