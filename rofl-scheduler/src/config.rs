use std::collections::BTreeMap;

use anyhow::{anyhow, Result};

use oasis_runtime_sdk::{
    cbor, core::common::crypto::hash::Hash, modules::rofl::app::prelude::*, types::address::Address,
};

use crate::SchedulerApp;

/// Local configuration key that contains the ROFL scheduler configuration.
const ROFL_SCHEDULER_CONFIG_KEY: &str = "rofl_scheduler";

/// Raw local configuration as serialized.
#[derive(Clone, Debug, Default, cbor::Decode)]
struct RawLocalConfig {
    /// Address of the provider.
    pub provider_address: String,
    /// Offers that the scheduler should accept.
    pub offers: Vec<String>,
    /// Allowed artifact hashes.
    ///
    /// Key is the artifact kind and value is a list of artifact SHA256 hashes. If a key doesn't
    /// exist, all artifacts are allowed.
    pub allowed_artifacts: BTreeMap<String, Vec<String>>,
}

/// Local scheduler configuration.
#[derive(Clone, Debug, Default)]
pub struct LocalConfig {
    /// Address of the provider.
    pub provider_address: Address,
    /// Offers that the scheduler should accept.
    pub offers: Vec<String>,
    /// Allowed artifact hashes.
    pub allowed_artifacts: BTreeMap<String, Vec<Hash>>,
}

impl LocalConfig {
    /// Read local configuration.
    pub fn from_env(env: Environment<SchedulerApp>) -> Result<Self> {
        let cfg: RawLocalConfig = env
            .untrusted_local_config()
            .remove(ROFL_SCHEDULER_CONFIG_KEY)
            .map(cbor::from_value)
            .transpose()?
            .unwrap_or_default();

        let provider_address = Address::from_bech32(&cfg.provider_address)
            .map_err(|_| anyhow!("bad provider address"))?;
        let allowed_artifacts = cfg
            .allowed_artifacts
            .into_iter()
            .map(|(kind, hashes)| -> Result<(String, Vec<Hash>)> {
                Ok((
                    kind,
                    hashes
                        .into_iter()
                        .map(|h| -> Result<Hash> { Ok(h.parse::<Hash>()?) })
                        .collect::<Result<Vec<_>>>()?,
                ))
            })
            .collect::<Result<_>>()
            .map_err(|_| anyhow!("bad allowed artifacts value"))?;

        Ok(LocalConfig {
            provider_address,
            offers: cfg.offers,
            allowed_artifacts,
        })
    }
}
