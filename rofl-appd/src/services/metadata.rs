use std::collections::BTreeMap;

use rocket::async_trait;
use tokio::sync::RwLock;

use oasis_runtime_sdk::modules::rofl::types::MetadataLimits;
use rofl_app_core::prelude::*;

/// Namespace prefix for user-provided metadata keys in on-chain registration.
pub const METADATA_NAMESPACE: &str = "net.oasis.app";

/// Reserved metadata slots for system use (TLS certs, provider attestations, etc).
const RESERVED_METADATA_SLOTS: usize = 10;

/// An instance metadata service.
///
/// User-provided metadata is stored without prefixes and automatically
/// namespaced with "net.oasis.app." when included in on-chain registration.
#[async_trait]
pub trait MetadataService: Send + Sync {
    /// Set metadata.
    ///
    /// This replaces all existing app provided metadata. Will trigger a registration
    /// refresh if the metadata has changed.
    async fn set(&self, metadata: BTreeMap<String, String>) -> Result<(), Error>;

    /// Get all user-set metadata key-value pairs.
    async fn get(&self) -> Result<BTreeMap<String, String>, Error>;
}

/// Error returned by the metadata service.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A service backed by the Oasis runtime.
pub struct OasisMetadataService<A: App> {
    env: Environment<A>,
    metadata: RwLock<BTreeMap<String, String>>,
    limits: MetadataLimits,
}

impl<A: App> OasisMetadataService<A> {
    /// Create a new metadata service, initializing from existing on-chain registration.
    pub async fn new(env: Environment<A>) -> Result<Self, Error> {
        // Query configured metadata limits.
        // TODO: Remove fallback after runtime upgrade includes rofl.MetadataLimits query.
        let round = env.client().latest_round().await?;
        let limits = env
            .client()
            .query::<_, MetadataLimits>(round, "rofl.MetadataLimits", ())
            .await
            .unwrap_or(MetadataLimits {
                max_pairs: 64,
                max_key_size: 1024,
                max_value_size: 16 * 1024,
            });

        Ok(Self {
            env,
            metadata: RwLock::new(BTreeMap::new()),
            limits,
        })
    }
}

#[async_trait]
impl<A: App> MetadataService for OasisMetadataService<A> {
    async fn set(&self, metadata: BTreeMap<String, String>) -> Result<(), Error> {
        // Validate metadata against runtime limits.
        let max_user_pairs =
            (self.limits.max_pairs as usize).saturating_sub(RESERVED_METADATA_SLOTS);
        if metadata.len() > max_user_pairs {
            return Err(Error::InvalidArgument);
        }

        for (key, value) in &metadata {
            // Account for namespace prefix when checking key size.
            let full_key_size = METADATA_NAMESPACE.len() + 1 + key.len();
            if full_key_size > self.limits.max_key_size as usize {
                return Err(Error::InvalidArgument);
            }
            if value.len() > self.limits.max_value_size as usize {
                return Err(Error::InvalidArgument);
            }
        }

        let mut map = self.metadata.write().await;
        *map = metadata;

        // Refresh registration.
        self.env.refresh_registration().await?;

        Ok(())
    }

    async fn get(&self) -> Result<BTreeMap<String, String>, Error> {
        let map = self.metadata.read().await;
        Ok(map.clone())
    }
}

pub struct InMemoryMetadataService {
    metadata: RwLock<BTreeMap<String, String>>,
}

impl Default for InMemoryMetadataService {
    fn default() -> Self {
        Self {
            metadata: RwLock::new(BTreeMap::new()),
        }
    }
}

#[async_trait]
impl MetadataService for InMemoryMetadataService {
    async fn set(&self, metadata: BTreeMap<String, String>) -> Result<(), Error> {
        let mut map = self.metadata.write().await;
        *map = metadata;
        Ok(())
    }

    async fn get(&self) -> Result<BTreeMap<String, String>, Error> {
        let map = self.metadata.read().await;
        Ok(map.clone())
    }
}
