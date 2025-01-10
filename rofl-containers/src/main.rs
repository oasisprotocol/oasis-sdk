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
use oasis_runtime_sdk::{cbor, modules::rofl::app::prelude::*};

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

    async fn run(self: Arc<Self>, env: Environment<Self>) {
        // Start the REST API server.
        let _ = rofl_appd::start(ROFL_APPD_ADDRESS, env).await;
    }
}

fn main() {
    ContainersApp.start();
}
