use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use cmd_lib::run_cmd;

use oasis_runtime_sdk::core::common::logger::get_logger;
use rofl_app_core::prelude::*;
use rofl_appd::services::{self, kms::OpenSecretRequest};

use crate::{containers, utils};

/// Initialize secrets available to containers.
pub async fn init(
    encrypted_secrets: &BTreeMap<String, Vec<u8>>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("secrets");

    // Create the ROFL secrets directory.
    run_cmd!(rm -rf "/run/rofl/secrets")?;
    run_cmd!(mkdir -p "/run/rofl/secrets")?;

    // Ensure all secrets are removed.
    run_cmd!(podman secret rm --all)?;
    // Create all requested secrets.
    for (pub_name, encrypted_value) in encrypted_secrets {
        if update_secret(&kms, pub_name, encrypted_value)
            .await
            .is_err()
        {
            continue; // Skip bad secrets.
        }

        slog::info!(logger, "provisioned secret"; "pub_name" => pub_name);
    }
    Ok(())
}

/// Update a single secret from the KMS.
pub async fn update_secret(
    kms: &Arc<dyn services::kms::KmsService>,
    pub_name: &str,
    encrypted_value: &[u8],
) -> Result<()> {
    // Decrypt and authenticate secret. In case of failures, the secret is skipped.
    let (name, value) = kms
        .open_secret(&OpenSecretRequest {
            name: pub_name,
            value: encrypted_value,
            context: None,
        })
        .await
        .map(|response| (response.name, response.value))?;
    // Assume the name and value are always valid strings.
    let name = String::from_utf8_lossy(&name);
    let name_upper = name.to_uppercase().replace(" ", "_");
    let value = String::from_utf8_lossy(&value);

    // Create a new Podman secret in temporary storage on /run to avoid it being persisted.
    let _ = run_cmd!(echo -n $value | podman secret create --driver-opts file=/run/podman/secrets --replace $name -);

    // Also store in the secrets environment file.
    containers::env().set(&name_upper, &value);

    // Also store in the ROFL secrets file.
    let sane_name = utils::sanitize_filename(name.as_ref());
    std::fs::create_dir_all(format!("/run/rofl/secrets/{}", sane_name))?;
    std::fs::write(
        format!("/run/rofl/secrets/{}/name", sane_name),
        name.as_ref(),
    )?;
    std::fs::write(
        format!("/run/rofl/secrets/{}/value", sane_name),
        value.as_ref(),
    )?;

    Ok(())
}
