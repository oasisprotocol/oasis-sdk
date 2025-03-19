use oasis_runtime_sdk::{core::common::logger::get_logger, modules::rofl::app::prelude::*};
use oasis_runtime_sdk_rofl_market as market;

use super::{config::LocalConfig, SchedulerApp};

/// Main loop of the ROFL scheduler.
pub async fn run(env: Environment<SchedulerApp>, cfg: LocalConfig) {
    let logger = get_logger("scheduler");

    loop {
        // TODO: Wait a bit before doing another pass.

        if let Err(err) = process_pending(&env, &cfg).await {
            slog::error!(logger, "failed to process pending instances"; "err" => ?err);
            continue;
        }
    }
}

async fn process_pending(env: &Environment<SchedulerApp>, cfg: &LocalConfig) -> Result<()> {
    let logger = get_logger("scheduler");
    let client = env.client();

    let round = client.latest_round().await?;
    let instances: Vec<market::types::Instance> = client
        .query(
            round,
            "roflmarket.Instances",
            market::types::ProviderQuery {
                provider: cfg.provider_address,
            },
        )
        .await?;
    // TODO: Check for any pending instances that need to be accepted or cancelled.
    //async fn query<Rq, Rs>(&self, round: u64, method: &str, args: Rq) -> Result<Rs>
    // TODO: Check against local configuration which instance requests may be accepted and accept them.
    // TODO: Deploy instances as needed.

    Ok(())
}
