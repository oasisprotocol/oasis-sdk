//! Wrapper to make development of ROFL components easier.
use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use base64::prelude::*;
use tokio::sync::mpsc;

use crate::{
    core::{
        app,
        common::version,
        config::Config,
        consensus::{roothash, verifier::TrustRoot},
        dispatcher::{PostInitState, PreInitState},
        start_runtime,
    },
    crypto,
    types::transaction,
};

pub mod client;
mod env;
pub mod init;
mod notifier;
pub mod prelude;
mod processor;
mod registration;
mod watchdog;

pub use crate::modules::rofl::app_id::AppId;
pub use client::Client;
pub use env::Environment;

/// ROFL component application.
#[allow(unused_variables)]
#[async_trait]
pub trait App: Send + Sync + 'static {
    /// ROFL application version.
    const VERSION: version::Version;

    /// Identifier of the application (used for registrations).
    fn id() -> AppId {
        // By default we fetch the application identifier from the build-time environment.
        #[allow(clippy::option_env_unwrap)]
        AppId::from_bech32(
            option_env!("ROFL_APP_ID").expect("Override App::id or specify ROFL_APP_ID."),
        )
        .expect("Corrupted ROFL_APP_ID (must be Bech32-encoded ROFL app ID).")
    }

    /// Return the consensus layer trust root for this runtime; if `None`, consensus layer integrity
    /// verification will not be performed.
    fn consensus_trust_root() -> Option<TrustRoot> {
        // By default we fetch the trust root from the build-time environment.
        option_env!("ROFL_CONSENSUS_TRUST_ROOT").map(|raw_trust_root| {
            // Parse from base64-encoded CBOR.
            cbor::from_slice(
                &BASE64_STANDARD
                    .decode(raw_trust_root)
                    .expect("Corrupted ROFL_CONSENSUS_TRUST_ROOT (must be Base64-encoded CBOR)."),
            )
            .expect("Corrupted ROFL_CONSENSUS_TRUST_ROOT (must be Base64-encoded CBOR).")
        })
    }

    /// Create a new unsigned transaction.
    fn new_transaction<B>(&self, method: &str, body: B) -> transaction::Transaction
    where
        B: cbor::Encode,
    {
        let mut tx = transaction::Transaction::new(method, body);
        // Make the ROFL module resolve the payer for all of our transactions.
        tx.set_fee_proxy("rofl", Self::id().as_ref());
        tx
    }

    /// Fetches custom app instance metadata that is included in its on-chain registration.
    ///
    /// This method is called before each registration refresh. Returning an error will not block
    /// registration, rather it will result in the metadata being cleared.
    async fn get_metadata(
        self: Arc<Self>,
        env: Environment<Self>,
    ) -> Result<BTreeMap<String, String>>
    where
        Self: Sized,
    {
        Ok(BTreeMap::new())
    }

    /// Custom post-registration initialization. It runs before any image-specific scripts are
    /// called by the runtime so it can be used to do things like set up custom storage after
    /// successful registration.
    ///
    /// Until this function completes, no further initialization will happen.
    async fn post_registration_init(self: Arc<Self>, env: Environment<Self>)
    where
        Self: Sized,
    {
        // Default implementation just runs the trivial initialization.
        init::post_registration_init();
    }

    /// Main application processing loop.
    async fn run(self: Arc<Self>, env: Environment<Self>)
    where
        Self: Sized,
    {
        // Default implementation does nothing.
    }

    /// Logic that runs on each runtime block. Only one of these will run concurrently.
    async fn on_runtime_block(self: Arc<Self>, env: Environment<Self>, round: u64)
    where
        Self: Sized,
    {
        // Default implementation does nothing.
    }

    /// Start the application.
    fn start(self)
    where
        Self: Sized,
    {
        start_runtime(
            Box::new(|state: PreInitState<'_>| -> PostInitState {
                // Fetch host information and configure domain separation context.
                let hi = state.protocol.get_host_info();
                crypto::signature::context::set_chain_context(
                    hi.runtime_id,
                    &hi.consensus_chain_context,
                );

                PostInitState {
                    app: Some(Box::new(AppWrapper::new(self, &state))),
                    ..Default::default()
                }
            }),
            Config {
                version: Self::VERSION,
                trust_root: Self::consensus_trust_root(),
                ..Default::default()
            },
        );
    }
}

struct AppWrapper {
    cmdq: mpsc::Sender<processor::Command>,
}

impl AppWrapper {
    fn new<A>(app: A, state: &PreInitState<'_>) -> Self
    where
        A: App,
    {
        Self {
            cmdq: processor::Processor::start(app, state),
        }
    }
}

#[async_trait]
impl app::App for AppWrapper {
    async fn on_runtime_block(&self, blk: &roothash::AnnotatedBlock) -> Result<()> {
        self.cmdq
            .send(processor::Command::ProcessRuntimeBlock(blk.clone()))
            .await?;
        Ok(())
    }

    async fn on_runtime_event(
        &self,
        _blk: &roothash::AnnotatedBlock,
        _tags: &[Vec<u8>],
    ) -> Result<()> {
        Ok(())
    }
}
