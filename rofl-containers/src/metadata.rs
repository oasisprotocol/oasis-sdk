use std::env;

use anyhow::Result;
use cmd_lib::run_cmd;

use oasis_runtime_sdk::{core::common::logger::get_logger, modules};
use rofl_app_core::prelude::*;
use rofl_appd::services;

use crate::{containers, secrets, utils};

/// Interval at which to refresh the application metadata.
const METADATA_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6);
/// Name of the environment variable for disabling periodic metadata refresh.
const METADATA_REFRESH_DISABLED_ENV_NAME: &str = "ROFL_METADATA_REFRESH_DISABLED";

/// Initializes the environment from the application metadata and optionally starts a
/// periodic metadata refresh task.
pub async fn start<A: App>(
    env: Environment<A>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("metadata_refresh");

    // Fetch initial app config.
    let app_cfg = match env.client().app_cfg().await {
        Ok(cfg) => cfg,
        Err(err) => {
            slog::error!(logger, "failed to fetch app config"; "err" => ?err);
            return Err(anyhow::anyhow!("failed to fetch app config"));
        }
    };

    // Create the ROFL env directory.
    run_cmd!(rm -rf "/run/rofl/env")?;
    run_cmd!(mkdir -p "/run/rofl/env")?;

    // Initialize environment variables from deployment metadata (env.* keys).
    slog::info!(logger, "initializing container environment variables");
    for (name, value) in containers::env_from_metadata(&app_cfg.metadata) {
        containers::env().set(&name, &value);
        slog::info!(logger, "provisioned environment variable"; "name" => name);
    }

    // Initialize secrets (runs after env vars so secrets take precedence on collision).
    slog::info!(logger, "initializing container secrets");
    if let Err(err) = secrets::init(&app_cfg.secrets, kms.clone()).await {
        slog::error!(logger, "failed to initialize container secrets"; "err" => ?err);
        return Err(anyhow::anyhow!("failed to initialize container secrets"));
    }

    match env::var(METADATA_REFRESH_DISABLED_ENV_NAME) {
        Ok(value) if ["1", "yes"].contains(&value.as_str()) => {
            slog::info!(logger, "metadata refresh is disabled");
            return Ok(());
        }
        _ => {
            slog::info!(logger, "starting metadata refresh task");
        }
    }

    tokio::task::spawn(async move {
        let mut app_cfg = app_cfg;

        loop {
            tokio::time::sleep(METADATA_REFRESH_INTERVAL).await;
            if let Err(err) = refresh_metadata(&env, &kms, &mut app_cfg).await {
                slog::error!(logger, "failed to refresh metadata"; "err" => ?err);
            }
        }
    });

    Ok(())
}

async fn refresh_metadata<A: App>(
    env: &Environment<A>,
    kms: &Arc<dyn services::kms::KmsService>,
    old_app_cfg: &mut modules::rofl::types::AppConfig,
) -> Result<()> {
    let logger = get_logger("metadata_refresh");

    let app_cfg = env.client().app_cfg().await?;

    // Check if any environment variables have changed.
    let mut old_env = containers::env_from_metadata(&old_app_cfg.metadata);
    let new_env = containers::env_from_metadata(&app_cfg.metadata);
    for (name, value) in &new_env {
        if let Some(old_value) = old_env.remove(name) {
            if old_value != *value {
                try_update_env(&logger, name, value);
            }
        } else {
            try_update_env(&logger, name, value);
        }
    }
    for name in old_env.keys() {
        // Replace removed environment variables with empty values.
        try_update_env(&logger, name, "");
    }

    // Check if any secrets have changed.
    let old_secrets = &old_app_cfg.secrets;
    let new_secrets = &app_cfg.secrets;
    for (pub_name, encrypted_value) in new_secrets {
        if let Some(old_value) = old_secrets.get(pub_name) {
            if old_value != encrypted_value {
                try_update_secret(&logger, kms, pub_name.as_ref(), encrypted_value).await;
            }
        } else {
            try_update_secret(&logger, kms, pub_name.as_ref(), encrypted_value).await;
        }
    }

    *old_app_cfg = app_cfg;
    Ok(())
}

async fn try_update_secret(
    logger: &slog::Logger,
    kms: &Arc<dyn services::kms::KmsService>,
    name: &str,
    encrypted_value: &[u8],
) {
    if let Err(err) = secrets::update_secret(kms, name, encrypted_value).await {
        slog::error!(logger, "failed to update secret"; "name" => name, "err" => ?err);
    } else {
        slog::info!(logger, "refreshed secret"; "name" => name);
    }
}

fn try_update_env(logger: &slog::Logger, name: &str, value: &str) {
    if let Err(err) = update_env(name, value) {
        slog::error!(logger, "failed to update environment variable"; "name" => name, "err" => ?err);
    } else {
        slog::info!(logger, "refreshed environment variable"; "name" => name);
    }
}

fn update_env(name: &str, value: &str) -> Result<()> {
    let sane_name = utils::sanitize_filename(name);

    std::fs::create_dir_all(format!("/run/rofl/env/{}", sane_name))?;
    std::fs::write(format!("/run/rofl/env/{}/name", sane_name), name)?;
    std::fs::write(format!("/run/rofl/env/{}/value", sane_name), value)?;
    Ok(())
}
