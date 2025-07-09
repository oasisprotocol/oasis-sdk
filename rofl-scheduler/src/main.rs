//! The rofl-scheduler is a ROFL app that acts as the instruction interpreter for the on-chain
//! control plane implemented by the roflmarket module.
#![feature(once_cell_try)]

use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    core::{
        common::{logger::get_logger, process},
        Protocol,
    },
    modules::rofl::app::prelude::*,
};

mod client;
mod config;
mod manager;
mod manifest;
mod qcow2;
mod serverd;
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

        self.cfg = Some(cfg);
    }

    async fn get_metadata(
        self: Arc<Self>,
        _env: Environment<Self>,
    ) -> Result<BTreeMap<String, String>> {
        let cfg = self.cfg.as_ref().unwrap();
        let mut meta = serverd::tls::Identity::global()?.metadata();
        if let Some(api_domain) = &cfg.api_domain {
            meta.insert(
                METADATA_KEY_ENDPOINT_URL.to_string(),
                format!("https://{}", api_domain),
            );
        }

        Ok(meta)
    }

    async fn run(self: Arc<Self>, env: Environment<Self>) {
        let logger = get_logger("scheduler");

        // Create the manager.
        let cfg = self.cfg.as_ref().unwrap();
        let manager = manager::Manager::new(env.clone(), cfg.clone());

        // Start API server when enabled.
        if let Some(domain) = &cfg.api_domain {
            if let Err(err) = serverd::serve(serverd::Config {
                address: "0.0.0.0:443",
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
        manager.run().await
    }
}

fn main() {
    SchedulerApp::new().start();
}
