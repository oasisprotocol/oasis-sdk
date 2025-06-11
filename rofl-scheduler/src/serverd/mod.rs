//! Client-accessible scheduler endpoint for managing their instances.
mod auth;
mod error;
mod routes;
pub mod tls;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{routing::post, Router};

use oasis_runtime_sdk::{modules::rofl::app::prelude::*, types::address::Address};

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
}

pub struct State {
    pub env: Environment<SchedulerApp>,
    pub manager: Arc<Manager>,
    pub domain: String,
    pub provider: Address,
}

/// Start the server endpoint in a background task.
pub async fn serve(cfg: Config<'_>) -> Result<()> {
    let tls_provisioner = tls::CertificateProvisioner::new(cfg.domain);
    let state = Arc::new(State {
        env: cfg.env,
        manager: cfg.manager,
        domain: cfg.domain.to_string(),
        provider: cfg.config.provider_address,
    });
    let app = Router::new()
        .route("/rofl-scheduler/v1/auth/login", post(auth::login))
        .route("/rofl-scheduler/v1/logs/get", post(routes::logs_get))
        .with_state(state);

    let addr = cfg.address.parse().context("bad address")?;
    let tls_cfg =
        axum_server::tls_rustls::RustlsConfig::from_config(tls_provisioner.server_config());

    tokio::spawn(tls_provisioner.provision());
    tokio::spawn(axum_server::bind_rustls(addr, tls_cfg).serve(app.into_make_service()));

    Ok(())
}
