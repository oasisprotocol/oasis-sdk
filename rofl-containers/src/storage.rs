use std::{
    cmp::Ordering,
    fs,
    os::{fd::AsRawFd, unix::fs::FileExt},
    sync::Arc,
};

use anyhow::{anyhow, Context, Result};
use byteorder::{ByteOrder, LittleEndian};
use cmd_lib::run_cmd;
use libc::{c_int, size_t};
use nix::{ioctl_read, ioctl_read_bad, ioctl_write_ptr, request_code_none};

use oasis_runtime_sdk::{core::common::logger::get_logger, modules::rofl::app::prelude::*};
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
    maybe_resize_storage()?;

    // Mount filesystem as /storage.
    run_cmd!(mount "/dev/mapper/storage" "/storage")?;

    // Setup /var.
    run_cmd!(
        mkdir -p "/storage/var/lib";
        mkdir -p "/storage/var/cache";
        mount --bind "/storage/var" "/var";
    )?;

    Ok(())
}

/// Attempt to open the storage partition block device using the given storage key.
fn open_storage(storage_key: &str) -> Result<()> {
    run_cmd!(
        echo -n ${storage_key} |
            cryptsetup open --type luks2 --disable-locks "/dev/mapper/part-storage" storage
    )
    .map_err(|_| anyhow!("failed to open storage device"))?;

    Ok(())
}

/// Format the storage partition block device using the given storage key.
fn format_storage(storage_key: &str) -> Result<()> {
    // Format block device.
    run_cmd!(
        echo -n ${storage_key} |
            cryptsetup luksFormat --type luks2 --integrity hmac-sha256 --disable-locks "/dev/mapper/part-storage"
    ).map_err(|_| anyhow!("failed to format storage device"))?;

    open_storage(storage_key)?;

    // Format filesystem.
    run_cmd!(mkfs.ext4 "/dev/mapper/storage")?;

    Ok(())
}

/// Checks whether the storage filesystem needs to be resized and then initializes the integrity
/// tags on the added sectors and resizes the filesystem.
fn maybe_resize_storage() -> Result<()> {
    let logger = get_logger("storage");

    ioctl_read_bad!(blksszget, request_code_none!(0x12, 104), c_int);
    ioctl_read!(blkbszget, 0x12, 112, size_t);
    ioctl_write_ptr!(blkbszset, 0x12, 113, size_t);
    ioctl_read!(blkgetsize64, 0x12, 114, u64);

    // Open block device and extract its metadata.
    let dev = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/mapper/storage")?;

    let sector_size = {
        let mut ssz = 0;
        unsafe { blksszget(dev.as_raw_fd(), &mut ssz) }?;
        u64::try_from(ssz)?
    };
    let block_size = {
        let mut bsz = 0;
        unsafe { blkbszget(dev.as_raw_fd(), &mut bsz) }?;
        bsz
    };
    if block_size != usize::try_from(sector_size).unwrap() {
        unsafe { blkbszset(dev.as_raw_fd(), &(sector_size as usize)) }?;
    }
    let dev_size = {
        let mut size = 0;
        unsafe { blkgetsize64(dev.as_raw_fd(), &mut size) }?;
        size
    };

    // Extract current filesystem size by parsing the ext4 superblock.
    let mut superblock = [0u8; 1024];
    dev.read_at(&mut superblock, 1024)
        .context("failed to read filesystem superblock")?;

    let magic = LittleEndian::read_u16(&superblock[0x38..]);
    if magic != 0xEF53 {
        return Err(anyhow!("bad magic in filesystem superblock"));
    }

    let block_count = LittleEndian::read_u32(&superblock[0x04..]);
    let block_size_shift = LittleEndian::read_u32(&superblock[0x18..]);
    let block_size = 1u64
        .checked_shl(10 + block_size_shift)
        .ok_or_else(|| anyhow!("invalid filesystem block size"))?;

    let fs_size = block_size
        .checked_mul(block_count as u64)
        .ok_or_else(|| anyhow!("invalid filesystem size"))?;

    // Determine whether a resize operation is required.
    match fs_size.cmp(&dev_size) {
        Ordering::Equal => {
            slog::info!(
                logger,
                "filesystem is already as big as the device, no resize needed"
            );
            return Ok(());
        }
        Ordering::Greater => {
            slog::error!(
                logger,
                "filesystem is bigger than the device but shrinking is not possible"
            );
            return Err(anyhow!("unable to shrink filesystem"));
        }
        Ordering::Less => {}
    }

    slog::info!(logger, "filesystem is smaller than the device, initializing integrity tags";
        "fs_size" => fs_size,
        "dev_size" => dev_size,
    );

    // First we need to wipe any added sectors to initialize dm-integrity tags.
    let start_sector = fs_size / sector_size;
    let end_sector = dev_size / sector_size;
    let zeroes = vec![0; sector_size.try_into().unwrap()];

    for sector in start_sector..end_sector {
        dev.write_all_at(&zeroes, sector * sector_size)
            .context("failed to zeroize sector")?;
    }

    dev.sync_data().context("failed to sync data")?;
    drop(dev);

    slog::info!(
        logger,
        "device integrity tags initialized, resizing filesystem"
    );

    run_cmd!(
        sh -c "e2fsck -f -p /dev/mapper/storage || [ $? -le 2 ]";
        resize2fs "/dev/mapper/storage";
    )?;

    Ok(())
}
