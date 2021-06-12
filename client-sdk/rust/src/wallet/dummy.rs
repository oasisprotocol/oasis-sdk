//! Dummy implementations of [`Wallet`], [`Signer`], and [`NonceProvider`] for testing
//! and runtimes that use different (or no) authentication.

use anyhow::Result;
use async_trait::async_trait;

use oasis_runtime_sdk::{
    crypto::signature::{ed25519, PublicKey},
    types::transaction::{AddressSpec, AuthProof},
};

use super::{Nonce, NonceProvider, Signer, Wallet};

/// A wallet for runtimes that do not require signatures.
pub struct DummyWallet {
    address: AddressSpec,
}

impl DummyWallet {
    pub fn new() -> Self {
        Self {
            address: AddressSpec::Signature(PublicKey::Ed25519(
                ed25519::PublicKey::from_bytes(&[0; 32]).unwrap(),
            )),
        }
    }
}

#[async_trait]
impl Wallet for DummyWallet {
    fn address(&self) -> &AddressSpec {
        &self.address
    }
}

#[async_trait]
impl Signer for DummyWallet {
    async fn sign(&self, context: &[u8], message: &[u8]) -> Result<AuthProof> {
        DummySigner.sign(context, message).await
    }
}

#[async_trait]
impl NonceProvider for DummyWallet {
    async fn next_nonce(&self) -> Result<Nonce> {
        DummyNonceProvider.next_nonce().await
    }
}

pub struct DummySigner;

#[async_trait]
impl Signer for DummySigner {
    async fn sign(&self, _context: &[u8], _message: &[u8]) -> Result<AuthProof> {
        Ok(AuthProof::Signature(vec![].into()))
    }
}

pub struct DummyNonceProvider;

#[async_trait]
impl NonceProvider for DummyNonceProvider {
    async fn next_nonce(&self) -> Result<Nonce> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dummy_wallet() {
        let wallet = DummyWallet::new();
        wallet.sign(&[], &[]).await.unwrap();
        wallet.next_nonce().await.unwrap();
    }
}
