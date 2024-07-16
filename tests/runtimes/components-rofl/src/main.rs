use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use oasis_runtime_sdk::{
    self as sdk,
    modules::rofl::app::{App, AppId, Environment},
    Version,
};

struct TestApp;

#[async_trait]
impl App for TestApp {
    /// Application version.
    const VERSION: Version = sdk::version_from_cargo!();

    /// Identifier of the application (used for registrations).
    ///
    /// Here we use an application identifier that was set at genesis to make tests simpler. In
    /// practice one would use the application identifier assigned when creating a ROFL app via the
    /// `rofl.Create` call.
    fn id() -> AppId {
        *components_ronl::EXAMPLE_APP_ID
    }

    async fn run(self: Arc<Self>, _env: Environment<Self>) {
        // We are running now!
        println!("Hello ROFL world!");
    }

    async fn on_runtime_block(self: Arc<Self>, env: Environment<Self>, _round: u64) {
        // This gets called for each runtime block. It will not be called again until the previous
        // invocation returns and if invocation takes multiple blocks to run, those blocks will be
        // skipped.
        if let Err(err) = self.run_oracle(env).await {
            println!("Failed to submit observation: {:?}", err);
        }
    }
}

impl TestApp {
    /// Fetch stuff from remote service via HTTPS and publish it on chain.
    async fn run_oracle(self: Arc<Self>, env: Environment<Self>) -> Result<()> {
        // TODO: Fetch stuff from remote service.
        let observation = components_ronl::oracle::types::Observation { value: 42, ts: 0 };
        let tx = self.new_transaction("oracle.Observe", observation);

        // Submit observation on chain.
        env.client().sign_and_submit_tx(env.signer(), tx).await?;

        Ok(())
    }
}

fn main() {
    TestApp.start();
}
