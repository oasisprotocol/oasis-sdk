//! Client-accessible scheduler endpoint for managing their instances.
mod auth;
mod error;
mod routes;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{http, routing::post, Router};
use tower::ServiceBuilder;
use tower_http::cors;

use oasis_runtime_sdk::types::address::Address;
use rofl_app_core::prelude::*;
pub use rofl_proxy::http::tls;

use crate::{config::LocalConfig, manager::Manager, SchedulerApp};

/// Server configuration.
#[derive(Clone)]
pub struct Config<'a> {
    /// Address where the service should listen on.
    pub address: &'a str,
    /// Domain name of the domain where the endpoint will be served from.
    pub domain: &'a str,
    /// Environment.
    pub env: Environment<SchedulerApp>,
    /// Manager.
    pub manager: Arc<Manager>,
    /// Scheduler configuration.
    pub config: Arc<LocalConfig>,
    /// ACME account.
    pub acme: tls::AcmeAccount,
}

pub struct State {
    pub env: Environment<SchedulerApp>,
    pub manager: Arc<Manager>,
    pub domain: String,
    pub provider: Address,
    pub token_lifetime: u64,
}

/// Start the server endpoint in a background task.
pub async fn serve(cfg: Config<'_>) -> Result<()> {
    let tls_provisioner = tls::CertificateProvisioner::new(cfg.acme);
    tls_provisioner.handle().add_domain(cfg.domain).await;

    let state = Arc::new(State {
        env: cfg.env,
        manager: cfg.manager,
        domain: cfg.domain.to_string(),
        provider: cfg.config.provider_address,
        token_lifetime: cfg.config.api_token_lifetime,
    });

    let cors = cors::CorsLayer::new()
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_origin(cors::Any)
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);

    let app = Router::new()
        .route("/rofl-scheduler/v1/auth/login", post(auth::login))
        .route("/rofl-scheduler/v1/logs/get", post(routes::logs_get))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(cors));

    let addr = cfg.address.parse().context("bad address")?;
    let tls_cfg =
        axum_server::tls_rustls::RustlsConfig::from_config(tls_provisioner.server_config(true));

    tls_provisioner.start();
    tokio::spawn(axum_server::bind_rustls(addr, tls_cfg).serve(app.into_make_service()));

    Ok(())
}
