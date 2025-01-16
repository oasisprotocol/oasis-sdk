use std::{fs, sync::Arc};

use anyhow::Result;
use cmd_lib::run_cmd;

use rofl_appd::services::{
    self,
    kms::{GenerateRequest, KeyKind},
};

/// Storage encryption key identifier.
const STORAGE_ENCRYPTION_KEY_ID: &str =
    "oasis-runtime-sdk/rofl-containers: storage encryption key v1";

/// Initialize stage 2 storage based on configuration.
pub async fn init(kms: Arc<dyn services::kms::KmsService>) -> Result<()> {
    // Parse kernel command line to determine relevant features.
    let cmdline = fs::read_to_string("/proc/cmdline")?;
    let storage_mode = cmdline
        .split(' ')
        .filter_map(|s| {
            if !s.is_empty() {
                Some(s.split_once('=')?)
            } else {
                None
            }
        })
        .filter(|(k, _)| *k == "oasis.stage2.storage_mode")
        .map(|(_, v)| v)
        .next();
    if storage_mode != Some("custom") {
        return Ok(()); // Ignore non-custom storage mode.
    }

    // Derive storage key.
    let storage_key = kms
        .generate(&GenerateRequest {
            key_id: STORAGE_ENCRYPTION_KEY_ID,
            kind: KeyKind::Raw384,
        })
        .await?;
    let storage_key = hex::encode(&storage_key.key);

    // Ensure all device mapper devices are present.
    run_cmd!(dmsetup mknodes)?;

    // Open or re-format storage.
    let result = open_storage(&storage_key);
    if result.is_err() {
        format_storage(&storage_key)?;
    }

    // Mount filesystem as /storage.
    run_cmd!(mount "/dev/mapper/storage" "/storage")?;

    // Setup /run and /var.
    run_cmd!(
        mkdir "/storage/run";
        mkdir -p "/storage/var/lib";
        mkdir -p "/storage/var/cache";
        mount --bind "/storage/run" "/run";
        mount --bind "/storage/var" "/var";
    )?;

    Ok(())
}

/// Attempt to open the storage partition block device using the given storage key.
fn open_storage(storage_key: &str) -> Result<()> {
    run_cmd!(
        echo -n ${storage_key} |
            cryptsetup open --type luks2 --disable-locks "/dev/mapper/part-storage" storage
    )?;

    Ok(())
}

/// Format the storage partition block device using the given storage key.
fn format_storage(storage_key: &str) -> Result<()> {
    // Format block device.
    run_cmd!(
        echo -n ${storage_key} |
            cryptsetup luksFormat --type luks2 --integrity hmac-sha256 --disable-locks "/dev/mapper/part-storage"
    )?;

    open_storage(storage_key)?;

    // Format filesystem.
    run_cmd!(mkfs.ext4 "/dev/mapper/storage")?;

    Ok(())
}
