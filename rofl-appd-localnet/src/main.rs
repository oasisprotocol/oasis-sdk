use tokio::sync::OnceCell;
use oasis_runtime_sdk::core::common::logger::get_logger;
use oasis_runtime_sdk::core::common::process;
use rofl_appd::services;
use rofl_app_core::prelude::*;

/// UNIX socket address where the REST API server will listen on.
const ROFL_APPD_ADDRESS: &str = "unix:/rofls/rofl-appd.sock";

struct AppdLocalnetApp {
    metadata: OnceCell<Arc<dyn services::metadata::MetadataService>>,
}

#[async_trait]
impl App for AppdLocalnetApp {
    /// Application version.
    const VERSION: Version = sdk::version_from_cargo!();

    async fn run(self: Arc<Self>, _env: Environment<Self>) {
        // We are running now!
        println!("Hello ROFL world!");
    }

    async fn post_registration_init(self: Arc<Self>, env: Environment<Self>) {
        let logger = get_logger("post_registration_init");

        // Start the key management service and wait for it to initialize.
        let kms: Arc<dyn services::kms::KmsService> =
            Arc::new(services::kms::OasisKmsService::new(env.clone()));
        let kms_task = kms.clone();
        tokio::spawn(async move { kms_task.start().await });
        let _ = kms.wait_ready().await;

        // Initialize the metadata service.
        let metadata_service =
            match services::metadata::OasisMetadataService::new(env.clone()).await {
                Ok(service) => Arc::new(service) as Arc<dyn services::metadata::MetadataService>,
                Err(err) => {
                    slog::error!(logger, "failed to create metadata service"; "err" => ?err);
                    process::abort();
                }
            };
        if self.metadata.set(metadata_service).is_err() {
            slog::error!(logger, "metadata service was already set");
            process::abort();
        }

        // Start the REST API server.
        slog::info!(logger, "starting the API server");
        let cfg = rofl_appd::Config {
            address: ROFL_APPD_ADDRESS,
            kms: kms.clone(),
            metadata: self
                .metadata
                .get()
                .cloned()
                .expect("Metadata service not set"),
        };
        let appd_logger = logger.clone();
        let appd_env = env.clone();
        tokio::spawn(async move {
            if let Err(err) = rofl_appd::start(cfg, appd_env).await {
                slog::error!(appd_logger, "failed to start API server"; "err" => ?err);
                process::abort();
            }
        });
    }
}

fn main() {
    let appd_localnet_app = AppdLocalnetApp {
        metadata: OnceCell::new(),
    };

    appd_localnet_app.start();
}
