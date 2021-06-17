pub mod ed25519;
pub mod nonce_provider;

use anyhow::Result;
use async_trait::async_trait;

use oasis_runtime_sdk::types::transaction::{AddressSpec, AuthProof};

pub use nonce_provider::{Nonce, NonceProvider};

#[async_trait]
pub trait Wallet: Signer + NonceProvider {
    fn address(&self) -> &AddressSpec;
}

#[async_trait]
pub trait Signer: Send + Sync {
    async fn sign(&self, context: &[u8], message: &[u8]) -> Result<AuthProof>;
}
