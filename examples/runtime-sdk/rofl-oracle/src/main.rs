use oasis_runtime_sdk::modules::rofl::app::prelude::*;

struct OracleApp;

#[async_trait]
impl App for OracleApp {
    /// Application version.
    const VERSION: Version = sdk::version_from_cargo!();

    /// Identifier of the application (used for registrations).
    ///
    /// Here we use an application identifier that was set at genesis to make tests simpler. In
    /// practice one would use the application identifier assigned when creating a ROFL app via the
    /// `rofl.Create` call.
    fn id() -> AppId {
        "rofl1qr98wz5t6q4x8ng6a5l5v7rqlx90j3kcnun5dwht".into() // TODO: Replace with your application ID.
    }

    /// Return the consensus layer trust root for this runtime; if `None`, consensus layer integrity
    /// verification will not be performed.
    fn consensus_trust_root() -> Option<TrustRoot> {
        // The trust root below is for Sapphire Testnet at consensus height 22110615.
        Some(TrustRoot {
            height: 22110615,
            hash: "95d1501f9cb88619050a5b422270929164ce739c5d803ed9500285b3b040985e".into(),
            runtime_id: "000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c".into(),
            chain_context: "0b91b8e4e44b2003a7c5e23ddadb5e14ef5345c0ebcb3ddcae07fa2f244cab76"
                .to_string(),
        })
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

impl OracleApp {
    /// Fetch stuff from remote service via HTTPS and publish it on chain.
    async fn run_oracle(self: Arc<Self>, env: Environment<Self>) -> Result<()> {
        // TODO: Fetch stuff from remote service.
        //let observation = components_ronl::oracle::types::Observation { value: 42, ts: 0 };
        //let tx = self.new_transaction("oracle.Observe", observation);

        // Submit observation on chain.
        //env.client().sign_and_submit_tx(env.signer(), tx).await?;

        Ok(())
    }
}

fn main() {
    OracleApp.start();
}
