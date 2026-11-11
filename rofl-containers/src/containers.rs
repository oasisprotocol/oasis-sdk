use std::{fs, process::Command, time::SystemTime};

use anyhow::Result;
use cmd_lib::run_cmd;

use crate::metadata;

/// Initialize container environment.
pub async fn init() -> Result<()> {
    // Setup networking.
    run_cmd!(
        ip link set lo up;
        udhcpc -i eth0 -q -n;
    )?;

    // Mount cgroups and create /dev/shm for Podman locks.
    run_cmd!(
        mount -t cgroup2 none "/sys/fs/cgroup";
        mkdir -p "/dev/shm";
        mount -t tmpfs none "/dev/shm";
    )?;

    // Cleanup state after reboot.
    run_cmd!(
        rm -rf "/storage/containers/run";
        rm -rf "/storage/containers/net";
        rm -rf "/var/lib/cni";

        mkdir -p "/storage/containers/run";
        mkdir -p "/storage/containers/graph";
        mkdir -p "/storage/containers/graph/tmp";
        mkdir -p "/storage/containers/net";
    )?;

    // Update TUN device permissions.
    run_cmd!(chmod 0666 "/dev/net/tun")?;

    // Migrate existing containers if needed and initialize state.
    run_cmd!(
        podman system migrate;
        podman system prune --external;
        podman image prune --all --force;
        podman secret rm --all;
    )?;

    fs::create_dir_all("/run/podman")?;

    Ok(())
}

/// Start containers.
pub async fn start(initial: metadata::Initial) -> Result<()> {
    // Initialize secrets.
    for (name, value) in &initial.secrets {
        // Create a new Podman secret in temporary storage on /run to avoid it being persisted.
        let _ = run_cmd!(echo -n $value | podman secret create --driver-opts file=/run/podman/secrets --replace $name -);
    }

    // Run the podman API service.
    Command::new("podman")
        .args(["system", "service", "--time=0", "unix:///run/podman.sock"])
        .spawn()?;

    // Bring containers up.
    let status =
        Command::new("podman-compose")
            .current_dir("/etc/oasis/containers")
            .args([
                "up",
                "--detach",
                "--remove-orphans",
                "--force-recreate",
                "--no-build",
            ])
            .envs(
                initial
                    .environment
                    .iter()
                    .map(|(key, value)| (key.clone(), value.as_str()))
                    .chain(initial.secrets.iter().map(|(name, value)| {
                        (name.to_uppercase().replace(' ', "_"), value.as_str())
                    })),
            )
            .status()?;
    anyhow::ensure!(status.success(), "failed to bring up containers");

    // Follow container logs.
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    Command::new("podman-compose")
        .args(["logs", "--follow", "--since", &format!("{now}")])
        .current_dir("/etc/oasis/containers")
        .spawn()?;

    Ok(())
}
