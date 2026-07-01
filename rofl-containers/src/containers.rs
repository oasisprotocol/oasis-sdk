use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    process::Command,
    sync::{LazyLock, Mutex},
    time::SystemTime,
};

use anyhow::Result;
use cmd_lib::run_cmd;

use crate::utils::RemoveFileOnDrop;

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

    // Migrate existing containers if needed.
    run_cmd!(
        podman system migrate;
        podman system prune --external;
        podman image prune --all --force;
    )?;

    fs::create_dir_all("/run/podman")?;

    Ok(())
}

/// Start containers.
pub async fn start() -> Result<()> {
    // Initialize the file with environment variables to expose to podman-compose.
    let mut env_file = File::create("/run/podman/env")?;
    for (key, value) in env().get() {
        writeln!(&mut env_file, "{key}={value}")?;
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

/// Extract environment variables from deployment metadata.
pub fn env_from_metadata(metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
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

static GLOBAL_ENVIRONMENT: LazyLock<Environment> = LazyLock::new(Environment::new);

/// Management of environment variables to expose to the compose file.
pub fn env() -> &'static Environment {
    &GLOBAL_ENVIRONMENT
}

/// Management of environment variables to expose to the compose file.
pub struct Environment {
    vars: Mutex<BTreeMap<String, String>>,
}

impl Environment {
    fn new() -> Self {
        Self {
            vars: Mutex::new(BTreeMap::new()),
        }
    }

    /// Set the given environment variable.
    pub fn set(&self, key: &str, value: &str) {
        let mut vars = self.vars.lock().unwrap();
        vars.insert(key.to_string(), value.to_string());
    }

    fn get(&self) -> BTreeMap<String, String> {
        let vars = self.vars.lock().unwrap();
        vars.clone()
    }
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
