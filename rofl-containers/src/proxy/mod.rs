mod compose;
mod firewall;

use std::{
    collections::{BTreeMap, HashSet},
    fs, future,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use base64::prelude::*;

use oasis_runtime_sdk::core::{
    common::logger::get_logger,
    host::attestation::{AttestLabelsRequest, LabelAttestation},
};
use rofl_app_core::prelude::*;
use rofl_appd::services;
use rofl_proxy::{
    http,
    wireguard::{self, WG_INTERFACE_NAME},
    ProxyLabel, LABEL_PROXY, PROXY_LABEL_ENCRYPTION_CONTEXT,
};

use crate::{App, Environment};

use compose::{ParsedCompose, PortMappingMode};
use firewall::Firewall;

/// Location of the compose file.
const COMPOSE_FILE_PATH: &str = "/etc/oasis/containers/compose.yaml";
/// Port that the HTTPS proxy should listen on.
const PROXY_HTTPS_LISTEN_PORT: u16 = 443;
/// Handshake timeout of the HTTPS proxy. This also covers the TLS handshake when terminating TLS,
/// which requires a round trip to the client.
const PROXY_HTTPS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Name of the environment variable for the host.
const PROXY_HOST_ENV_NAME: &str = "ROFL_PROXY_HOST";
/// Name of the environment variable for the external address.
const PROXY_EXTERNAL_ADDRESS_ENV_NAME: &str = "ROFL_PROXY_EXTERNAL_ADDRESS";
/// Supported PROXY protocol version.
const PROXY_PROTOCOL_V2: &str = "v2";

/// State of the initialized proxy.
#[derive(Debug, Clone, Default)]
pub struct ProxyState {
    /// Extra environment variables to set.
    pub extra_environment: BTreeMap<String, String>,
}

/// Start the proxy if configured.
pub(crate) async fn start<A: App>(
    env: Environment<A>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<ProxyState> {
    let mut state = ProxyState::default();
    let logger = get_logger("proxy");

    // Fetch proxy configuration to see if the proxy is available.
    let rsp = env
        .host()
        .attestation()
        .attest_labels(AttestLabelsRequest {
            labels: vec![LABEL_PROXY.to_string()],
        })
        .await?;
    let la: LabelAttestation = cbor::from_slice(&rsp.attestation)?;
    let proxy_label: ProxyLabel = match la.labels.get(LABEL_PROXY) {
        Some(value) if !value.is_empty() => {
            let proxy_label = BASE64_STANDARD.decode(value)?;
            let proxy_label = kms
                .open_secret(&services::kms::OpenSecretRequest {
                    name: "",
                    value: &proxy_label,
                    context: Some(PROXY_LABEL_ENCRYPTION_CONTEXT),
                })
                .await
                .context("corrupted proxy configuration")?
                .value;
            cbor::from_slice(&proxy_label).context("malformed proxy configuration")?
        }
        _ => return Ok(state),
    };

    // Parse and process compose file to see if we even need the proxy.
    let data = fs::read_to_string(COMPOSE_FILE_PATH).context("failed to load compose file")?;
    let compose = ParsedCompose::parse(&data).context("failed to parse compose file")?;
    let compose = postprocess_compose(compose, proxy_label.http.proxy_protocol, &logger);
    if compose.port_mappings.is_empty() {
        slog::info!(
            logger,
            "no port mappings are configured, not starting proxy"
        );
        return Ok(state);
    }

    // Store the proxy domain and optional external address in an environment variable so that the
    // containers can use it in their configuration.
    state.extra_environment.insert(
        PROXY_HOST_ENV_NAME.to_string(),
        proxy_label.http.host.clone(),
    );
    if let Some(external_address) = &proxy_label.http.external_address {
        state.extra_environment.insert(
            PROXY_EXTERNAL_ADDRESS_ENV_NAME.to_string(),
            external_address.clone(),
        );
    }

    slog::info!(logger, "proxy configuration is available, starting proxy");
    tokio::spawn(async move {
        if let Err(err) = run(proxy_label, compose, kms).await {
            slog::error!(logger, "failed to start proxy"; "err" => ?err);
        }
    });

    Ok(state)
}

async fn run(
    proxy_label: ProxyLabel,
    compose: ParsedCompose,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("proxy");

    // We only need the IP address part of the IP/CIDR format.
    let listen_address = proxy_label
        .wireguard
        .address
        .split("/")
        .next()
        .ok_or(anyhow!("bad proxy listen address"))?
        .to_string();

    let hub_address = proxy_label
        .wireguard
        .hub_address
        .split("/")
        .next()
        .ok_or(anyhow!("bad hub address"))?;

    // When the hub sends a PROXY protocol header, only the hub may connect to the proxy.
    let proxy_protocol_hub = if proxy_label.http.proxy_protocol {
        Some(hub_address.parse().context("bad hub address")?)
    } else {
        None
    };

    // Setup firewall.
    slog::info!(logger, "setting up firewall");
    let mut firewall = Firewall::new();
    firewall
        .add_wireguard(
            WG_INTERFACE_NAME,
            hub_address,
            &listen_address,
            PROXY_HTTPS_LISTEN_PORT,
        )
        .context("failed to add proxy firewall rules")?;
    firewall.start().context("failed to start firewall")?;

    // Setup wireguard tunnel.
    slog::info!(logger, "setting up wireguard");
    let wg_cfg = wireguard::ClientConfig {
        listen_port: wireguard::WG_DEFAULT_LISTEN_PORT,
        ..proxy_label.wireguard.clone()
    };
    let mut wireguard =
        wireguard::Client::new(wg_cfg).context("failed to create wireguard client")?;
    wireguard
        .start()
        .context("failed to start wireguard client")?;

    // Create ACME account from KMS-derived key.
    slog::info!(logger, "creating ACME account");
    let kms_acme = kms.clone();
    let acme = http::tls::init_acme_account(
        move |key_id: &[u8]| {
            let kms_acme = kms_acme.clone();
            let key_id = std::str::from_utf8(key_id)
                .expect("key_id must be valid UTF-8")
                .to_string();
            async move {
                let response = kms_acme
                    .generate(&services::kms::GenerateRequest {
                        key_id,
                        kind: services::kms::KeyKind::Raw256,
                    })
                    .await
                    .context("failed to generate ACME key from KMS")?;
                Ok(http::tls::RawAcmeKey(response.key.clone()))
            }
        },
        // We could allow apps to configure to use staging url.
        http::tls::LetsEncrypt::Production.url(),
    )
    .await
    .context("failed to initialize ACME account")?;

    // Setup HTTPS proxy.
    slog::info!(logger, "setting up application proxy");
    let mut http = http::Proxy::new(
        http::Config {
            listen_address: listen_address.clone(),
            listen_port: PROXY_HTTPS_LISTEN_PORT,
            timeout_handshake: PROXY_HTTPS_HANDSHAKE_TIMEOUT,
            proxy_protocol_hub,
            ..Default::default()
        },
        acme,
    )
    .context("failed to create https proxy")?;
    http.start();

    let mut names = HashSet::new();
    for mapping in compose.port_mappings {
        let name = match mapping.custom_domain {
            Some(domain) => domain,
            None => format!("p{}.{}", mapping.port.host_port, proxy_label.http.host),
        };

        let Some(canonical_name) = http::canonical_name(&name) else {
            slog::warn!(logger, "ignoring mapping with invalid domain name";
                "service" => &mapping.service,
                "name" => &name,
                "host_port" => mapping.port.host_port,
            );
            continue;
        };

        // Names that only differ in spelling (e.g. case) map to the same proxy route.
        if !names.insert(canonical_name) {
            slog::warn!(logger, "ignoring duplicate mapping";
                "service" => &mapping.service,
                "name" => &name,
                "host_address" => &mapping.port.host_address,
                "host_port" => mapping.port.host_port,
                "container_port" => mapping.port.container_port,
            );
            continue;
        }

        slog::info!(logger, "adding mapping for port";
            "service" => &mapping.service,
            "name" => &name,
            "host_address" => &mapping.port.host_address,
            "host_port" => mapping.port.host_port,
            "container_port" => mapping.port.container_port,
            "timeouts" => ?mapping.timeouts,
        );

        let result = http
            .add_mapping(http::Mapping {
                name: name.clone(),
                dst_address: mapping.port.host_address,
                dst_port: mapping.port.host_port,
                mode: match mapping.mode {
                    PortMappingMode::TerminateTls => http::Mode::TerminateTls,
                    PortMappingMode::Passthrough => http::Mode::ForwardOnly,
                    PortMappingMode::Ignore => continue,
                },
                proxy_protocol: mapping.proxy_protocol.is_some(),
                timeouts: mapping.timeouts,
                connection_group: None,
            })
            .await;
        if let Err(err) = result {
            slog::warn!(logger, "failed to add mapping for port";
                "service" => &mapping.service,
                "name" => &name,
                "err" => ?err,
            );
        }
    }

    // Wait forever.
    future::pending().await
}

/// Postprocess the parsed compose file.
fn postprocess_compose(
    mut compose: ParsedCompose,
    proxy_protocol: bool,
    logger: &slog::Logger,
) -> ParsedCompose {
    // Filter out port mappings that we don't care about.
    compose.port_mappings = compose
        .port_mappings
        .into_iter()
        .filter(|mapping| mapping.port.protocol == "tcp")
        .filter(|mapping| mapping.mode != PortMappingMode::Ignore)
        // Never forward to a port that expects a PROXY protocol header without sending one.
        .filter(|mapping| match mapping.proxy_protocol.as_deref() {
            None => true,
            Some(PROXY_PROTOCOL_V2) if proxy_protocol => true,
            Some(version) => {
                slog::warn!(logger, "ignoring mapping with unavailable PROXY protocol";
                    "service" => &mapping.service,
                    "host_port" => mapping.port.host_port,
                    "proxy_protocol" => version,
                );
                false
            }
        })
        .collect();

    compose
}

#[cfg(test)]
mod test {
    use super::{
        compose::{ParsedPort, PortMapping},
        *,
    };

    #[test]
    fn test_postprocess_compose_proxy_protocol() {
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        let mapping = |proxy_protocol: Option<&str>| PortMapping {
            proxy_protocol: proxy_protocol.map(str::to_string),
            port: ParsedPort {
                protocol: "tcp".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let compose = ParsedCompose {
            port_mappings: vec![mapping(None), mapping(Some("v2")), mapping(Some("v1"))],
        };

        // Mappings that expect a header should be removed when it cannot be sent.
        let result = postprocess_compose(compose.clone(), false, &logger);
        assert_eq!(result.port_mappings, vec![mapping(None)]);

        let result = postprocess_compose(compose, true, &logger);
        assert_eq!(
            result.port_mappings,
            vec![mapping(None), mapping(Some("v2"))]
        );
    }
}
