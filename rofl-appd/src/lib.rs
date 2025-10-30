//! REST API daemon accessible by ROFL apps.

mod routes;
pub mod services;
pub(crate) mod state;

use std::sync::Arc;

use rocket::{figment::Figment, routes};

use rofl_app_core::{App, Environment};

/// API server configuration.
#[derive(Clone)]
pub struct Config<'a> {
    /// Address where the service should listen on.
    pub address: &'a str,
    /// Key management service to use.
    pub kms: Arc<dyn services::kms::KmsService>,
    /// App provided instance metadata service.
    pub metadata: Arc<dyn services::metadata::MetadataService>,
}

/// Start the REST API server.
pub async fn start<A>(cfg: Config<'_>, env: Environment<A>) -> Result<(), rocket::Error>
where
    A: App,
{
    // Oasis runtime environment.
    let env: Arc<dyn state::Env> = Arc::new(state::EnvImpl::new(env));

    // Server configuration.
    let rocket_cfg = Figment::from(rocket::config::Config::default())
        .select("default")
        .merge(("address", cfg.address))
        .merge(("reuse", true));

    let server = rocket::custom(rocket_cfg)
        .manage(env)
        .manage(cfg.kms)
        .mount("/rofl/v1/app", routes![routes::app::id,])
        .mount("/rofl/v1/keys", routes![routes::keys::generate,]);

    let server = server.manage(cfg.metadata).mount(
        "/rofl/v1/metadata",
        routes![routes::metadata::set, routes::metadata::get],
    );

    let server = server.mount("/rofl/v1/query", routes![routes::query::query]);

    #[cfg(feature = "tx")]
    let server = server
        .manage(routes::tx::Config::default())
        .mount("/rofl/v1/tx", routes![routes::tx::sign_and_submit]);

    server.launch().await?;

    Ok(())
}
