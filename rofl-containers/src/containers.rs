use std::{process::Command, time::SystemTime};

use anyhow::Result;
use cmd_lib::run_cmd;

/// Initialize container environment.
pub async fn init() -> Result<()> {
    // Setup networking.
    run_cmd!(
        mount none -t tmpfs "/tmp";
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

    // Migrate existing containers if needed.
    run_cmd!(
        podman system migrate;
        podman system prune --external;
        podman image prune --all --force;
    )?;

    Ok(())
}

/// Start containers.
pub async fn start() -> Result<()> {
    // Bring containers up.
    run_cmd!(
        cd "/etc/oasis/containers";
        podman-compose --env-file "/run/podman/secrets.env" up --detach --remove-orphans --force-recreate --no-build;
    )?;

    // Follow container logs.
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    Command::new("podman-compose")
        .args(["logs", "--follow", "--since", &format!("{}", now)])
        .current_dir("/etc/oasis/containers")
        .spawn()?;

    Ok(())
}
