use anyhow::Result;
use async_trait::async_trait;

use oasis_runtime_sdk::{
    core::common::crypto::signature::{PrivateKey as CorePrivateKey, Signer as _},
    crypto::signature::PublicKey,
    types::transaction::{AddressSpec, AuthProof},
};

use super::{Nonce, NonceProvider, Signer, Wallet};

pub struct Ed25519Wallet<N> {
    address: AddressSpec,
    private_key: CorePrivateKey,
    nonce_provider: N,
}

impl<N: NonceProvider> Ed25519Wallet<N> {
    pub fn new(keypair: ed25519_dalek::Keypair, nonce_provider: N) -> Self {
        let private_key = CorePrivateKey(keypair);
        let address = AddressSpec::Signature(PublicKey::Ed25519(private_key.public_key().into()));
        Self {
            address,
            private_key,
            nonce_provider,
        }
    }
}

#[async_trait]
impl<N: NonceProvider> Signer for Ed25519Wallet<N> {
    async fn sign(&self, context: &[u8], message: &[u8]) -> Result<AuthProof> {
        Ok(AuthProof::Signature(
            self.private_key.sign(context, message)?.0.to_vec().into(),
        ))
    }
}

#[async_trait]
impl<N: NonceProvider> NonceProvider for Ed25519Wallet<N> {
    async fn next_nonce(&self) -> Result<Nonce> {
        self.nonce_provider.next_nonce().await
    }
}

#[async_trait]
impl<N: NonceProvider> Wallet for Ed25519Wallet<N> {
    fn address(&self) -> &AddressSpec {
        &self.address
    }
}
