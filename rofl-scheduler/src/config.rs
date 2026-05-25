use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use base64::prelude::*;

use oasis_runtime_sdk::{
    cbor,
    core::{
        common::crypto::{hash::Hash, signature::PublicKey},
        Protocol,
    },
    types::address::Address,
};
use oasis_runtime_sdk_rofl_market as market;

/// Local configuration key that contains the ROFL scheduler configuration.
const ROFL_SCHEDULER_CONFIG_KEY: &str = "rofl_scheduler";

/// Raw per-offer configuration as serialized.
#[derive(Clone, Debug, Default, cbor::Decode)]
pub struct RawOfferConfig {
    /// Allowed instance creator addresses for this offer.
    ///
    /// When set, overrides the global `allowed_creators`. An empty list means all creators are
    /// allowed. When not set (`None`), the global `allowed_creators` is used as a fallback.
    pub allowed_creators: Option<Vec<String>>,
    /// Allowed artifact hashes for this offer.
    ///
    /// When set, overrides the global `allowed_artifacts` entirely. When not set (`None`), the
    /// global `allowed_artifacts` is used as a fallback.
    pub allowed_artifacts: Option<BTreeMap<String, Vec<String>>>,
}

/// A map entry with an explicit `id` field and optional per-offer config overrides.
#[derive(Clone, Debug, Default, cbor::Decode)]
struct RawOfferEntry {
    pub id: String,
    pub allowed_creators: Option<Vec<String>>,
    pub allowed_artifacts: Option<BTreeMap<String, Vec<String>>>,
}

/// Backwards-compatible wrapper for the `offers` field in [`RawLocalConfig`].
///
/// Each element of the sequence is either a plain string (old format, no overrides) or a
/// map with an `id` field and optional per-offer config overrides:
///
/// ```yaml
/// offers:
///   - playground_short             # plain string — global defaults apply
///   - id: oasis_internal           # map entry — per-offer overrides
///     allowed_creators:
///       - "oasis1..."
/// ```
#[derive(Clone, Debug, Default)]
pub(crate) struct RawOffersField(BTreeMap<String, RawOfferConfig>);

impl cbor::Decode for RawOffersField {
    fn try_default() -> Result<Self, cbor::DecodeError> {
        Ok(Self(BTreeMap::new()))
    }

    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        let cbor::Value::Array(items) = value else {
            return Err(cbor::DecodeError::UnexpectedType);
        };
        let mut map = BTreeMap::new();
        for item in items {
            match item {
                // Plain string: "offer-name" → empty config (global defaults apply).
                cbor::Value::TextString(key) => {
                    map.insert(key, RawOfferConfig::default());
                }
                // Map with an `id` field and optional overrides.
                cbor::Value::Map(_) => {
                    let entry = RawOfferEntry::try_from_cbor_value(item)?;
                    map.insert(
                        entry.id,
                        RawOfferConfig {
                            allowed_creators: entry.allowed_creators,
                            allowed_artifacts: entry.allowed_artifacts,
                        },
                    );
                }
                _ => return Err(cbor::DecodeError::UnexpectedType),
            }
        }
        Ok(Self(map))
    }
}

