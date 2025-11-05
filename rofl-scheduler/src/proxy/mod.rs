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
    types::{domain_verification_token, METADATA_KEY_PROXY_CUSTOM_DOMAINS},
};

/// Maximum number of custom domains that can be configured at the same time by the same
/// deployment.
const MAX_CUSTOM_DOMAINS: usize = 3;

struct InstanceInfo {
    wireguard_pk: x25519::PublicKey,
    http_host: String,
    dst_address: String,
    dst_port: u16,
    custom_domains: Vec<String>,
    #[allow(dead_code)]
    domain_verification_handle: Arc<CancelVerificationsOnDrop>,
}

/// Proxy for apps to expose services over.
pub struct Proxy {
    wireguard: wireguard::Hub,
    http: http::Proxy,
    domain: String,
    external_address: Option<String>,
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

        // If the instance already exists, first deprovision the existing instance.
        let existing = instances.remove(&instance.id);
        if let Some(info) = existing {
            self.http.remove_mapping(&info.http_host).await;
            self.wireguard.deprovision_client(&info.wireguard_pk)?;
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

        self.http
            .add_mapping(http::Mapping {
                name: http_host.clone(),
                dst_address: dst_address.clone(),
                dst_port: 443,
                mode: http::Mode::ForwardOnly,
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
        );

        let label = ProxyLabel {
            wireguard,
            http: HttpConfig {
                host: http_host,
                external_address: self.external_address.clone(),
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
            self.http.remove_mapping(&info.http_host).await;
            for domain in info.custom_domains {
                self.http.remove_mapping(&domain).await;
            }
            self.wireguard.deprovision_client(&info.wireguard_pk)?;

            slog::info!(self.logger, "deprovisioned keys for instance";
                "id" => ?id,
            );
        }
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
                })
                .await;
        }
    }
}
