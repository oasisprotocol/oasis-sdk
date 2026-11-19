//! REST API daemon accessible by ROFL apps.

mod routes;
pub mod services;
pub(crate) mod state;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use axum_server::{Address, Server};
use hyper_util::rt::TokioTimer;

use rofl_app_core::{App, Environment};

/// Maximum size of a request body.
const MAX_REQUEST_BODY_SIZE: usize = 1024 * 1024;
/// Timeout for reading request headers.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(15);

/// API server configuration.
#[derive(Clone)]
pub struct Config<'a> {
    /// Address where the service should listen on.
    ///
    /// Use `unix:<path>` for a UNIX socket or `<host>:<port>` for a TCP socket.
    pub address: &'a str,
    /// Key management service to use.
    pub kms: Arc<dyn services::kms::KmsService>,
    /// App provided instance metadata service.
    pub metadata: Arc<dyn services::metadata::MetadataService>,
}

/// Start the REST API server.
pub async fn start<A>(cfg: Config<'_>, env: Environment<A>) -> Result<()>
where
    A: App,
{
    let state = Arc::new(state::AppState {
        env: Arc::new(state::EnvImpl::new(env)),
        kms: cfg.kms,
        metadata: cfg.metadata,
        #[cfg(feature = "tx")]
        tx: routes::tx::Config::default(),
    });

    let app = Router::new()
        .route("/rofl/v1/app/id", get(routes::app::id))
        .route(
            "/rofl/v1/app/config/metadata",
            get(routes::app::config_metadata),
        )
        .route("/rofl/v1/keys/generate", post(routes::keys::generate))
        .route("/rofl/v1/secrets/get", post(routes::secrets::get))
        .route(
            "/rofl/v1/metadata",
            get(routes::metadata::get)
                .post(routes::metadata::set)
                .put(routes::metadata::upsert)
                .delete(routes::metadata::delete),
        )
        .route("/rofl/v1/query", post(routes::query::query));

    #[cfg(feature = "tx")]
    let app = app.route("/rofl/v1/tx/sign-submit", post(routes::tx::sign_and_submit));

    let app = app
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_SIZE))
        .with_state(state);

    serve(cfg.address, app).await
}

/// Bind to the given address and serve the API until an error occurs.
async fn serve(address: &str, app: Router) -> Result<()> {
    match address.strip_prefix("unix:") {
        Some(path) => {
            // Remove any stale socket left behind by a previous instance.
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err).context("failed to remove stale UNIX socket"),
            }

            let listener =
                tokio::net::UnixListener::bind(path).context("failed to bind UNIX socket")?;
            server_with_header_timeout::<std::os::unix::net::SocketAddr>(listener)
                .serve(app.into_make_service())
                .await?;
        }
        None => {
            let listener = tokio::net::TcpListener::bind(address)
                .await
                .context("failed to bind TCP socket")?;
            server_with_header_timeout::<std::net::SocketAddr>(listener)
                .serve(app.into_make_service())
                .await?;
        }
    }

    Ok(())
}

fn server_with_header_timeout<A: Address>(listener: A::Listener) -> Server<A> {
    let mut server = Server::<A>::from_listener(listener);
    server
        .http_builder()
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT);
    server
}
