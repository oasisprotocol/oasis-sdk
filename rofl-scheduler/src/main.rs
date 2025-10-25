//! The rofl-scheduler is a ROFL app that acts as the instruction interpreter for the on-chain
//! control plane implemented by the roflmarket module.
#![feature(once_cell_try)]

use std::collections::BTreeMap;

use oasis_runtime_sdk::core::{
    common::{logger::get_logger, process},
    Protocol,
};
use rofl_app_core::prelude::*;

mod client;
mod config;
mod manager;
mod manifest;
mod proxy;
mod qcow2;
mod serverd;
mod state;
mod types;

struct SchedulerApp {
    cfg: Option<Arc<config::LocalConfig>>,
}

impl SchedulerApp {
    fn new() -> Self {
        Self { cfg: None }
    }
}

/// Name of the metadata key used to store the endpoint URL.
const METADATA_KEY_ENDPOINT_URL: &str = "net.oasis.scheduler.api";
/// Metadata key used to store the used proxy domain.
const METADATA_KEY_PROXY_DOMAIN: &str = "net.oasis.proxy.domain";

/// TCP port that the HTTPS server should listen on.
const HTTPS_SERVER_PORT: u16 = 443;

#[async_trait]
impl App for SchedulerApp {
    const VERSION: Version = sdk::version_from_cargo!();

    fn init(&mut self, host: Arc<Protocol>) {
        let logger = get_logger("scheduler");

        let cfg = match config::LocalConfig::from_host(host) {
            Ok(cfg) => Arc::new(cfg),
            Err(err) => {
                slog::error!(logger, "failed to load configuration"; "err" => ?err);
                process::abort();
            }
        };

        if let Err(err) = serverd::tls::Identity::init() {
            slog::error!(logger, "failed to initialize TLS identity"; "err" => ?err);
            process::abort();
        }

        self.cfg = Some(cfg);
    }

    async fn get_metadata(
        self: Arc<Self>,
        _env: Environment<Self>,
    ) -> Result<BTreeMap<String, String>> {
        let cfg = self.cfg.as_ref().unwrap();
        let mut meta = BTreeMap::new();
        if let Some(identity) = serverd::tls::Identity::global() {
            meta.extend(identity.metadata());
        }
        if let Some(api_domain) = &cfg.api_domain {
            meta.insert(
                METADATA_KEY_ENDPOINT_URL.to_string(),
                format!("https://{api_domain}"),
            );
        }
        if let Some(proxy) = &cfg.proxy {
            meta.insert(METADATA_KEY_PROXY_DOMAIN.to_string(), proxy.domain.clone());
        }

        Ok(meta)
    }

    async fn run(self: Arc<Self>, env: Environment<Self>) {
        let logger = get_logger("scheduler");
        let cfg = self.cfg.as_ref().unwrap();

        // Create and start the proxy when configured.
        let proxy = cfg.proxy.as_ref().map(|cfg| {
            let mut proxy = proxy::Proxy::new(cfg, HTTPS_SERVER_PORT).unwrap();
            proxy.start();

            Arc::new(proxy)
        });

        // Create the manager.
        let manager = manager::Manager::new(env.clone(), cfg.clone(), proxy.clone());

        // Start API server when enabled.
        if let Some(domain) = &cfg.api_domain {
            let address = match proxy.as_ref() {
                Some(proxy) => {
                    // When proxy is configured change the listen address and add mapping.
                    let api_server_address = "127.0.0.1";
                    let api_server_port = 444;

                    proxy
                        .add_static_mapping(rofl_proxy::http::Mapping {
                            name: domain.clone(),
                            dst_address: api_server_address.to_string(),
                            dst_port: api_server_port,
                            mode: rofl_proxy::http::Mode::ForwardOnly,
                        })
                        .await;

                    &format!("0.0.0.0:{api_server_port}")
                }
                None => {
                    // When no proxy is configured, directly listen on the HTTPS port.
                    &format!("0.0.0.0:{HTTPS_SERVER_PORT}")
                }
            };

            if let Err(err) = serverd::serve(serverd::Config {
                address,
                domain,
                env,
                manager: manager.clone(),
                config: cfg.clone(),
            })
            .await
            {
                slog::error!(logger, "failed to start API server"; "err" => ?err);
            }
        }

        // Start the manager.
        slog::info!(logger, "starting manager");
        manager.run().await
    }
}

fn main() {
    SchedulerApp::new().start();
}
