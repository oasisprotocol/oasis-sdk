//! Wrapper to make development of ROFL components easier.
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    core::{
        config::Config,
        consensus::roothash,
        dispatcher::{PostInitState, PreInitState},
        rofl, start_runtime,
    },
    crypto,
    types::transaction,
    Runtime,
};

mod client;
mod env;
mod notifier;
mod processor;
mod registration;

pub use crate::modules::rofl::app_id::AppId;
pub use client::Client;
pub use env::Environment;

/// ROFL component application.
#[allow(unused_variables)]
#[async_trait]
pub trait App: Send + Sync + 'static {
    /// Runtime to attach the application to.
    ///
    /// The runtime must have the ROFL module enabled so that the application can register to it.
    type AttachTo: Runtime;

    /// Identifier of the application (used for registrations).
    fn id() -> AppId;

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
                // Use the same version as the runtime we are attaching to.
                version: Self::AttachTo::VERSION,
                // Use the same trust root as the runtime we are attaching to.
                trust_root: Self::AttachTo::consensus_trust_root(),
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
impl rofl::App for AppWrapper {
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
