use std::collections::{BTreeMap, BTreeSet};

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

    /// Insert or update the given metadata key-value pairs.
    ///
    /// Existing keys not present in `metadata` are left untouched. Will trigger a
    /// registration refresh if the metadata has changed.
    async fn upsert(&self, metadata: BTreeMap<String, String>) -> Result<(), Error>;

    /// Delete the given metadata keys.
    ///
    /// Keys that do not exist are ignored. Will trigger a registration refresh if the
    /// metadata has changed.
    async fn delete(&self, keys: BTreeSet<String>) -> Result<(), Error>;

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

    /// Validate the given metadata map against the configured runtime limits.
    fn validate(&self, metadata: &BTreeMap<String, String>) -> Result<(), Error> {
        let max_user_pairs =
            (self.limits.max_pairs as usize).saturating_sub(RESERVED_METADATA_SLOTS);
        if metadata.len() > max_user_pairs {
            return Err(Error::InvalidArgument);
        }

        for (key, value) in metadata {
            // Account for namespace prefix when checking key size.
            let full_key_size = METADATA_NAMESPACE.len() + 1 + key.len();
            if full_key_size > self.limits.max_key_size as usize {
                return Err(Error::InvalidArgument);
            }
            if value.len() > self.limits.max_value_size as usize {
                return Err(Error::InvalidArgument);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<A: App> MetadataService for OasisMetadataService<A> {
    async fn set(&self, metadata: BTreeMap<String, String>) -> Result<(), Error> {
        self.validate(&metadata)?;

        let mut map = self.metadata.write().await;
        if *map == metadata {
            return Ok(());
        }
        *map = metadata;

        self.env.refresh_registration().await?;

        Ok(())
    }

    async fn upsert(&self, metadata: BTreeMap<String, String>) -> Result<(), Error> {
        let mut map = self.metadata.write().await;

        let mut updated = map.clone();
        let mut changed = false;
        for (key, value) in metadata {
            if updated.get(&key) != Some(&value) {
                updated.insert(key, value);
                changed = true;
            }
        }
        self.validate(&updated)?;

        if !changed {
            return Ok(());
        }
        *map = updated;

        self.env.refresh_registration().await?;

        Ok(())
    }

    async fn delete(&self, keys: BTreeSet<String>) -> Result<(), Error> {
        let mut map = self.metadata.write().await;

        let mut changed = false;
        for key in &keys {
            if map.remove(key).is_some() {
                changed = true;
            }
        }

        if !changed {
            return Ok(());
        }

        self.env.refresh_registration().await?;

        Ok(())
    }

    async fn get(&self) -> Result<BTreeMap<String, String>, Error> {
        let map = self.metadata.read().await;
        Ok(map.clone())
    }
}
