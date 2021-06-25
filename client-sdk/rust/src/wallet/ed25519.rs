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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sign_verify() {
        let keypair = oasis_runtime_sdk::core::common::crypto::signature::PrivateKey::generate();
        let public_key: oasis_runtime_sdk::crypto::signature::ed25519::PublicKey =
            keypair.public_key().into();
        let wallet = Ed25519Wallet::new(
            keypair.0,
            crate::wallet::nonce_provider::SimpleNonceProvider::default(),
        );
        let context = b"world";
        let message = b"hello";
        let signature = match wallet.sign(context, message).await.unwrap() {
            oasis_runtime_sdk::types::transaction::AuthProof::Signature(sig) => sig,
            _ => panic!("expected single signature"),
        };
        public_key.verify(context, message, &signature).unwrap();
    }
}
