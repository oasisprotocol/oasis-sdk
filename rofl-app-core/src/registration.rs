use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use tokio::sync::mpsc;

use oasis_runtime_sdk::{
    core::{
        common::logger::get_logger,
        consensus::{
            beacon::EpochTime, state::beacon::ImmutableState as BeaconState, verifier::Verifier,
        },
        host::attestation::{AttestLabelsRequest, LabelAttestation},
    },
    modules::rofl::types::{AppInstanceQuery, Register, Registration},
};
use oasis_runtime_sdk_rofl_market::policy::{
    ProviderAttestation, LABEL_PROVIDER, METADATA_KEY_POLICY_PROVIDER_ATTESTATION,
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

        let mut last_registration_epoch: Option<EpochTime> = None;

        while self.notify.recv().await.is_some() {
            let backoff = backoff::ExponentialBackoff::default();

            let result = backoff::future::retry(backoff, async || {
                let result = self.refresh_registration(last_registration_epoch).await;
                if let Err(ref err) = result {
                    slog::error!(self.logger, "failed to refresh registration";
                        "err" => ?err,
                    );
                }

                result.map_err(backoff::Error::transient)
            })
            .await;

            match result {
                Ok(epoch) => last_registration_epoch = Some(epoch),
                Err(_) => continue,
            }
        }

        slog::info!(self.logger, "registration task stopped");
    }

    /// Perform application registration refresh.
    ///
    /// On success, it returns the epoch for which the registration was refreshed.
    async fn refresh_registration(
        &self,
        last_registration_epoch: Option<EpochTime>,
    ) -> Result<EpochTime> {
        // Determine current epoch.
        let state = self.state.consensus_verifier.latest_state().await?;
        let epoch = tokio::task::spawn_blocking(move || {
            let beacon = BeaconState::new(&state);
            beacon.epoch()
        })
        .await??;

        // Skip refresh in case epoch has not changed.
        if last_registration_epoch == Some(epoch) {
            return Ok(epoch);
        }

        // Query our current registration and see if we need to update it.
        let round = self.env.client().latest_round().await?;
        if let Ok(existing) = self
            .env
            .client()
            .query::<_, Registration>(
                round,
                "rofl.AppInstance",
                AppInstanceQuery {
                    app: A::id(),
                    rak: self.state.identity.public_rak().into(),
                },
            )
            .await
        {
            // Check if we already registered for this epoch by comparing expiration.
            if existing.expiration >= epoch + 2 {
                slog::info!(self.logger, "registration already refreshed"; "epoch" => epoch);

                self.env
                    .send_command(processor::Command::RegistrationRefreshed)
                    .await?;
                return Ok(epoch);
            }
        }

        slog::info!(self.logger, "refreshing registration";
            "last_registration_epoch" => last_registration_epoch,
            "epoch" => epoch,
        );

        let mut metadata = match self.state.app.clone().get_metadata(self.env.clone()).await {
            Ok(metadata) => metadata,
            Err(err) => {
                slog::error!(self.logger, "failed to get instance metadata"; "err" => ?err);
                // Do not prevent registration, just clear metadata.
                Default::default()
            }
        };

        // Include provider-specific metadata if available.
        if let Err(err) = self.collect_provider_metadata(&mut metadata).await {
            slog::error!(self.logger, "failed to collect provider metadata"; "err" => ?err);
        }

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
                    timeout: Some(Duration::from_millis(60_000)),
                    encrypt: false, // Needed for initial fee payments.
                    ..Default::default()
                },
            )
            .await?
            .ok()?;

        slog::info!(self.logger, "refreshed registration"; "result" => ?result);

        if last_registration_epoch.is_none() {
            // If this is the first registration, notify processor that initial registration has
            // been completed so it can do other stuff.
            self.env
                .send_command(processor::Command::InitialRegistrationCompleted)
                .await?;
        }

        // Notify about registration refresh.
        self.env
            .send_command(processor::Command::RegistrationRefreshed)
            .await?;

        Ok(epoch)
    }

    async fn collect_provider_metadata(
        &self,
        metadata: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let rsp = self
            .env
            .host()
            .attestation()
            .attest_labels(AttestLabelsRequest {
                labels: vec![LABEL_PROVIDER.to_string()],
            })
            .await?;

        // Decode the attestation to check if the provider label is set and skip setting
        // metadata in case it is not.
        let la: LabelAttestation = cbor::from_slice(&rsp.attestation)?;
        match la.labels.get(LABEL_PROVIDER) {
            None => return Ok(()),
            Some(value) if value.is_empty() => return Ok(()),
            _ => {}
        }

        let pa = ProviderAttestation {
            label_attestation: rsp.attestation,
            signature: rsp.signature,
        };

        let pa = BASE64_STANDARD.encode(cbor::to_vec(pa));
        metadata.insert(METADATA_KEY_POLICY_PROVIDER_ATTESTATION.to_string(), pa);

        Ok(())
    }
}
