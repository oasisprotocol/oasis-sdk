use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use cmd_lib::run_cmd;

use oasis_runtime_sdk::core::common::logger::get_logger;
use rofl_app_core::prelude::*;
use rofl_appd::services::{self, kms::OpenSecretRequest};

use crate::containers;

/// Initialize secrets available to containers.
pub async fn init(
    encrypted_secrets: &BTreeMap<String, Vec<u8>>,
    kms: Arc<dyn services::kms::KmsService>,
) -> Result<()> {
    let logger = get_logger("secrets");

    // Ensure all secrets are removed.
    run_cmd!(podman secret rm --all)?;
    // Create all requested secrets.
    for (pub_name, encrypted_value) in encrypted_secrets {
        // Decrypt and authenticate secret. In case of failures, the secret is skipped.
        let (name, value) = match kms
            .open_secret(&OpenSecretRequest {
                name: pub_name,
                value: encrypted_value,
                context: None,
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
        containers::env().set(&name_upper, &value);

        slog::info!(logger, "provisioned secret"; "pub_name" => pub_name);
    }
    Ok(())
}
