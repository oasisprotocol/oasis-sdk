use anyhow::Result;
use tokio::{sync::mpsc, time};

use crate::core::common::{logger::get_logger, process};

/// Interval in which at least one keep-alive must be delivered to avoid the watchdog from
/// terminating the application.
const WATCHDOG_TRIGGER_INTERVAL: u64 = 6 * 3600; // 6 hours

/// Application watchdog task.
pub(super) struct Task {
    imp: Option<Impl>,
    tx: mpsc::Sender<()>,
}

impl Task {
    /// Create an application watchdog task.
    pub(super) fn new() -> Self {
        let (tx, rx) = mpsc::channel(16);

        let imp = Impl {
            logger: get_logger("modules/rofl/app/watchdog"),
            notify: rx,
        };

        Self { imp: Some(imp), tx }
    }

    /// Start the application watchdog task.
    pub(super) fn start(&mut self) {
        if let Some(imp) = self.imp.take() {
            imp.start();
        }
    }

    /// Notify the watchdog that we are still alive.
    pub(super) async fn keep_alive(&self) -> Result<()> {
        self.tx.send(()).await?;
        Ok(())
    }
}

struct Impl {
    logger: slog::Logger,

    notify: mpsc::Receiver<()>,
}

impl Impl {
    /// Start the application watchdog task.
    pub(super) fn start(self) {
        tokio::task::spawn(self.run());
    }

    /// Run the application watchdog task.
    async fn run(mut self) {
        slog::info!(self.logger, "starting watchdog task");

        loop {
            tokio::select! {
                Some(()) = self.notify.recv() => {
                    // Keep-alive received, reset watchdog.
                },

                _ = time::sleep(time::Duration::from_secs(WATCHDOG_TRIGGER_INTERVAL)) => {
                    // Watchdog triggered, kill the process.
                    slog::error!(self.logger, "keep-alive not received, terminating application");
                    process::abort();
                },

                else => break,
            }
        }

        slog::info!(self.logger, "watchdog task stopped");
    }
}
