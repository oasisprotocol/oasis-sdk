use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::core::common::logger::get_logger;

use super::{processor, App, Environment};

/// Notification to deliver to the application.
pub(super) enum Notify {
    RuntimeBlock(u64),
    RuntimeBlockDone,
    InitialRegistrationCompleted,
}

#[derive(Default)]
struct NotifyState {
    pending: bool,
    running: bool,
}

/// Application notifier task.
pub(super) struct Task<A: App> {
    imp: Option<Impl<A>>,
    tx: mpsc::Sender<Notify>,
}

impl<A> Task<A>
where
    A: App,
{
    /// Create an application notifier task.
    pub(super) fn new(state: Arc<processor::State<A>>, env: Environment<A>) -> Self {
        let (tx, rx) = mpsc::channel(16);

        let imp = Impl {
            state,
            env,
            logger: get_logger("modules/rofl/app/notifier"),
            notify: rx,
            notify_tx: tx.downgrade(),
        };

        Self { imp: Some(imp), tx }
    }

    /// Start the application notifier task.
    pub(super) fn start(&mut self) {
        if let Some(imp) = self.imp.take() {
            imp.start();
        }
    }

    /// Deliver a notification.
    pub(super) async fn notify(&self, notification: Notify) -> Result<()> {
        self.tx.send(notification).await?;
        Ok(())
    }
}

struct Impl<A: App> {
    state: Arc<processor::State<A>>,
    env: Environment<A>,
    logger: slog::Logger,

    notify: mpsc::Receiver<Notify>,
    notify_tx: mpsc::WeakSender<Notify>,
}

impl<A> Impl<A>
where
    A: App,
{
    /// Start the application notifier task.
    pub(super) fn start(self) {
        tokio::task::spawn(self.run());
    }

    /// Run the application notifier task.
    async fn run(mut self) {
        slog::info!(self.logger, "starting notifier task");

        // Pending notifications.
        let mut registered = false;
        let mut block = NotifyState::default();
        let mut last_round = 0;

        while let Some(notification) = self.notify.recv().await {
            match notification {
                Notify::RuntimeBlock(round) if registered => {
                    block.pending = true;
                    last_round = round;
                }
                Notify::RuntimeBlock(_) => continue, // Skip blocks before registration.
                Notify::RuntimeBlockDone => block.running = false,
                Notify::InitialRegistrationCompleted => registered = true,
            }

            // Don't do anything unless registered.
            if !registered {
                continue;
            }

            // Block notifications.
            if block.pending && !block.running {
                block.pending = false;
                block.running = true;

                let notify_tx = self.notify_tx.clone();
                let app = self.state.app.clone();
                let env = self.env.clone();

                tokio::spawn(async move {
                    app.on_runtime_block(env, last_round).await;
                    if let Some(tx) = notify_tx.upgrade() {
                        let _ = tx.send(Notify::RuntimeBlockDone).await;
                    }
                });
            }
        }

        slog::info!(self.logger, "notifier task stopped");
    }
}
