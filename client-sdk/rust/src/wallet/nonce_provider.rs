use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use async_trait::async_trait;

pub type Nonce = u64;

#[async_trait]
pub trait NonceProvider: Send + Sync {
    /// Fetches the next nonce. May be called several times before any transaction is submitted.
    async fn next_nonce(&self) -> Result<Nonce>;
}

/// A simple `NonceProvider` that returns the next number without accounting
/// for which nonces have already been used.
#[derive(Default)]
pub struct SimpleNonceProvider {
    next_nonce: AtomicU64,
}

impl SimpleNonceProvider {
    pub fn new(start_nonce: Nonce) -> Self {
        Self {
            next_nonce: AtomicU64::new(start_nonce),
        }
    }
}

#[async_trait::async_trait]
impl NonceProvider for SimpleNonceProvider {
    async fn next_nonce(&self) -> anyhow::Result<Nonce> {
        Ok(self.next_nonce.fetch_add(1, Ordering::SeqCst))
    }
}
