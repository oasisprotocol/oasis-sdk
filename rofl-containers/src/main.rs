use std::env;

use base64::prelude::*;
use oasis_runtime_sdk::{cbor, modules::rofl::app::prelude::*};

/// A generic container-based ROFL application.
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

    async fn run(self: Arc<Self>, _env: Environment<Self>) {
        // TODO: Start the REST API server.
    }
}

fn main() {
    ContainersApp.start();
}
