mod domain;

use std::{collections::BTreeMap, time::Duration};

use anyhow::{anyhow, Result};

use oasis_runtime_sdk::core::common::{crypto::x25519, logger::get_logger};
use oasis_runtime_sdk_rofl_market::types::{Deployment, Instance, InstanceId};
use rofl_app_core::prelude::*;
use rofl_proxy::{http, wireguard, HttpConfig, ProxyLabel};
use tokio::sync::Mutex;

use self::domain::{CancelVerificationsOnDrop, CustomDomainVerifier};
use crate::{
    config::ProxyConfig,
    proxy::domain::CustomDomainVerificationNotifier,
    types::{
        domain_verification_token, METADATA_KEY_PROXY_CUSTOM_DOMAINS, METADATA_KEY_PROXY_PROTOCOL,
        METADATA_KEY_PROXY_TIMEOUT_CONNECTION, METADATA_KEY_PROXY_TIMEOUT_RW,
        METADATA_VALUE_PROXY_PROTOCOL_V2,
    },
};

/// Maximum number of custom domains that can be configured at the same time by the same
/// deployment.
const MAX_CUSTOM_DOMAINS: usize = 3;

struct InstanceInfo {
    wireguard_pk: x25519::PublicKey,
    http_host: String,
    dst_address: String,
    dst_port: u16,
    proxy_protocol: bool,
    timeouts: http::Timeouts,
    /// Connection group shared by all mappings of the instance.
    connection_group: http::ConnectionGroup,
    custom_domains: Vec<String>,
    #[allow(dead_code)]
    domain_verification_handle: Arc<CancelVerificationsOnDrop>,
}

/// Range of timeout values that deployments may request.
#[derive(Clone, Copy, Debug)]
struct TimeoutBounds {
    min: Duration,
    max: Duration,
}

impl TimeoutBounds {
    /// Create bounds from the proxy default and an optional maximum (in seconds).
    fn new(default: Duration, max: Option<u64>) -> Self {
        let max = max.map_or(default, Duration::from_secs);
        Self {
            min: default,
            max: max.max(default),
        }
    }

    fn clamp(&self, timeout: Duration) -> Duration {
        timeout.clamp(self.min, self.max)
    }
}

/// Policy for proxy timeouts that deployments may request.
#[derive(Clone, Copy, Debug)]
struct TimeoutPolicy {
    rw: TimeoutBounds,
    connection: TimeoutBounds,
}

impl TimeoutPolicy {
    fn new(cfg: &ProxyConfig, http_cfg: &http::Config) -> Self {
        Self {
            rw: TimeoutBounds::new(http_cfg.timeout_rw, cfg.max_timeout_rw),
            connection: TimeoutBounds::new(http_cfg.timeout_connection, cfg.max_timeout_connection),
        }
    }

    /// Proxy timeouts requested by the deployment, clamped to the policy bounds.
    ///
    /// Deployments can only raise timeouts as the proxy inside the instance enforces the exact
    /// per-port values.
    fn timeouts(&self, deployment: &Deployment) -> http::Timeouts {
        let requested = |key: &str, bounds: &TimeoutBounds| {
            deployment
                .metadata
                .get(key)
                .and_then(|value| value.parse::<u64>().ok())
                .map(|secs| bounds.clamp(Duration::from_secs(secs)))
        };

        http::Timeouts {
            rw: requested(METADATA_KEY_PROXY_TIMEOUT_RW, &self.rw),
            connection: requested(METADATA_KEY_PROXY_TIMEOUT_CONNECTION, &self.connection),
            ..Default::default()
        }
    }
}

/// Proxy for apps to expose services over.
pub struct Proxy {
    wireguard: wireguard::Hub,
    http: http::Proxy,
    domain: String,
    external_address: Option<String>,
    timeout_policy: TimeoutPolicy,
    max_instance_connections: Option<usize>,
    logger: slog::Logger,
    instances: Arc<Mutex<BTreeMap<InstanceId, InstanceInfo>>>,
    domain_verifier: CustomDomainVerifier,
}

