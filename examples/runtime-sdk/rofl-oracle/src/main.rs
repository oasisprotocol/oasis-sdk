use oasis_runtime_sdk::modules::rofl::app::prelude::*;

/// Address where the oracle contract is deployed.
// #region oracle-contract-address
const ORACLE_CONTRACT_ADDRESS: &str = "0x5FbDB2315678afecb367f032d93F642f64180aa3"; // TODO: Replace with your contract address.
// #endregion oracle-contract-address

struct OracleApp;

#[async_trait]
impl App for OracleApp {
    /// Application version.
    const VERSION: Version = sdk::version_from_cargo!();

    /// Identifier of the application (used for registrations).
    // #region app-id
    fn id() -> AppId {
        "rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf".into() // TODO: Replace with your application ID.
    }
    // #endregion app-id

    /// Return the consensus layer trust root for this runtime; if `None`, consensus layer integrity
    /// verification will not be performed (e.g. Localnet).
    // #region consensus-trust-root
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
    // #endregion consensus-trust-root

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
        // Fetch data from remote service.
        let observation = tokio::task::spawn_blocking(move || -> Result<_> {
            // Request some data from Coingecko API.
            let rsp: serde_json::Value = rofl_utils::https::agent()
                .get("https://www.binance.com/api/v3/ticker/price?symbol=ROSEUSDT")
                .call()?
                .body_mut()
                .read_json()?;

            // Extract price and convert to integer.
            let price = rsp
                .pointer("/price")
                .ok_or(anyhow::anyhow!("price not available"))?
                .as_str().unwrap()
                .parse::<f64>()?;
            let price = (price * 1_000_000.0) as u128;

            Ok(price)
        }).await??;

        // Prepare the oracle contract call.
        let mut tx = self.new_transaction(
            "evm.Call",
            module_evm::types::Call {
                address: ORACLE_CONTRACT_ADDRESS.parse().unwrap(),
                value: 0.into(),
                data: [
                    ethabi::short_signature("submitObservation", &[ethabi::ParamType::Uint(128)])
                        .to_vec(),
                    ethabi::encode(&[ethabi::Token::Uint(observation.into())]),
                ]
                .concat(),
            },
        );
        tx.set_fee_gas(200_000);

        // Submit observation on chain.
        env.client().sign_and_submit_tx(env.signer(), tx).await?;

        Ok(())
    }
}

fn main() {
    OracleApp.start();
}
