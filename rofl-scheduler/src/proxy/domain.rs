use std::{sync::Weak, time::Duration};

use anyhow::{anyhow, Result};
use backoff::backoff::Backoff;
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

use oasis_runtime_sdk_rofl_market::types::InstanceId;
use rofl_app_core::prelude::*;

/// Trait for receiving domain verification notifications.
///
/// This trait should be implemented by components that need to be notified when
/// domain verification completes (successfully or with failure).
#[async_trait]
pub trait CustomDomainVerifierNotifier: Send + Sync {
    /// Called when a domain verification completes successfully.
    async fn verification_completed(&self, id: InstanceId, domain: &str);
}

/// Handle that can be used to cancel the verification.
pub struct DomainVerificationHandle;

impl DomainVerificationHandle {
    /// Creates a new domain verification handle.
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

/// Represents a single domain verification request.
///
/// This struct contains all the information needed to verify that a domain
/// has the correct TXT record for the given instance.
#[derive(Clone)]
pub struct DomainVerification {
    /// The instance ID that requested this domain verification.
    instance_id: InstanceId,
    /// The domain name to verify.
    domain: String,
    /// The token to verify.
    token: String,
    /// When this verification should be retried next.
    retry_at: Option<Instant>,
    /// Exponential backoff for retrying the verification.
    retry_backoff: Option<backoff::ExponentialBackoff>,
    /// Weak reference to the handle that can be used to cancel the verification.
    handle: Weak<DomainVerificationHandle>,
}

impl DomainVerification {
    /// Create a new domain verification instance.
    ///
    /// When all strong instances of `handle` are dropped, the verification will be canceled.
    pub fn new(
        instance_id: InstanceId,
        domain: &str,
        token: &str,
        handle: &Arc<DomainVerificationHandle>,
    ) -> Self {
        Self {
            instance_id,
            domain: domain.to_owned(),
            token: token.to_owned(),
            retry_at: None,
            retry_backoff: None,
            handle: Arc::downgrade(handle),
        }
    }

    /// Calculate the next retry time using exponential backoff.
    ///
    /// Returns `true` if the retry was scheduled successfully, `false` if no further retries
    /// should be attempted.
    fn schedule_retry(&mut self) -> bool {
        if self.is_cancelled() {
            return false;
        }

        let retry_backoff = self.retry_backoff.get_or_insert_with(|| {
            backoff::ExponentialBackoffBuilder::new()
                .with_max_elapsed_time(Some(Duration::from_secs(60)))
                .build()
        });

        match retry_backoff.next_backoff() {
            Some(next) => {
                self.retry_at = Some(Instant::now() + next);
                true
            }
            None => false,
        }
    }

    /// Check if the verification request has been cancelled.
    fn is_cancelled(&self) -> bool {
        self.handle.upgrade().is_none()
    }
}

/// Custom domain verifier with non-blocking exponential backoff retry mechanism.
pub struct CustomDomainVerifier {
    /// Number of worker tasks to spawn.
    workers: usize,
    /// Channel for sending new verification requests to workers.
    work_tx: tokio_mpmc::Sender<DomainVerification>,
    /// Channel for receiving verification requests (consumed on start).
    work_rx: Option<tokio_mpmc::Receiver<DomainVerification>>,
    /// Notifier for domain verification events.
    notifier: Arc<dyn CustomDomainVerifierNotifier>,
    /// Logger for this verifier.
    logger: slog::Logger,
}

impl CustomDomainVerifier {
    /// Create a new custom domain verifier.
    pub fn new(
        workers: usize,
        notifier: Arc<dyn CustomDomainVerifierNotifier>,
        logger: slog::Logger,
    ) -> Self {
        let (work_tx, work_rx) = tokio_mpmc::channel(1024);

        Self {
            workers,
            work_tx,
            work_rx: Some(work_rx),
            notifier,
            logger,
        }
    }

    /// Queue a new domain verification request.
    ///
    /// When all strong instances of `handle` are dropped, the verification will be canceled.
    pub async fn queue_verification(
        &self,
        id: InstanceId,
        domain: &str,
        token: &str,
        handle: &Arc<DomainVerificationHandle>,
    ) -> Result<()> {
        self.work_tx
            .send(DomainVerification::new(id, domain, token, handle))
            .await
            .map_err(|_| anyhow!("failed to queue domain verification"))?;
        Ok(())
    }

    /// Start the domain verifier.
    pub fn start(&mut self) {
        if let Some(cmd_rx) = self.work_rx.take() {
            tokio::spawn(Self::run(
                self.workers,
                cmd_rx,
                self.work_tx.clone(),
                self.notifier.clone(),
                self.logger.clone(),
            ));
        }
    }