impl Proxy {
    /// Create a new app proxy.
    pub fn new(
        cfg: &ProxyConfig,
        listen_port: u16,
        acme: rofl_proxy::http::tls::AcmeAccount,
    ) -> Result<Self> {
        let logger = get_logger("scheduler/proxy");

        let wireguard = wireguard::Hub::new(wireguard::HubConfig {
            external_address: cfg.external_wireguard_address.clone(),
            external_port: wireguard::WG_DEFAULT_LISTEN_PORT,
        })?;

        let mut http_cfg = http::Config {
            listen_port,
            ..Default::default()
        };
        if let Some(timeout) = cfg.timeout_handshake {
            http_cfg.timeout_handshake = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_connect {
            http_cfg.timeout_connect = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_connection {
            http_cfg.timeout_connection = Duration::from_secs(timeout);
        }
        if let Some(timeout) = cfg.timeout_rw {
            http_cfg.timeout_rw = Duration::from_secs(timeout);
        }
        if let Some(max_connections) = cfg.max_connections {
            http_cfg.max_connections = max_connections as usize;
        }
        let timeout_policy = TimeoutPolicy::new(cfg, &http_cfg);
        let max_instance_connections = cfg
            .max_instance_connections
            .filter(|&max| max > 0)
            .map(|max| max as usize);
        let http = http::Proxy::new(http_cfg, acme)?;

        let instances = Arc::new(Mutex::new(BTreeMap::new()));
        let notifier = Arc::new(ProxyDomainVerificationHandler::new(
            http.handle().clone(),
            instances.clone(),
        ));

        Ok(Self {
            wireguard,
            http,
            domain: cfg.domain.clone(),
            external_address: cfg.external_proxy_address.clone(),
            timeout_policy,
            max_instance_connections,
            logger: logger.clone(),
            instances,
            domain_verifier: CustomDomainVerifier::new(8, notifier, logger),
        })
    }

    /// Start the app proxy services.
    pub fn start(&mut self) {
        slog::info!(self.logger, "starting proxy");

        self.http.start();
        self.domain_verifier.start();

        // Spawn task that periodically dumps Wireguard interface status.
        let logger = self.logger.clone();
        let wireguard = self.wireguard.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                match wireguard.current_status() {
                    Ok(status) => {
                        for (key, peer) in status.peers {
                            slog::info!(logger, "wireguard peer status";
                               "peer" => ?key,
                               "endpoint" => ?peer.endpoint,
                               "last_handshake" => ?peer.last_handshake,
                               "tx_bytes" => peer.tx_bytes,
                               "rx_bytes" => peer.rx_bytes,
                               "allowed_ips" => ?peer.allowed_ips,
                            );
                        }
                    }
                    Err(err) => {
                        slog::warn!(logger, "failed to get wireguard status"; "err" => ?err)
                    }
                }
            }
        });
    }

    /// Add a static HTTP proxy mapping.
    pub async fn add_static_mapping(&self, mapping: http::Mapping) {
        self.http.add_mapping(mapping).await;
    }

    /// Generate a fresh key pair and assign an IP address for the instance.
    ///
    /// Returns the proxy label that should be set on the deployed instance, encrypted to the
    /// app's SEK.
    pub async fn provision_instance(
        &self,
        instance: &Instance,
        deployment: &Deployment,
    ) -> Result<ProxyLabel> {
        let mut instances = self.instances.lock().await;

        // If the instance already exists, first tear down the existing instance.
        if let Some(info) = instances.remove(&instance.id) {
            self.teardown_instance(info).await?;
        }

        // Generate HTTP host.
        let short_id: u64 = instance.id.into();
        let http_host = format!("m{}.{}", short_id, self.domain);

        let wireguard = self.wireguard.provision_client()?;
        let dst_address = wireguard
            .address
            .split("/")
            .next()
            .ok_or(anyhow!("bad proxy listen address"))?
            .to_string();

        // Check if the deployment expects a PROXY protocol header.
        let proxy_protocol = deployment
            .metadata
            .get(METADATA_KEY_PROXY_PROTOCOL)
            .is_some_and(|v| v == METADATA_VALUE_PROXY_PROTOCOL_V2);

        let timeouts = self.timeout_policy.timeouts(deployment);
        let connection_group = http::ConnectionGroup::new(self.max_instance_connections);

        self.http
            .add_mapping(http::Mapping {
                name: http_host.clone(),
                dst_address: dst_address.clone(),
                dst_port: 443,
                mode: http::Mode::ForwardOnly,
                proxy_protocol,
                timeouts,
                connection_group: Some(connection_group.clone()),
            })
            .await;

        let domain_verification_handle = CancelVerificationsOnDrop::new();

        instances.insert(
            instance.id,
            InstanceInfo {
                wireguard_pk: wireguard.sk.public_key(),
                http_host: http_host.clone(),
                dst_address: dst_address.clone(),
                dst_port: 443,
                proxy_protocol,
                timeouts,
                connection_group,
                custom_domains: Default::default(),
                domain_verification_handle: domain_verification_handle.clone(),
            },
        );

        // Check if any custom domains have been configured and queue verification.
        let custom_domains = Self::extract_custom_domains(deployment);
        for domain in &custom_domains {
            let token = domain_verification_token(instance, deployment, domain);

            if let Err(err) = self
                .domain_verifier
                .queue_verification(instance.id, domain, &token, &domain_verification_handle)
                .await
            {
                slog::error!(self.logger, "failed to queue domain verification";
                    "id" => ?instance.id,
                    "domain" => domain,
                    "err" => ?err
                );
            }
        }

        slog::info!(self.logger, "provisioned keys for instance";
            "id" => ?instance.id,
            "host" => &http_host,
            "address" => &dst_address,
            "timeouts" => ?timeouts,
        );

        let label = ProxyLabel {
            wireguard,
            http: HttpConfig {
                host: http_host,
                external_address: self.external_address.clone(),
                proxy_protocol,
            },
        };

        Ok(label)
    }

    /// Extract custom domains from deployment metadata.
    fn extract_custom_domains(deployment: &Deployment) -> Vec<String> {
        let empty = String::new();
        deployment
            .metadata
            .get(METADATA_KEY_PROXY_CUSTOM_DOMAINS)
            .unwrap_or(&empty)
            .split(' ')
            .filter(|s| !s.is_empty())
            .take(MAX_CUSTOM_DOMAINS)
            .map(|s| s.to_string())
            .collect()
    }

    /// Deprovision the key pair associated with the given instance.
    pub async fn deprovision_instance(&self, id: InstanceId) -> Result<()> {
        let mut instances = self.instances.lock().await;
        if let Some(info) = instances.remove(&id) {
            self.teardown_instance(info).await?;

            slog::info!(self.logger, "deprovisioned keys for instance";
                "id" => ?id,
            );
        }
        Ok(())
    }

    /// Remove all proxy state of an instance and close its existing connections.
    async fn teardown_instance(&self, info: InstanceInfo) -> Result<()> {
        // Remove mappings first so that new connections cannot join the closed group.
        self.http.remove_mapping(&info.http_host).await;
        for domain in &info.custom_domains {
            self.http.remove_mapping(domain).await;
        }
        info.connection_group.close();
        self.wireguard.deprovision_client(&info.wireguard_pk)?;
        Ok(())
    }
}

