use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::{
    core::{
        common::logger::get_logger,
        consensus::{
            beacon::EpochTime, state::beacon::ImmutableState as BeaconState, verifier::Verifier,
        },
    },
    modules::rofl::types::Register,
};

use super::{client::SubmitTxOpts, processor, App, Environment};

/// Registration task.
pub(super) struct Task<A: App> {
    imp: Option<Impl<A>>,
    tx: mpsc::Sender<()>,
}

impl<A> Task<A>
where
    A: App,
{
    /// Create a registration task.
    pub(super) fn new(state: Arc<processor::State<A>>, env: Environment<A>) -> Self {
        let (tx, rx) = mpsc::channel(1);

        let imp = Impl {
            state,
            env,
            logger: get_logger("modules/rofl/app/registration"),
            notify: rx,
            last_registration_epoch: None,
        };

        Self { imp: Some(imp), tx }
    }

    /// Start the registration task.
    pub(super) fn start(&mut self) {
        if let Some(imp) = self.imp.take() {
            imp.start();
        }
    }

    /// Ask the registration task to refresh the registration.
    pub(super) fn refresh(&self) {
        let _ = self.tx.try_send(());
    }
}

struct Impl<A: App> {
    state: Arc<processor::State<A>>,
    env: Environment<A>,
    logger: slog::Logger,

    notify: mpsc::Receiver<()>,
    last_registration_epoch: Option<EpochTime>,
}

impl<A> Impl<A>
where
    A: App,
{
    /// Start the registration task.
    pub(super) fn start(self) {
        tokio::task::spawn(self.run());
    }

    /// Run the registration task.
    async fn run(mut self) {
        slog::info!(self.logger, "starting registration task");

        // TODO: Handle retries etc.
        while self.notify.recv().await.is_some() {
            if let Err(err) = self.refresh_registration().await {
                slog::error!(self.logger, "failed to refresh registration";
                    "err" => ?err,
                );
            }
        }

        slog::info!(self.logger, "registration task stopped");
    }

    /// Perform application registration refresh.
    async fn refresh_registration(&mut self) -> Result<()> {
        // Determine current epoch.
        let state = self.state.consensus_verifier.latest_state().await?;
        let epoch = tokio::task::spawn_blocking(move || {
            let beacon = BeaconState::new(&state);
            beacon.epoch()
        })
        .await??;

        // Skip refresh in case epoch has not changed.
        if self.last_registration_epoch == Some(epoch) {
            return Ok(());
        }

        slog::info!(self.logger, "refreshing registration";
            "last_registration_epoch" => self.last_registration_epoch,
            "epoch" => epoch,
        );

        let metadata = match self.state.app.clone().get_metadata(self.env.clone()).await {
            Ok(metadata) => metadata,
            Err(err) => {
                slog::error!(self.logger, "failed to get instance metadata"; "err" => ?err);
                // Do not prevent registration, just clear metadata.
                Default::default()
            }
        };

        // Refresh registration.
        let ect = self
            .state
            .identity
            .endorsed_capability_tee()
            .ok_or(anyhow!("endorsed TEE capability not available"))?;
        let register = Register {
            app: A::id(),
            ect,
            expiration: epoch + 2,
            extra_keys: vec![self.env.signer().public_key()],
            metadata,
        };

        let tx = self.state.app.new_transaction("rofl.Register", register);
        let result = self
            .env
            .client()
            .multi_sign_and_submit_tx_opts(
                &[self.state.identity.clone(), self.env.signer()],
                tx,
                SubmitTxOpts {
                    encrypt: false, // Needed for initial fee payments.
                    ..Default::default()
                },
            )
            .await?
            .ok()?;

        slog::info!(self.logger, "refreshed registration"; "result" => ?result);

        if self.last_registration_epoch.is_none() {
            // If this is the first registration, notify processor that initial registration has
            // been completed so it can do other stuff.
            self.env
                .send_command(processor::Command::InitialRegistrationCompleted)
                .await?;
        }
        self.last_registration_epoch = Some(epoch);

        // Notify about registration refresh.
        self.env
            .send_command(processor::Command::RegistrationRefreshed)
            .await?;

        Ok(())
    }
}
