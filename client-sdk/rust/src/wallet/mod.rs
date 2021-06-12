pub mod dummy;
pub mod ed25519;

use anyhow::Result;
use async_trait::async_trait;

use oasis_runtime_sdk::types::transaction::{AddressSpec, AuthProof};

pub type Nonce = u64;

#[async_trait]
pub trait Wallet: Signer + NonceProvider {
    fn address(&self) -> &AddressSpec;
}

#[async_trait]
pub trait Signer: Send + Sync {
    async fn sign(&self, context: &[u8], message: &[u8]) -> Result<AuthProof>;
}

#[async_trait]
pub trait NonceProvider: Send + Sync {
    /// Fetches the next nonce. May be called several times before any transaction is submitted.
    async fn next_nonce(&self) -> Result<Nonce>;
}
