//! Keymanager interface.
use std::sync::Arc;

use io_context::Context as IoContext;
use tiny_keccak::{Hasher, TupleHash};
use tokio::runtime::Handle as TokioHandle;

pub use oasis_core_keymanager_api_common::{
    KeyManagerError, KeyPair, KeyPairId, PrivateKey, PublicKey, SignedPublicKey, StateKey,
    TrustedPolicySigners,
};
use oasis_core_keymanager_client::{KeyManagerClient as CoreKeyManagerClient, RemoteClient};
use oasis_core_runtime::{
    common::namespace::Namespace, protocol::Protocol, rak::RAK, RpcDispatcher,
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
        rak: Arc<RAK>,
        rpc: &mut RpcDispatcher,
        key_cache_sizes: usize,
        signers: TrustedPolicySigners,
    ) -> Self {
        let remote_client = Arc::new(RemoteClient::new_runtime(
            runtime_id,
            protocol,
            rak,
            key_cache_sizes,
            signers,
        ));
        let handler_remote_client = remote_client.clone();
        rpc.set_keymanager_policy_update_handler(Some(Box::new(move |raw_signed_policy| {
            handler_remote_client
                .set_policy(raw_signed_policy)
                .expect("failed to update key manager policy");
        })));
        KeyManagerClient {
            inner: remote_client,
        }
    }

    /// Create a client proxy which will forward calls to the inner client using the given context.
    /// Only public key queries will be allowed.
    pub(crate) fn with_context(self: &Arc<Self>, ctx: Arc<IoContext>) -> Box<dyn KeyManager> {
        Box::new(KeyManagerClientWithContext::new(self.clone(), ctx, false)) as Box<dyn KeyManager>
    }

    /// Create a client proxy which will forward calls to the inner client using the given context.
    /// Public and private key queries will be allowed.
    pub(crate) fn with_private_context(
        self: &Arc<Self>,
        ctx: Arc<IoContext>,
    ) -> Box<dyn KeyManager> {
        Box::new(KeyManagerClientWithContext::new(self.clone(), ctx, true)) as Box<dyn KeyManager>
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
        ctx: IoContext,
        key_pair_id: KeyPairId,
    ) -> Result<KeyPair, KeyManagerError> {
        self.inner.get_or_create_keys(ctx, key_pair_id).await
    }

    /// Get public key for a key pair id.
    ///
    /// See the oasis-core documentation for details.
    pub(crate) async fn get_public_key(
        &self,
        ctx: IoContext,
        key_pair_id: KeyPairId,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError> {
        self.inner.get_public_key(ctx, key_pair_id).await
    }
}

/// Key manager interface.
pub trait KeyManager {
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
    fn get_public_key(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError>;

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
    ctx: Arc<IoContext>,
}

impl KeyManagerClientWithContext {
    fn new(
        parent: Arc<KeyManagerClient>,
        ctx: Arc<IoContext>,
        allow_private: bool,
    ) -> KeyManagerClientWithContext {
        KeyManagerClientWithContext {
            parent,
            ctx,
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
            return Err(KeyManagerError::NotAuthenticated);
        }

        self.parent
            .get_or_create_keys(IoContext::create_child(&self.ctx), key_pair_id)
            .await
    }

    /// Get public key for a key pair id.
    ///
    /// See the oasis-core documentation for details.
    async fn get_public_key_async(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError> {
        self.parent
            .get_public_key(IoContext::create_child(&self.ctx), key_pair_id)
            .await
    }
}

impl KeyManager for KeyManagerClientWithContext {
    fn clear_cache(&self) {
        self.parent.clear_cache();
    }

    fn get_or_create_keys(&self, key_pair_id: KeyPairId) -> Result<KeyPair, KeyManagerError> {
        TokioHandle::current().block_on(self.get_or_create_keys_async(key_pair_id))
    }

    fn get_public_key(
        &self,
        key_pair_id: KeyPairId,
    ) -> Result<Option<SignedPublicKey>, KeyManagerError> {
        TokioHandle::current().block_on(self.get_public_key_async(key_pair_id))
    }

    fn box_clone(&self) -> Box<dyn KeyManager> {
        Box::new(self.clone())
    }
}

/// Key pair ID domain separation context.
pub const KEY_PAIR_ID_CONTEXT: &[u8] = b"oasis-runtime-sdk/keymanager: key pair id";

/// Derive a `KeyPairId` for use with the key manager functions.
pub fn get_key_pair_id(context: &[&[u8]]) -> KeyPairId {
    let mut h = TupleHash::v256(KEY_PAIR_ID_CONTEXT);
    for item in context {
        h.update(item);
    }
    let mut key_pair_id = [0u8; 32];
    h.finalize(&mut key_pair_id);

    KeyPairId(key_pair_id)
}
