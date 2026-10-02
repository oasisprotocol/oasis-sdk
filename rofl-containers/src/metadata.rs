use std::{collections::BTreeMap, env};

use anyhow::Result;
use zeroize::Zeroizing;

use oasis_runtime_sdk::{core::common::logger::get_logger, modules};
use rofl_app_core::prelude::*;
use rofl_appd::services;

use crate::utils::RoflDir;

/// Interval at which to refresh the application metadata.
const METADATA_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6);
/// Name of the environment variable for disabling periodic metadata refresh.
const METADATA_REFRESH_DISABLED_ENV_NAME: &str = "ROFL_METADATA_REFRESH_DISABLED";

/// The ephemeral directory for environment variables.
const ROFL_ENV_DIR: &str = "env";
/// The ephemeral directory for secrets.
const ROFL_SECRETS_DIR: &str = "secrets";

/// Initial secrets and environment for the application.
#[derive(Debug, Clone, Default)]
pub struct Initial {
    /// Environment variables.
    pub environment: BTreeMap<String, String>,
    /// Secrets.
    pub secrets: BTreeMap<String, Zeroizing<String>>,
}

/// Initializes the environment from the application metadata and optionally starts a
/// periodic metadata refresh task.
pub async fn start<A: App>(
    env: Environment<A>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<Initial> {
    let logger = get_logger("metadata");

    // Fetch initial app config.
    let app_cfg = match env.client().app_cfg().await {
        Ok(cfg) => cfg,
        Err(err) => {
            slog::error!(logger, "failed to fetch app config"; "err" => ?err);
            return Err(anyhow::anyhow!("failed to fetch app config"));
        }
    };

    let mut initial = Initial::default();

    // Create the ROFL environment and secrets directories.
    let state_dir = RoflDir::new(&[ROFL_ENV_DIR, ROFL_SECRETS_DIR]);
    state_dir.init()?;

    // Initialize environment variables from deployment metadata (env.* keys).
    slog::info!(logger, "initializing container environment variables");
    for (name, value) in env_from_metadata(&app_cfg.metadata) {
        try_update_env(&logger, &state_dir, &name, &value);
        initial.environment.insert(name, value);
    }

    // Initialize secrets.
    slog::info!(logger, "initializing container secrets");
    for (name, value) in secrets_from_metadata(&logger, &kms, &app_cfg.secrets).await? {
        try_update_secret(&logger, &state_dir, &name, &value);
        initial.secrets.insert(name, value);
    }

    match env::var(METADATA_REFRESH_DISABLED_ENV_NAME) {
        Ok(value) if ["1", "yes"].contains(&value.as_str()) => {
            slog::info!(logger, "metadata refresh is disabled");
            return Ok(initial);
        }
        _ => {
            slog::info!(logger, "starting metadata refresh task");
        }
    }

    tokio::task::spawn(async move {
        let mut app_cfg = app_cfg;

        loop {
            tokio::time::sleep(METADATA_REFRESH_INTERVAL).await;

            if let Err(err) = refresh_metadata(&env, &kms, &mut app_cfg, &state_dir).await {
                slog::error!(logger, "failed to refresh metadata"; "err" => ?err);
            }
        }
    });

    Ok(initial)
}

/// Refresh the metadata for the given app, updating environment variables and secrets as needed.
async fn refresh_metadata<A: App>(
    env: &Environment<A>,
    kms: &Arc<dyn services::kms::KmsService>,
    old_app_cfg: &mut modules::rofl::types::AppConfig,
    state_dir: &RoflDir,
) -> Result<()> {
    let logger = get_logger("metadata");

    let app_cfg = env.client().app_cfg().await?;

    // Check if any environment variables have changed.
    let mut old_env = env_from_metadata(&old_app_cfg.metadata);
    let new_env = env_from_metadata(&app_cfg.metadata);
    for (name, value) in &new_env {
        if old_env.remove(name).as_ref() != Some(value) {
            try_update_env(&logger, state_dir, name, value);
        }
    }
    for name in old_env.keys() {
        // Replace removed environment variables with empty values.
        try_update_env(&logger, state_dir, name, "");
    }

    // Check if any secrets have changed.
    let mut old_secrets = secrets_from_metadata(&logger, kms, &old_app_cfg.secrets).await?;
    let new_secrets = secrets_from_metadata(&logger, kms, &app_cfg.secrets).await?;
    for (name, value) in new_secrets {
        if old_secrets.remove(&name).as_ref() != Some(&value) {
            try_update_secret(&logger, state_dir, &name, &value);
        }
    }
    for name in old_secrets.keys() {
        // Replace removed secrets with empty values.
        try_update_secret(&logger, state_dir, name, "");
    }

    *old_app_cfg = app_cfg;
    Ok(())
}

fn try_update_secret(logger: &slog::Logger, state_dir: &RoflDir, name: &str, value: &str) {
    if let Err(err) = state_dir.set(ROFL_SECRETS_DIR, name, value) {
        slog::error!(logger, "failed to update secret"; "name" => name, "err" => ?err);
    } else {
        slog::info!(logger, "refreshed secret"; "name" => name);
    }
}

/// Extract secrets from the given metadata, opening them from the KMS as needed.
async fn secrets_from_metadata(
    logger: &slog::Logger,
    kms: &Arc<dyn services::kms::KmsService>,
    secrets: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Zeroizing<String>>> {
    let mut result = BTreeMap::new();
    for (pub_name, encrypted_value) in secrets {
        let (name, value) = match open_secret(kms, pub_name, encrypted_value).await {
            Ok((name, value)) => (name, value),
            Err(err) => {
                slog::warn!(logger, "failed to open secret"; "name" => pub_name, "err" => ?err);
                continue;
            }
        };
        result.insert(name, value);
    }
    Ok(result)
}

/// Open a secret from the KMS using the given public name and encrypted value.
async fn open_secret(
    kms: &Arc<dyn services::kms::KmsService>,
    pub_name: &str,
    encrypted_value: &[u8],
) -> Result<(String, Zeroizing<String>)> {
    // Decrypt and authenticate secret.
    let (name, value) = kms
        .open_secret(&services::kms::OpenSecretRequest {
            name: pub_name,
            value: encrypted_value,
            context: None,
        })
        .await
        .map(|response| (response.name, response.value))?;

    // Assume the name and value are always valid strings.
    let name = String::from_utf8_lossy(&name);
    let value = String::from_utf8_lossy(&value);

    Ok((name.into_owned(), Zeroizing::new(value.into_owned())))
}

fn try_update_env(logger: &slog::Logger, state_dir: &RoflDir, name: &str, value: &str) {
    if let Err(err) = state_dir.set(ROFL_ENV_DIR, name, value) {
        slog::error!(logger, "failed to update environment variable"; "name" => name, "err" => ?err);
    } else {
        slog::info!(logger, "refreshed environment variable"; "name" => name);
    }
}

/// Extract environment variables from deployment metadata.
fn env_from_metadata(metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    metadata
        .iter()
        .filter_map(|(key, value)| {
            parse_env_metadata_key(key).map(|name| (name.to_string(), value.clone()))
        })
        .collect()
}

/// Parse a metadata key that follows the environment variable convention.
///
/// The canonical format is `env.<VAR>`, where `<VAR>` environment variable
/// consists of a non-empty ASCII alphanumeric characters or `_`.
///
/// Dotted forms are intentionally rejected so `env.<SERVICE>.<VAR>` can be added
/// later without being breaking.
fn parse_env_metadata_key(key: &str) -> Option<&str> {
    let name = key.strip_prefix("env.")?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    Some(name)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_env_metadata_key() {
        let tcs = vec![
            ("foo", None),
            ("env.", None),
            ("env.MY_VAR", Some("MY_VAR")),
            ("env.MY_VAR_1", Some("MY_VAR_1")),
            ("env.service_A.MY_VAR", None),
            ("env.MY-VAR", None),
            ("env.MY VAR", None),
            ("env.my_var", Some("my_var")),
        ];
        for tc in tcs {
            assert_eq!(parse_env_metadata_key(tc.0), tc.1);
        }
    }

    #[test]
    fn test_env_from_empty_metadata() {
        assert_eq!(env_from_metadata(&BTreeMap::new()), BTreeMap::new());
    }

    #[test]
    fn test_env_from_metadata_with_non_env_keys() {
        let metadata = BTreeMap::from([
            (".env".to_string(), "ignored".to_string()),
            ("env.MY_VAR".to_string(), "my value".to_string()),
        ]);

        assert_eq!(
            env_from_metadata(&metadata),
            BTreeMap::from([("MY_VAR".to_string(), "my value".to_string())])
        );
    }

    #[test]
    fn test_env_from_metadata() {
        let metadata = BTreeMap::from([
            ("env.".to_string(), "ignored".to_string()),
            ("env.MY_VAR".to_string(), "my value".to_string()),
            ("env.OTHER_VAR".to_string(), "other value".to_string()),
            ("env.service_A.MY_VAR".to_string(), "reserved".to_string()),
            ("env.MY-VAR".to_string(), "ignored".to_string()),
            ("net.oasis.foo".to_string(), "ignored".to_string()),
        ]);

        assert_eq!(
            env_from_metadata(&metadata),
            BTreeMap::from([
                ("MY_VAR".to_string(), "my value".to_string()),
                ("OTHER_VAR".to_string(), "other value".to_string()),
            ])
        );
    }
}
