//! REST API daemon accessible by ROFL apps.

mod routes;
mod state;

use std::sync::Arc;

use rocket::{figment::Figment, routes};

use oasis_runtime_sdk::modules::rofl::app::{App, Environment};

/// Start the REST API server.
pub async fn start<A>(address: &str, env: Environment<A>) -> Result<(), rocket::Error>
where
    A: App,
{
    let env: Arc<dyn state::Env> = Arc::new(state::EnvImpl::new(env));
    let cfg = Figment::new().join(("address", address));

    rocket::custom(cfg)
        .manage(env)
        .mount("/rofl/v1/app", routes![routes::app::id,])
        .mount("/rofl/v1/keys", routes![routes::keys::generate,])
        .mount("/rofl/v1/tx", routes![routes::tx::submit,])
        .launch()
        .await?;

    Ok(())
}