/// Handler for domain verification events.
struct ProxyDomainVerificationHandler {
    http: http::ProxyHandle,
    instances: Arc<Mutex<BTreeMap<InstanceId, InstanceInfo>>>,
}

impl ProxyDomainVerificationHandler {
    /// Creates a new instance of `ProxyDomainVerificationHandler`.
    pub fn new(
        http: http::ProxyHandle,
        instances: Arc<Mutex<BTreeMap<InstanceId, InstanceInfo>>>,
    ) -> Self {
        Self { http, instances }
    }
}

#[async_trait]
impl CustomDomainVerificationNotifier for ProxyDomainVerificationHandler {
    async fn verification_completed(&self, id: InstanceId, domain: &str) {
        let mut instances = self.instances.lock().await;
        if let Some(instance) = instances.get_mut(&id) {
            instance.custom_domains.push(domain.to_string());
            self.http
                .add_mapping(http::Mapping {
                    name: domain.to_string(),
                    dst_address: instance.dst_address.clone(),
                    dst_port: instance.dst_port,
                    mode: http::Mode::ForwardOnly,
                    proxy_protocol: instance.proxy_protocol,
                    timeouts: instance.timeouts,
                    connection_group: Some(instance.connection_group.clone()),
                })
                .await;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn deployment(metadata: &[(&str, &str)]) -> Deployment {
        Deployment {
            metadata: metadata
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_timeout_policy() {
        let http_cfg = http::Config {
            timeout_rw: Duration::from_secs(30),
            timeout_connection: Duration::from_secs(45),
            ..Default::default()
        };
        let secs = |secs| Some(Duration::from_secs(secs));

        // Without configured maximums, deployments cannot raise timeouts.
        let policy = TimeoutPolicy::new(&ProxyConfig::default(), &http_cfg);
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "300"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "3600"),
        ]));
        assert_eq!(timeouts.rw, secs(30));
        assert_eq!(timeouts.connection, secs(45));

        let cfg = ProxyConfig {
            max_timeout_rw: Some(600),
            max_timeout_connection: Some(1800),
            ..Default::default()
        };
        let policy = TimeoutPolicy::new(&cfg, &http_cfg);

        // Missing or malformed requests should use the defaults.
        assert_eq!(policy.timeouts(&deployment(&[])), http::Timeouts::default());
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "invalid"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "-1"),
        ]));
        assert_eq!(timeouts, http::Timeouts::default());

        // Requests within the bounds should be honored.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "300"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "900"),
        ]));
        assert_eq!(timeouts.rw, secs(300));
        assert_eq!(timeouts.connection, secs(900));
        assert_eq!(timeouts.connect, None);

        // Requests above the maximum should be clamped.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "100000"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "100000"),
        ]));
        assert_eq!(timeouts.rw, secs(600));
        assert_eq!(timeouts.connection, secs(1800));

        // Requests cannot lower the timeouts below the defaults.
        let timeouts = policy.timeouts(&deployment(&[
            (METADATA_KEY_PROXY_TIMEOUT_RW, "1"),
            (METADATA_KEY_PROXY_TIMEOUT_CONNECTION, "0"),
        ]));
        assert_eq!(timeouts.rw, secs(30));
        assert_eq!(timeouts.connection, secs(45));

        // Maximums below the defaults should not lower the defaults.
        let cfg = ProxyConfig {
            max_timeout_rw: Some(10),
            ..Default::default()
        };
        let policy = TimeoutPolicy::new(&cfg, &http_cfg);
        let timeouts = policy.timeouts(&deployment(&[(METADATA_KEY_PROXY_TIMEOUT_RW, "300")]));
        assert_eq!(timeouts.rw, secs(30));
    }
}
