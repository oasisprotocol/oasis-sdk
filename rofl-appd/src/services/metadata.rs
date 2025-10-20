use std::collections::BTreeMap;

use rocket::async_trait;
use tokio::sync::RwLock;

use rofl_app_core::prelude::*;

/// Namespace prefix for user-provided metadata keys in on-chain registration.
pub const METADATA_NAMESPACE: &str = "net.oasis.app";

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
}

impl<A: App> OasisMetadataService<A> {
    /// Create a new metadata service, initializing from existing on-chain registration.
    pub async fn new(env: Environment<A>) -> Result<Self, Error> {
        // TODO: Query metadata limits here.

        Ok(Self {
            env,
            metadata: RwLock::new(BTreeMap::new()),
        })
    }
}

#[async_trait]
impl<A: App> MetadataService for OasisMetadataService<A> {
    async fn set(&self, metadata: BTreeMap<String, String>) -> Result<(), Error> {
        // TODO: Validate values, and number of entries. Would need to add a query to fetch the configured limits on start.
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
