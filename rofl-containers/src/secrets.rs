use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write,
    sync::Arc,
};

use anyhow::Result;
use cmd_lib::run_cmd;

use oasis_runtime_sdk::{core::common::logger::get_logger, modules::rofl::app::prelude::*};
use rofl_appd::services::{self, kms::OpenSecretRequest};

/// Initialize secrets available to containers.
pub async fn init<A: App>(
    env: Environment<A>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("secrets");

    // Query own app cfg to get encrypted secrets.
    let encrypted_secrets = env.client().app_cfg().await?.secrets;

    // Also generate secrets in an environment file.
    fs::create_dir_all("/run/podman")?;
    let mut secrets_env = File::create("/run/podman/secrets.env")?;

    // Ensure all secrets are removed.
    run_cmd!(podman secret rm --all)?;
    // Create all requested secrets.
    let mut existing_env_vars = BTreeSet::new();
    for (pub_name, value) in encrypted_secrets {
        // Decrypt and authenticate secret. In case of failures, the secret is skipped.
        let (name, value) = match kms
            .open_secret(&OpenSecretRequest {
                name: &pub_name,
                value: &value,
            })
            .await
        {
            Ok(response) => (response.name, response.value),
            Err(_) => continue, // Skip bad secrets.
        };
        // Assume the name and value are always valid strings.
        let name = String::from_utf8_lossy(&name);
        let name_upper = name.to_uppercase().replace(" ", "_");
        let value = String::from_utf8_lossy(&value);

        // Create a new Podman secret in temporary storage on /run to avoid it being persisted.
        let _ = run_cmd!(echo -n $value | podman secret create --driver-opts file=/run/podman/secrets --replace $name -);

        // Also store in the secrets environment file.
        if !existing_env_vars.contains(&name_upper) {
            writeln!(&mut secrets_env, "{name_upper}={value}")?;
            existing_env_vars.insert(name_upper);
        }

        slog::info!(logger, "provisioned secret"; "pub_name" => pub_name);
    }
    Ok(())
}
