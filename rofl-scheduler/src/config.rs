use std::{collections::BTreeSet, sync::Arc};

use anyhow::{anyhow, Result};
use base64::prelude::*;

use oasis_runtime_sdk::{
    cbor,
    core::{common::crypto::signature::PublicKey, Protocol},
    types::address::Address,
};
use oasis_runtime_sdk_rofl_market as market;

/// Local configuration key that contains the ROFL scheduler configuration.
const ROFL_SCHEDULER_CONFIG_KEY: &str = "rofl_scheduler";

/// Raw local configuration as serialized.
///
/// Unknown fields are ignored so that configs still carrying the removed `allowed_creators` and
/// `allowed_artifacts` keys (now sourced from on-chain offer metadata) continue to load.
#[derive(Clone, Debug, Default, cbor::Decode)]
#[cbor(allow_unknown)]
pub struct RawLocalConfig {
    /// Address of the provider.
    pub provider_address: String,
    /// Offers that the scheduler should accept. If no offers are configured, all are accepted.
    ///
    /// Each entry is the value of the `net.oasis.scheduler.offer` metadata key. Per-offer access
    /// policy (allowed creators and artifacts) lives in the on-chain offer metadata, not here.
    pub offers: Vec<String>,
    /// Resource capacity.
    pub capacity: Resources,
    /// Internal on which the scheduler will do its processing (in seconds).
    pub processing_interval: Option<u64>,
    /// Interval on which the scheduler will claim payment for an instance (in hours).
    pub claim_payment_interval: Option<u64>,
    /// Timeout for pulling images during deployment (in seconds).
    pub deploy_pull_timeout: Option<u64>,
    /// A list of node addresses to transfer the instances from.
    pub transfer_instances_from: Vec<String>,
    /// Domain used to serve the scheduler API endpoint. If not set, the endpoint is disabled.
    pub api_domain: Option<String>,
    /// Lifetime of issued JWT tokens for API access (in seconds).
    pub api_token_lifetime: Option<u64>,
    /// Optional proxy configuration.
    pub proxy: Option<ProxyConfig>,
}

/// Proxy configuration.
#[derive(Clone, Debug, Default, cbor::Decode)]
pub struct ProxyConfig {
    /// Domain used as a base to forward to apps running in deployed machines. All subdomains
    /// should be redirected to the same address.
    pub domain: String,
    /// External IP address to use for incoming Wireguard connections.
    pub external_wireguard_address: String,
    /// External IP address to use for incoming HTTPS proxy connections.
    pub external_proxy_address: Option<String>,

    /// Optional handshake timeout (in seconds).
    pub timeout_handshake: Option<u64>,
    /// Optional connect timeout (in seconds).
    pub timeout_connect: Option<u64>,
    /// Optional maximum connection duration (in seconds).
    pub timeout_connection: Option<u64>,
    /// Optional read/write timeout (in seconds).
    pub timeout_rw: Option<u64>,

    /// Optional maximum connection limit.
    pub max_connections: Option<u64>,
}

impl RawLocalConfig {
    /// Create a new raw local config by parsing the CBOR provided by the host.
    pub fn new(host: Arc<Protocol>) -> Result<Self> {
        let cfg = host
            .get_host_info()
            .local_config
            .remove(ROFL_SCHEDULER_CONFIG_KEY)
            .map(cbor::from_value)
            .transpose()?
            .unwrap_or_default();
        Ok(cfg)
    }
}

/// Resources.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Decode)]
pub struct Resources {
    /// Amount of VM instances.
    pub instances: u16,
    /// Amount of memory in megabytes.
    pub memory: u64,
    /// Amount of vCPUs.
    pub cpus: u16,
    /// Amount of storage in megabytes.
    pub storage: u64,
}

impl Resources {
    /// Add instance resources to this resource descriptor and return the updated descriptor.
    pub fn add(&self, other: &market::types::Resources) -> Self {
        let mut resources = self.clone();
        resources.instances = resources.instances.saturating_add(1);
        resources.memory = resources.memory.saturating_add(other.memory);
        resources.cpus = resources.cpus.saturating_add(other.cpus);
        resources.storage = resources.storage.saturating_add(other.storage);
        resources
    }

    /// Whether the current resource set has enough resources to satisfy an allocation request.
    pub fn can_allocate(&self, other: &Self) -> bool {
        if other.instances > self.instances {
            return false;
        }
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
    ///
    /// Each entry is the value of the `net.oasis.scheduler.offer` metadata key. If the set is
    /// empty, all offers are accepted. Per-offer access policy (allowed creators and artifacts)
    /// lives in the on-chain offer metadata.
    pub offers: BTreeSet<String>,
    /// Resource capacity.
    pub capacity: Resources,
    /// Internal on which the scheduler will do its processing (in seconds).
    pub processing_interval_secs: u64,
    /// Interval on which the scheduler will claim payment for an instance (in seconds).
    pub claim_payment_interval_secs: u64,
    /// Timeout for pulling images during deployment (in seconds).
    pub deploy_pull_timeout: u64,
    /// A list of node addresses to transfer the instances from.
    pub transfer_instances_from: BTreeSet<PublicKey>,
    /// Domain used to serve the scheduler API endpoint. If not set, the endpoint is disabled.
    pub api_domain: Option<String>,
    /// Lifetime of issued JWT tokens for API access (in seconds).
    pub api_token_lifetime: u64,
    /// Optional proxy configuration.
    pub proxy: Option<ProxyConfig>,
}

impl LocalConfig {
    /// Read local configuration from host.
    pub fn from_host(host: Arc<Protocol>) -> Result<Self> {
        let cfg = RawLocalConfig::new(host)?;
        Self::from_raw(cfg)
    }

    /// Read given raw local configuration.
    pub fn from_raw(cfg: RawLocalConfig) -> Result<Self> {
        let provider_address = Address::from_bech32(&cfg.provider_address)
            .map_err(|_| anyhow!("bad provider address"))?;

        let offers = cfg.offers.into_iter().collect();

        let transfer_instances_from = cfg
            .transfer_instances_from
            .into_iter()
            .map(|raw| -> Result<PublicKey> {
                let raw = BASE64_STANDARD.decode(raw)?;
                if raw.len() != PublicKey::len() {
                    return Err(anyhow!("bad node identifier"));
                }
                Ok(raw.into())
            })
            .collect::<Result<BTreeSet<_>>>()
            .map_err(|_| anyhow!("bad transfer instances from value"))?;

        Ok(LocalConfig {
            provider_address,
            offers,
            capacity: cfg.capacity,
            processing_interval_secs: cfg.processing_interval.unwrap_or(3),
            claim_payment_interval_secs: cfg.claim_payment_interval.unwrap_or(24) * 3600,
            deploy_pull_timeout: cfg.deploy_pull_timeout.unwrap_or(60),
            transfer_instances_from,
            api_domain: cfg.api_domain,
            api_token_lifetime: cfg
                .api_token_lifetime
                .unwrap_or(6 * 3600) // Default to 6 hours.
                .clamp(60, 7 * 24 * 3600),
            proxy: cfg.proxy,
        })
    }

    /// Check whether the given node identifier is among the list of nodes to transfer instances from.
    pub fn should_transfer_instance_from(&self, node_id: &PublicKey) -> bool {
        self.transfer_instances_from.contains(node_id)
    }
}
