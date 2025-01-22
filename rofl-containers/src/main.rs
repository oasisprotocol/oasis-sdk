//! The rofl-containers runtime is a generic ROFL app that is used when building all TDX
//! container-based ROFL apps (e.g. using the Oasis CLI).
//!
//! It expects `ROFL_APP_ID` and `ROFL_CONSENSUS_TRUST_ROOT` to be passed via environment variables.
//! Usually these would be set in the kernel command-line so that they are part of the runtime
//! measurements.
//!
//! It currently just starts a REST API server (rofl-appd) that exposes information about the
//! application together with a simple KMS interface. In the future it will also manage secrets and
//! expose other interfaces.
use std::env;

use base64::prelude::*;
use oasis_runtime_sdk::{
    cbor,
    core::common::{logger::get_logger, process},
    modules::rofl::app::prelude::*,
};
use rofl_appd::services;

mod containers;
mod reaper;
mod secrets;
mod storage;

/// UNIX socket address where the REST API server will listen on.
const ROFL_APPD_ADDRESS: &str = "unix:/run/rofl-appd.sock";

struct ContainersApp;

#[async_trait]
impl App for ContainersApp {
    const VERSION: Version = sdk::version_from_cargo!();

    fn id() -> AppId {
        // Fetch application ID from the ROFL_APP_ID environment variable.
        // This would usually be passed via the kernel cmdline.
        AppId::from_bech32(&env::var("ROFL_APP_ID").expect("Must configure ROFL_APP_ID."))
            .expect("Corrupted ROFL_APP_ID (must be Bech32-encoded ROFL app ID).")
    }

    fn consensus_trust_root() -> Option<TrustRoot> {
        // Fetch consensus trust root from the ROFL_CONSENSUS_TRUST_ROOT environment variable.
        // This would usually be passed via the kernel cmdline.
        let raw_trust_root = env::var("ROFL_CONSENSUS_TRUST_ROOT")
            .expect("Must configure ROFL_CONSENSUS_TRUST_ROOT.");
        cbor::from_slice(
            &BASE64_STANDARD
                .decode(raw_trust_root)
                .expect("Corrupted ROFL_CONSENSUS_TRUST_ROOT (must be Base64-encoded CBOR)."),
        )
        .expect("Corrupted ROFL_CONSENSUS_TRUST_ROOT (must be Base64-encoded CBOR).")
    }

    async fn post_registration_init(self: Arc<Self>, env: Environment<Self>) {
        // Temporarily disable the default process reaper as it interferes with scripts.
        let _guard = reaper::disable_default_reaper();
        let logger = get_logger("post_registration_init");

        // Start the key management service and wait for it to initialize.
        let kms: Arc<dyn services::kms::KmsService> =
            Arc::new(services::kms::OasisKmsService::new(env.clone()));
        let kms_task = kms.clone();
        tokio::spawn(async move { kms_task.start().await });
        let _ = kms.wait_ready().await;

        // Initialize storage when configured in the kernel cmdline.
        slog::info!(logger, "initializing stage 2 storage");
        if let Err(err) = storage::init(kms.clone()).await {
            slog::error!(logger, "failed to initialize stage 2 storage"; "err" => ?err);
            process::abort();
        }

        // Start the REST API server.
        slog::info!(logger, "starting the API server");
        let cfg = rofl_appd::Config {
            address: ROFL_APPD_ADDRESS,
            kms: kms.clone(),
        };
        let appd_logger = logger.clone();
        let appd_env = env.clone();
        tokio::spawn(async move {
            if let Err(err) = rofl_appd::start(cfg, appd_env).await {
                slog::error!(appd_logger, "failed to start API server"; "err" => ?err);
                process::abort();
            }
        });

        // Initialize containers.
        slog::info!(logger, "initializing container environment");
        if let Err(err) = containers::init().await {
            slog::error!(logger, "failed to initialize container environment"; "err" => ?err);
            process::abort();
        }

        // Initialize secrets.
        slog::info!(logger, "initializing container secrets");
        if let Err(err) = secrets::init(env.clone(), kms.clone()).await {
            slog::error!(logger, "failed to initialize container secrets"; "err" => ?err);
            process::abort();
        }

        // Start containers.
        slog::info!(logger, "starting containers");
        if let Err(err) = containers::start().await {
            slog::error!(logger, "failed to start containers"; "err" => ?err);
            process::abort();
        }

        slog::info!(logger, "everything is up and running");
    }
}

fn main() {
    // Configure the binary search path.
    // SAFETY: This is safe as no other threads are running yet.
    unsafe {
        env::set_var("PATH", "/usr/sbin:/usr/bin:/sbin:/bin");
    }

    ContainersApp.start();
}
