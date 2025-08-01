use std::{fs, sync::Arc};

use anyhow::{anyhow, Context, Result};
use base64::prelude::*;

use oasis_runtime_sdk::core::{
    common::logger::get_logger,
    host::attestation::{AttestLabelsRequest, LabelAttestation},
};
use regex::Regex;
use rofl_app_core::prelude::*;
use rofl_appd::services;
use rofl_proxy::{http, wireguard, ProxyLabel, LABEL_PROXY, PROXY_LABEL_ENCRYPTION_CONTEXT};
use yaml_rust2::{Yaml, YamlLoader};

use crate::{App, Environment};

/// Location of the compose file.
const COMPOSE_FILE_PATH: &str = "/etc/oasis/containers/compose.yaml";

/// Initialize the proxy.
pub(crate) async fn init<A: App>(env: Environment<A>, kms: Arc<dyn services::kms::KmsService>) {
    let logger = get_logger("proxy");

    if let Err(err) = maybe_start(env, kms).await {
        slog::error!(logger, "failed to start proxy"; "err" => ?err);
    }
}

async fn maybe_start<A: App>(
    env: Environment<A>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("proxy");

    // Fetch proxy configuration and only start the proxy if available.
    let rsp = env
        .host()
        .attestation()
        .attest_labels(AttestLabelsRequest {
            labels: vec![LABEL_PROXY.to_string()],
        })
        .await?;
    let la: LabelAttestation = cbor::from_slice(&rsp.attestation)?;
    let proxy_label: ProxyLabel = match la.labels.get(LABEL_PROXY) {
        None => return Ok(()),
        Some(value) if value.is_empty() => return Ok(()),
        Some(value) => {
            let proxy_label = BASE64_STANDARD.decode(value)?;
            let proxy_label = kms
                .open_secret(&services::kms::OpenSecretRequest {
                    name: "",
                    value: &proxy_label,
                    context: Some(PROXY_LABEL_ENCRYPTION_CONTEXT),
                })
                .await?
                .value;
            cbor::from_slice(&proxy_label)?
        }
    };

    slog::info!(logger, "proxy configuration is available, starting proxy");
    tokio::spawn(async move {
        if let Err(err) = run(proxy_label).await {
            slog::error!(logger, "failed to start proxy for containers"; "err" => ?err);
        }
    });

    Ok(())
}

async fn run(proxy_label: ProxyLabel) -> Result<()> {
    let logger = get_logger("proxy");

    let _wireguard = wireguard::Client::new(proxy_label.wireguard.clone())
        .context("failed to start wireguard client")?;

    // We only need the IP address part of the IP/CIDR format.
    let listen_address = proxy_label
        .wireguard
        .address
        .split("/")
        .next()
        .ok_or(anyhow!("bad proxy listen address"))?
        .to_string();

    let mut http = http::Proxy::new(http::Config {
        mode: http::Mode::TerminateTls,
        listen_address: listen_address.clone(),
        listen_port: 443,
        ..Default::default()
    })
    .context("failed to start https proxy")?;
    http.start();

    // Parse compose file and add mappings for all ports.
    let compose_port_re = Regex::new(r"^(?P<host_port>\d+):(?P<container_port>\d+)$").unwrap();
    let data = fs::read_to_string(COMPOSE_FILE_PATH).context("failed to load compose file")?;
    let compose = YamlLoader::load_from_str(&data).context("failed to parse compose file")?;
    let compose = compose.first().ok_or(anyhow!("empty compose file"))?;
    let services = compose["services"]
        .as_hash()
        .ok_or(anyhow!("bad services definition"))?;
    for (service_name, service) in services {
        let service_name = match service_name.as_str() {
            Some(service_name) => service_name,
            None => continue,
        };
        let ports = match service["ports"].as_vec() {
            Some(ports) => ports,
            None => continue,
        };

        for port in ports {
            let port: u16 = match port {
                Yaml::String(port) => {
                    // Short port definition.
                    match compose_port_re.captures(port).and_then(|caps| {
                        caps.name("host_port")
                            .and_then(|port| port.as_str().parse().ok())
                    }) {
                        Some(port) => port,
                        None => continue,
                    }
                }
                Yaml::Hash(_) => {
                    // Long port definition.
                    match port["published"]
                        .as_str()
                        .and_then(|port| port.parse().ok())
                    {
                        Some(port) => port,
                        None => continue,
                    }
                }
                _ => continue,
            };

            let name = format!("p{}.{}", port, proxy_label.http_host);

            slog::info!(logger, "adding mapping for port";
                "service" => service_name,
                "port" => port,
                "name" => &name,
            );

            http.add_mapping(http::Mapping {
                name,
                dst_address: "127.0.0.1".to_string(),
                dst_port: port,
            })
            .await;
        }
    }

    loop {
        tokio::task::yield_now().await;
    }
}
