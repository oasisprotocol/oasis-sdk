use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use oasis_runtime_sdk::{
    cbor, core::common::crypto::hash::Hash, modules::rofl::app::prelude::*, types::address::Address,
};
use oasis_runtime_sdk_rofl_market as market;

use crate::SchedulerApp;

/// Local configuration key that contains the ROFL scheduler configuration.
const ROFL_SCHEDULER_CONFIG_KEY: &str = "rofl_scheduler";

/// Raw local configuration as serialized.
#[derive(Clone, Debug, Default, cbor::Decode)]
struct RawLocalConfig {
    /// Address of the provider.
    pub provider_address: String,
    /// Offers that the scheduler should accept.
    ///
    /// Each offer identifier is the value of the `net.oasis.scheduler.offer` metadata key.
    pub offers: BTreeSet<String>,
    /// Allowed artifact hashes.
    ///
    /// Key is the artifact kind and value is a list of artifact SHA256 hashes. If a key doesn't
    /// exist, all artifacts are allowed.
    pub allowed_artifacts: BTreeMap<String, Vec<String>>,
    /// Allowed instance creator addresses.
    ///
    /// If empty, any address is allowed.
    pub allowed_creators: Vec<String>,
    /// Resource capacity.
    pub capacity: Resources,
}

/// Resources.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Decode)]
pub struct Resources {
    /// Amount of memory in megabytes.
    pub memory: u64,
    /// Amount of vCPUs.
    pub cpus: u16,
    /// Amount of storage in megabytes.
    pub storage: u64,
}

impl Resources {
    /// Add instance resources to this resource descriptor.
    pub fn add(&mut self, other: &market::types::Resources) {
        self.memory = self.memory.saturating_add(other.memory);
        self.cpus = self.cpus.saturating_add(other.cpus);
        self.storage = self.storage.saturating_add(other.storage);
    }

    /// Whether the current resource set has enough resources to satisfy an allocation request.
    pub fn can_allocate(&self, other: &Self) -> bool {
        if other.memory > self.memory {
            return false;
        }
        if other.cpus > self.cpus {
            return false;
        }
        if other.storage > self.storage {
            return false;
        }
        true
    }
}

/// Local scheduler configuration.
#[derive(Clone, Debug, Default)]
pub struct LocalConfig {
    /// Address of the provider.
    pub provider_address: Address,
    /// Offers that the scheduler should accept.
    pub offers: BTreeSet<String>,
    /// Allowed artifact hashes.
    pub allowed_artifacts: BTreeMap<String, Vec<Hash>>,
    /// Allowed instance creator addresses.
    pub allowed_creators: BTreeSet<Address>,
    /// Resource capacity.
    pub capacity: Resources,
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
        let allowed_creators = cfg
            .allowed_creators
            .into_iter()
            .map(|raw| Address::from_bech32(&raw))
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|_| anyhow!("bad allowed creators value"))?;

        Ok(LocalConfig {
            provider_address,
            offers: cfg.offers,
            allowed_artifacts,
            allowed_creators,
            capacity: cfg.capacity,
        })
    }
}