    /// Run the domain verifier with workers and retry scheduler.
    ///
    /// This spawns:
    /// - A retry scheduler that manages failed verifications with backoff timing.
    /// - Multiple worker tasks that process verifications without blocking.
    /// - A forwarder that sends new requests to workers.
    async fn run(
        workers: usize,
        work_rx: tokio_mpmc::Receiver<DomainVerification>,
        work_tx: tokio_mpmc::Sender<DomainVerification>,
        notifier: Arc<dyn CustomDomainVerifierNotifier>,
        logger: slog::Logger,
    ) {
        let mut tasks = JoinSet::new();
        let (retry_tx, retry_rx) = mpsc::unbounded_channel();

        // Spawn retry scheduler.
        tasks.spawn(Self::run_retry_scheduler(
            retry_rx,
            work_tx.clone(),
            logger.clone(),
        ));

        // Spawn domain verification workers.
        for worker_id in 0..workers {
            tasks.spawn(Self::run_worker(
                worker_id,
                work_rx.clone(),
                retry_tx.clone(),
                notifier.clone(),
                logger.clone(),
            ));
        }

        tasks.join_all().await;
    }

    /// Run the retry scheduler that manages failed verifications.
    ///
    /// The scheduler maintains a list of pending retries and sleeps until the next
    /// retry is due. When a verification is ready, it's sent back to the worker queue
    /// for immediate processing.
    async fn run_retry_scheduler(
        mut retry_rx: mpsc::UnboundedReceiver<DomainVerification>,
        work_tx: tokio_mpmc::Sender<DomainVerification>,
        logger: slog::Logger,
    ) -> Result<()> {
        let mut pending_retries = Vec::new();

        loop {
            // Calculate when the next retry is due.
            let next_retry_time = pending_retries
                .iter()
                .filter_map(|v: &DomainVerification| v.retry_at)
                .min();

            tokio::select! {
                // Handle new retry requests.
                verification = retry_rx.recv() => {
                    match verification {
                        Some(mut verification) => {
                            if !verification.schedule_retry() {
                                slog::warn!(logger, "giving up on domain verification";
                                    "domain" => &verification.domain,
                                    "instance_id" => ?verification.instance_id,
                                );
                            } else {
                                slog::debug!(logger, "scheduling domain verification retry";
                                    "domain" => &verification.domain,
                                    "instance_id" => ?verification.instance_id,
                                    "retry_at" => ?verification.retry_at,
                                );
                                pending_retries.push(verification);
                            }
                        }
                        None => break,
                    }
                }

                // Sleep until the next retry is due.
                _ = async {
                    match next_retry_time {
                        Some(retry_time) => {
                            tokio::time::sleep_until(retry_time).await;
                        }
                        None => {
                            // No pending retries, sleep indefinitely until new ones arrive.
                            std::future::pending::<()>().await;
                        }
                    }
                } => {
                    // Process all retries that are now ready.
                    let now = Instant::now();
                    let mut ready_retries = Vec::new();

                    pending_retries.retain(|v| {
                        if v.retry_at.is_none_or(|retry_time| now >= retry_time) {
                            ready_retries.push(v.clone());
                            false
                        } else {
                            true
                        }
                    });

                    for verification in ready_retries {
                        let _ = work_tx.send(verification).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Run a worker task that processes domain verifications.
    async fn run_worker(
        worker_id: usize,
        work_rx: tokio_mpmc::Receiver<DomainVerification>,
        retry_tx: mpsc::UnboundedSender<DomainVerification>,
        notifier: Arc<dyn CustomDomainVerifierNotifier>,
        logger: slog::Logger,
    ) -> Result<()> {
        let worker_logger = logger.new(slog::o!("worker_id" => worker_id));
        let resolver = hickory_resolver::Resolver::builder_tokio()?.build();

        while let Some(verification) = work_rx.recv().await? {
            // Skip cancelled verifications.
            if verification.is_cancelled() {
                continue;
            }

            slog::info!(worker_logger, "processing domain verification";
                "domain" => &verification.domain,
                "instance_id" => ?verification.instance_id,
            );

            // Perform single verification attempt (non-blocking)
            match Self::verify_domain(&resolver, &verification).await {
                Ok(_) => {
                    slog::info!(worker_logger, "domain verification successful";
                        "domain" => &verification.domain,
                        "instance_id" => ?verification.instance_id,
                    );

                    if !verification.is_cancelled() {
                        notifier
                            .verification_completed(verification.instance_id, &verification.domain)
                            .await;
                    }
                }
                Err(err) => {
                    slog::warn!(worker_logger, "domain verification error, scheduling retry";
                        "domain" => &verification.domain,
                        "instance_id" => ?verification.instance_id,
                        "err" => ?err,
                    );
                    let _ = retry_tx.send(verification);
                }
            }
        }

        Ok(())
    }

    /// Verify a single domain by checking its TXT records.
    ///
    /// This method performs a DNS TXT record lookup for the given domain and
    /// checks if it contains the expected verification token for the instance.
    async fn verify_domain(
        resolver: &hickory_resolver::TokioResolver,
        verification: &DomainVerification,
    ) -> Result<()> {
        let expected_token = format!("oasis-rofl-verification={}", verification.token);

        let txt_records = resolver.txt_lookup(&verification.domain).await?;
        for record in txt_records {
            for txt_data in record.txt_data() {
                let txt_string = String::from_utf8_lossy(txt_data);
                if txt_string.contains(&expected_token) {
                    return Ok(());
                }
            }
        }

        Err(anyhow::anyhow!("domain verification failed"))
    }
}
