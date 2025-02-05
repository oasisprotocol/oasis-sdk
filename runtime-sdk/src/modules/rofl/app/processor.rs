use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::{mpsc, oneshot};

use crate::{
    core::{
        common::logger::get_logger,
        consensus::{roothash, verifier::Verifier},
        dispatcher::PreInitState,
        host::{self, Host as _},
        identity::Identity,
        protocol::Protocol,
    },
    crypto::signature::{secp256k1, Signer},
};
use rand::rngs::OsRng;

use super::{notifier, registration, watchdog, App, Environment};

/// Size of the processor command queue.
const CMDQ_BACKLOG: usize = 32;

/// Command sent to the processor task.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum Command {
    /// Process a notification of a new runtime block.
    ProcessRuntimeBlock(roothash::AnnotatedBlock),
    /// Retrieve the latest known round.
    GetLatestRound(oneshot::Sender<u64>),
    /// Notification that initial registration has been completed.
    InitialRegistrationCompleted,
    /// Registration refreshed.
    RegistrationRefreshed,
}

/// Processor state.
pub(super) struct State<A: App> {
    pub(super) identity: Arc<Identity>,
    pub(super) host: Arc<Protocol>,
    pub(super) consensus_verifier: Arc<dyn Verifier>,
    pub(super) signer: Arc<dyn Signer>,
    pub(super) app: Arc<A>,
}

struct Tasks<A: App> {
    registration: registration::Task<A>,
    notifier: notifier::Task<A>,
    watchdog: watchdog::Task,
}

/// Processor.
pub(super) struct Processor<A: App> {
    state: Arc<State<A>>,
    env: Environment<A>,
    tasks: Tasks<A>,
    cmdq: mpsc::Receiver<Command>,
    logger: slog::Logger,

    latest_round: u64,
}

impl<A> Processor<A>
where
    A: App,
{
    /// Create and start a new processor.
    pub(super) fn start(app: A, state: &PreInitState<'_>) -> mpsc::Sender<Command> {
        // Create the command channel.
        let (tx, rx) = mpsc::channel(CMDQ_BACKLOG);

        // Provision keys. Currently we provision a random key for signing transactions to avoid
        // using the RAK directly as the RAK is an Ed25519 key which cannot easily be used for EVM
        // calls due to the limitations of the current implementation.
        let signer = secp256k1::MemorySigner::random(&mut OsRng).unwrap();

        // Prepare state.
        let state = Arc::new(State {
            identity: state.identity.clone(),
            host: state.protocol.clone(),
            consensus_verifier: state.consensus_verifier.clone(),
            signer: Arc::new(signer),
            app: Arc::new(app),
        });

        // Prepare application environment.
        let env = Environment::new(state.clone(), tx.downgrade());

        // Create the processor and start it.
        let processor = Self {
            tasks: Tasks {
                registration: registration::Task::new(state.clone(), env.clone()),
                notifier: notifier::Task::new(state.clone(), env.clone()),
                watchdog: watchdog::Task::new(),
            },
            state,
            env,
            cmdq: rx,
            logger: get_logger("modules/rofl/app"),
            latest_round: 0,
        };
        tokio::spawn(processor.run());

        tx
    }

    /// Run the processor.
    async fn run(mut self) {
        slog::info!(self.logger, "starting processor";
            "app_id" => A::id(),
        );

        // Register for notifications.
        if let Err(err) = self
            .state
            .host
            .register_notify(host::RegisterNotifyOpts {
                runtime_block: true,
                runtime_event: vec![],
            })
            .await
        {
            slog::error!(self.logger, "failed to register for notifications";
                "err" => ?err,
            );
        }

        // Start the tasks.
        self.tasks.registration.start();
        self.tasks.notifier.start();
        self.tasks.watchdog.start();

        slog::info!(self.logger, "entering processor loop");
        while let Some(cmd) = self.cmdq.recv().await {
            if let Err(err) = self.process(cmd).await {
                slog::error!(self.logger, "failed to process command";
                    "err" => ?err,
                );
            }
        }

        slog::info!(self.logger, "processor stopped");
    }

    /// Process a command.
    async fn process(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::ProcessRuntimeBlock(blk) => self.cmd_process_runtime_block(blk).await,
            Command::GetLatestRound(ch) => self.cmd_get_latest_round(ch).await,
            Command::InitialRegistrationCompleted => {
                self.cmd_initial_registration_completed().await
            }
            Command::RegistrationRefreshed => self.tasks.watchdog.keep_alive().await,
        }
    }

    async fn cmd_process_runtime_block(&mut self, blk: roothash::AnnotatedBlock) -> Result<()> {
        // Update latest known round.
        if blk.block.header.round <= self.latest_round {
            return Err(anyhow!("round seems to have moved backwards"));
        }
        self.latest_round = blk.block.header.round;

        // Notify registration task.
        self.tasks.registration.refresh();
        // Notify notifier task.
        let _ = self
            .tasks
            .notifier
            .notify(notifier::Notify::RuntimeBlock(self.latest_round))
            .await;

        Ok(())
    }

    async fn cmd_get_latest_round(&self, ch: oneshot::Sender<u64>) -> Result<()> {
        let _ = ch.send(self.latest_round);
        Ok(())
    }

    async fn cmd_initial_registration_completed(&self) -> Result<()> {
        slog::info!(self.logger, "initial registration completed");

        // Start application after first registration.
        slog::info!(self.logger, "starting application");
        tokio::spawn(self.state.app.clone().run(self.env.clone()));

        // Perform post-registration initialization.
        let app = self.state.app.clone();
        let env = self.env.clone();
        let logger = self.logger.clone();
        tokio::spawn(async move {
            slog::info!(
                logger,
                "performing app-specific post-registration initialization"
            );

            app.post_registration_init(env).await;
        });

        // Notify notifier task.
        self.tasks
            .notifier
            .notify(notifier::Notify::InitialRegistrationCompleted)
            .await?;

        Ok(())
    }
}
