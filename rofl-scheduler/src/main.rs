//! The rofl-scheduler is a ROFL app that acts as the instruction interpreter for the on-chain
//! control plane implemented by the roflmarket module.
use oasis_runtime_sdk::{
    core::common::{logger::get_logger, process},
    modules::rofl::app::prelude::*,
};

mod client;
mod config;
mod manager;
mod manifest;
mod types;

struct SchedulerApp;

#[async_trait]
impl App for SchedulerApp {
    const VERSION: Version = sdk::version_from_cargo!();

    async fn run(self: Arc<Self>, env: Environment<Self>) {
        let logger = get_logger("scheduler");

        // Read local coniguration.
        let cfg = match config::LocalConfig::from_env(env.clone()) {
            Ok(cfg) => cfg,
            Err(err) => {
                slog::error!(logger, "failed to load configuration"; "err" => ?err);
                process::abort();
            }
        };

        let manager = manager::Manager::new(env, cfg);
        manager.run().await
    }
}

fn main() {
    SchedulerApp.start();
}
