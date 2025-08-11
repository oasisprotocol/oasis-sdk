use std::{collections::BTreeMap, sync::Mutex, time::Duration};

use anyhow::{anyhow, Result};

use oasis_runtime_sdk::core::common::{crypto::x25519, logger::get_logger};
use oasis_runtime_sdk_rofl_market::types::InstanceId;
use rofl_app_core::prelude::*;
use rofl_proxy::{http, wireguard, HttpConfig, ProxyLabel};

use crate::config::ProxyConfig;

struct InstanceInfo {
    wireguard_pk: x25519::PublicKey,
    http_host: String,
}

/// Proxy for apps to expose services over.
pub struct Proxy {
    wireguard: wireguard::Hub,
    http: http::Proxy,
    domain: String,
    external_address: Option<String>,
    logger: slog::Logger,
    instances: Mutex<BTreeMap<InstanceId, InstanceInfo>>,
}

impl Proxy {
    /// Create a new app proxy.
    pub fn new(cfg: &ProxyConfig, listen_port: u16) -> Result<Self> {
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
        let http = http::Proxy::new(http_cfg)?;

        Ok(Self {
            wireguard,
            http,
            domain: cfg.domain.clone(),
            external_address: cfg.external_proxy_address.clone(),
            logger,
            instances: Mutex::new(BTreeMap::new()),
        })
    }

    /// Start the app proxy services.
    pub fn start(&mut self) {
        slog::info!(self.logger, "starting proxy");

        self.http.start();
    }

    /// Add a static HTTP proxy mapping.
    pub async fn add_static_mapping(&self, mapping: http::Mapping) {
        self.http.add_mapping(mapping).await;
    }

    /// Generate a fresh key pair and assign an IP address for the instance.
    ///
    /// Returns the proxy label that should be set on the deployed instance, encrypted to the
    /// app's SEK.
    pub async fn provision_instance(&self, id: InstanceId) -> Result<ProxyLabel> {
        // If the instance already exists, first deprovision the existing instance.
        let existing = { self.instances.lock().unwrap().remove(&id) };
        if let Some(info) = existing {
            self.http.remove_mapping(&info.http_host).await;
            self.wireguard.deprovision_client(&info.wireguard_pk)?;
        }

        // Generate HTTP host.
        let short_id: u64 = id.into();
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
                dst_address,
                dst_port: 443,
                mode: http::Mode::ForwardOnly,
            })
            .await;

        self.instances.lock().unwrap().insert(
            id,
            InstanceInfo {
                wireguard_pk: wireguard.sk.public_key(),
                http_host: http_host.clone(),
            },
        );

        slog::info!(self.logger, "provisioned keys for instance";
            "id" => ?id,
            "host" => &http_host,
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

    /// Deprovision the key pair associated with the given instance.
    pub async fn deprovision_instance(&self, id: InstanceId) -> Result<()> {
        let existing = { self.instances.lock().unwrap().remove(&id) };
        if let Some(info) = existing {
            self.http.remove_mapping(&info.http_host).await;
            self.wireguard.deprovision_client(&info.wireguard_pk)?;

            slog::info!(self.logger, "deprovisioned keys for instance";
                "id" => ?id,
            );
        }
        Ok(())
    }
}
