use std::{env, sync::Arc};
use rofl_appd::{services::kms::MockKmsService, Config};
use rofl_app_core::{App, Environment};

#[derive(Default)]
struct RoflAppd;

impl App for RoflAppd {
    const VERSION: oasis_runtime_sdk::Version = oasis_runtime_sdk::version_from_cargo!();
    fn id() -> oasis_runtime_sdk::types::AppId { oasis_runtime_sdk::types::AppId::default() }
    fn consensus_trust_root() -> Option<oasis_runtime_sdk::types::TrustRoot> { None }
}

#[tokio::main]
async fn main() -> Result<(), rocket::Error> {
    let socket = env::args().nth(1).unwrap_or_else(|| "unix:/run/rofl-appd.sock".to_string());

    // Use the mock KMS and in-memory metadata service.
    // #[cfg(feature = "mock")]
    // {
        use oasis_runtime_sdk::crypto::signature::Signer;

    let config =
        Config {
            address: &socket,
            kms: Arc::new(MockKmsService),
            metadata: Arc::new(InMemoryMetadataService::default()),
            // ...
        };

        /*
        /// Application environment.
pub struct Environment<A: App> {
    app: Arc<A>,
    client: client::Client<A>,
    signer: Arc<dyn Signer>,
    identity: Arc<Identity>,
    host: Arc<Protocol>,
    cmdq: mpsc::WeakSender<processor::Command>,
}
 */     
    let env = Environment::new
       
    // }

    // #[cfg(not(feature = "mock"))]
    // {
    //     todo!("Non-mock mode is not implemented yet");
    //     let config = 
    //     Config {
    //         address: &socket,
    //         kms: Arc::new(OasisKmsService::new(env.clone())),
    //         metadata: Arc::new(OasisMetadataService::new(env.clone()).await?),
    //         // ...
    //     };
    // }


    

    println!("Starting minimal rofl-appd () at {}", socket);

    rofl_appd::start(config, env).await
}