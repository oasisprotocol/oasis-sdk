use std::{
    fs::{self, File},
    io::Write,
    process::Command,
    time::SystemTime,
};

use anyhow::Result;
use cmd_lib::run_cmd;

use crate::{metadata, utils::RemoveFileOnDrop};

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
    // Initialize the file with environment variables to expose to podman-compose.
    let mut env_file = File::create("/run/podman/env")?;
    for (key, value) in initial.environment {
        writeln!(&mut env_file, "{key}={value}")?;
    }

    // Initialize secrets.
    for (name, value) in initial.secrets {
        // Create a new Podman secret in temporary storage on /run to avoid it being persisted.
        let _ = run_cmd!(echo -n $value | podman secret create --driver-opts file=/run/podman/secrets --replace $name -);
        // Also store the secrets in the environment file.
        let name_upper = name.to_uppercase().replace(" ", "_");
        let value = value.as_str();
        writeln!(&mut env_file, "{name_upper}={value}")?;
    }

    drop(env_file); // Close the file.
    let _guard = RemoveFileOnDrop::new("/run/podman/env");

    // Run the podman API service.
    Command::new("podman")
        .args(["system", "service", "--time=0", "unix:///run/podman.sock"])
        .spawn()?;

    // Bring containers up.
    run_cmd!(
        cd "/etc/oasis/containers";
        podman-compose --env-file "/run/podman/env" up --detach --remove-orphans --force-recreate --no-build;
    )?;

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
