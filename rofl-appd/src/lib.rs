//! REST API daemon accessible by ROFL apps.

mod routes;
pub(crate) mod services;
pub(crate) mod state;

use std::sync::Arc;

use rocket::{figment::Figment, routes};

use oasis_runtime_sdk::modules::rofl::app::{App, Environment};

/// Start the REST API server.
pub async fn start<A>(address: &str, env: Environment<A>) -> Result<(), rocket::Error>
where
    A: App,
{
    // KMS service.
    let kms_service: Arc<dyn services::kms::KmsService> =
        Arc::new(services::kms::OasisKmsService::new(env.clone()));
    let kms_service_task = kms_service.clone();
    tokio::spawn(async move { kms_service_task.start().await });

    // Oasis runtime environment.
    let env: Arc<dyn state::Env> = Arc::new(state::EnvImpl::new(env));

    // Server configuration.
    let cfg = Figment::new().join(("address", address));

    rocket::custom(cfg)
        .manage(env)
        .manage(kms_service)
        .mount("/rofl/v1/app", routes![routes::app::id,])
        .mount("/rofl/v1/keys", routes![routes::keys::generate,])
        .launch()
        .await?;

    Ok(())
}