/// Raw local configuration as serialized.
#[derive(Clone, Debug, Default, cbor::Decode)]
pub struct RawLocalConfig {
    /// Address of the provider.
    pub provider_address: String,
    /// Offers that the scheduler should accept. If no offers are configured, all are accepted.
    ///
    /// Each key is the value of the `net.oasis.scheduler.offer` metadata key. The value is an
    /// optional per-offer configuration that overrides the global defaults.
    ///
    /// Accepts the legacy plain-string array form (backwards compatible) or the new map-entry form.
    pub(crate) offers: RawOffersField,
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

/// Per-offer configuration.
#[derive(Clone, Debug, Default)]
pub struct OfferConfig {
    /// Allowed instance creator addresses.
    ///
    /// When `Some`, overrides the global `allowed_creators`. An empty set means all creators are
    /// allowed. When `None`, the global `allowed_creators` is used as a fallback.
    pub allowed_creators: Option<BTreeSet<Address>>,
    /// Allowed artifact hashes.
    ///
    /// When `Some`, overrides the global `allowed_artifacts` entirely. When `None`, the global
    /// `allowed_artifacts` is used as a fallback.
    pub allowed_artifacts: Option<BTreeMap<String, BTreeSet<Hash>>>,
}

/// Local scheduler configuration.
#[derive(Clone, Debug, Default)]
pub struct LocalConfig {
    /// Address of the provider.
    pub provider_address: Address,
    /// Offers that the scheduler should accept, with optional per-offer configuration.
    ///
    /// Each key is the value of the `net.oasis.scheduler.offer` metadata key. If the map is
    /// empty, all offers are accepted using the global defaults.
    pub offers: BTreeMap<String, OfferConfig>,
    /// Allowed artifact hashes (global default, may be overridden per offer).
    pub allowed_artifacts: BTreeMap<String, BTreeSet<Hash>>,
    /// Allowed instance creator addresses (global default, may be overridden per offer).
    pub allowed_creators: BTreeSet<Address>,
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
        let allowed_artifacts = cfg
            .allowed_artifacts
            .into_iter()
            .map(|(kind, hashes)| -> Result<(String, BTreeSet<Hash>)> {
                Ok((
                    kind,
                    hashes
                        .into_iter()
                        .map(|h| -> Result<Hash> { Ok(h.parse::<Hash>()?) })
                        .collect::<Result<BTreeSet<_>>>()?,
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

        let offers = cfg
            .offers
            .0
            .into_iter()
            .map(|(key, raw_cfg)| -> Result<(String, OfferConfig)> {
                let offer_creators = raw_cfg
                    .allowed_creators
                    .map(|creators| {
                        creators
                            .into_iter()
                            .map(|raw| Address::from_bech32(&raw))
                            .collect::<Result<BTreeSet<_>, _>>()
                            .map_err(|_| anyhow!("bad allowed_creators in offer '{key}'"))
                    })
                    .transpose()?;
                let offer_artifacts = raw_cfg
                    .allowed_artifacts
                    .map(|artifacts| {
                        artifacts
                            .into_iter()
                            .map(|(kind, hashes)| -> Result<(String, BTreeSet<Hash>)> {
                                Ok((
                                    kind,
                                    hashes
                                        .into_iter()
                                        .map(|h| -> Result<Hash> { Ok(h.parse::<Hash>()?) })
                                        .collect::<Result<BTreeSet<_>>>()?,
                                ))
                            })
                            .collect::<Result<_>>()
                            .map_err(|_| anyhow!("bad allowed_artifacts in offer '{key}'"))
                    })
                    .transpose()?;
                Ok((
                    key,
                    OfferConfig {
                        allowed_creators: offer_creators,
                        allowed_artifacts: offer_artifacts,
                    },
                ))
            })
            .collect::<Result<_>>()?;

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
            allowed_artifacts,
            allowed_creators,
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

    /// Validate the given artifact hash against the set of allowed artifacts.
    ///
    /// Uses the per-offer artifact list when the offer has one configured; otherwise falls back
    /// to the global `allowed_artifacts`.
    pub fn ensure_artifact_allowed(&self, offer_key: &str, kind: &str, hash: &Hash) -> Result<()> {
        let artifacts = self
            .offers
            .get(offer_key)
            .and_then(|cfg| cfg.allowed_artifacts.as_ref())
            .unwrap_or(&self.allowed_artifacts);

        match artifacts.get(kind) {
            None => Ok(()), // all artifacts of this kind are allowed
            Some(allowed_hashes) => {
                if !allowed_hashes.contains(hash) {
                    Err(anyhow!("{kind} artifact not allowed"))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Check whether the given creator is among the allowed creators for the given offer.
    ///
    /// Uses the per-offer creator list when the offer has one configured; otherwise falls back to
    /// the global `allowed_creators`. An empty list means all creators are allowed.
    pub fn is_creator_allowed(&self, offer_key: &str, address: &Address) -> bool {
        if let Some(offer_cfg) = self.offers.get(offer_key) {
            if let Some(ref creators) = offer_cfg.allowed_creators {
                return creators.is_empty() || creators.contains(address);
            }
        }
        self.allowed_creators.is_empty() || self.allowed_creators.contains(address)
    }

    /// Check whether the given node identifier is among the list of nodes to transfer instances from.
    pub fn should_transfer_instance_from(&self, node_id: &PublicKey) -> bool {
        self.transfer_instances_from.contains(node_id)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_raw(offers_cbor: cbor::Value) -> RawLocalConfig {
        let mut raw = RawLocalConfig::default();
        raw.offers = cbor::Decode::try_from_cbor_value(offers_cbor).unwrap();
        raw
    }

    #[test]
    fn test_offers_legacy_array_format() {
        // Old format: plain array of strings. Each becomes a key with empty OfferConfig.
        let raw = make_raw(cbor::Value::Array(vec![
            cbor::Value::TextString("playground_short".into()),
            cbor::Value::TextString("playground_short_sgx".into()),
        ]));
        assert!(raw.offers.0.contains_key("playground_short"));
        assert!(raw.offers.0.contains_key("playground_short_sgx"));
        assert_eq!(raw.offers.0.len(), 2);
        // No per-offer overrides — both fields are None (global fallback applies).
        assert!(raw.offers.0["playground_short"].allowed_creators.is_none());
    }

    #[test]
    fn test_offers_mixed_format() {
        // Mixed: plain strings alongside a map entry with an explicit `id` field and overrides.
        let raw = make_raw(cbor::Value::Array(vec![
            cbor::Value::TextString("playground_short".into()),
            cbor::Value::TextString("playground_short_sgx".into()),
            cbor::Value::Map(vec![
                (
                    cbor::Value::TextString("id".into()),
                    cbor::Value::TextString("oasis_internal".into()),
                ),
                (
                    cbor::Value::TextString("allowed_creators".into()),
                    cbor::Value::Array(vec![cbor::Value::TextString(
                        "oasis1qp0cnmkjl22gky6p7q0tgkwmsc6g4c5er6x0hsk7".into(),
                    )]),
                ),
            ]),
        ]));
        assert!(raw.offers.0.contains_key("playground_short"));
        assert!(raw.offers.0["playground_short"].allowed_creators.is_none());
        assert!(raw.offers.0.contains_key("oasis_internal"));
        assert!(raw.offers.0["oasis_internal"].allowed_creators.is_some());
    }

    #[test]
    fn test_is_creator_allowed_fallback() {
        // Per-offer None → global fallback.
        let cfg = LocalConfig {
            offers: BTreeMap::from([("public".into(), OfferConfig::default())]),
            allowed_creators: BTreeSet::new(), // global: allow all
            ..Default::default()
        };
        let addr = Address::from_bech32("oasis1qp0cnmkjl22gky6p7q0tgkwmsc6g4c5er6x0hsk7").unwrap();
        assert!(cfg.is_creator_allowed("public", &addr));
    }

    #[test]
    fn test_is_creator_allowed_per_offer_override() {
        let allowed =
            Address::from_bech32("oasis1qp0cnmkjl22gky6p7q0tgkwmsc6g4c5er6x0hsk7").unwrap();
        let blocked =
            Address::from_bech32("oasis1qrad7s7nqm4gvyzr8yt48jkrjxuqc6d7pvjm4ze").unwrap();

        let cfg = LocalConfig {
            offers: BTreeMap::from([(
                "internal".into(),
                OfferConfig {
                    allowed_creators: Some(BTreeSet::from([allowed])),
                    allowed_artifacts: None,
                },
            )]),
            allowed_creators: BTreeSet::new(), // global: allow all (should not apply here)
            ..Default::default()
        };

        assert!(cfg.is_creator_allowed("internal", &allowed));
        assert!(!cfg.is_creator_allowed("internal", &blocked));
        // Unknown offer key → global fallback (allow all).
        assert!(cfg.is_creator_allowed("public", &blocked));
    }
}
