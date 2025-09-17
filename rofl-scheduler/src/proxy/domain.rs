use std::{collections::BinaryHeap, sync::Weak, time::Duration};

use anyhow::{anyhow, Result};
use backoff::backoff::Backoff;
use hickory_resolver::config::NameServerConfigGroup;
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

use oasis_runtime_sdk_rofl_market::types::InstanceId;
use rofl_app_core::prelude::*;

/// Trait for receiving domain verification notifications.
///
/// This trait should be implemented by components that need to be notified when
/// domain verification completes.
#[async_trait]
pub trait CustomDomainVerificationNotifier: Send + Sync {
    /// Called when a domain verification completes successfully.
    async fn verification_completed(&self, id: InstanceId, domain: &str);
}

/// Handle that can be used to cancel the verification.
pub struct CancelVerificationsOnDrop;

impl CancelVerificationsOnDrop {
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
    retry_at: Instant,
    /// Exponential backoff for retrying the verification.
    retry_backoff: Option<backoff::ExponentialBackoff>,
    /// Weak reference to the handle that can be used to cancel the verification.
    handle: Weak<CancelVerificationsOnDrop>,
}

impl DomainVerification {
    /// Create a new domain verification instance.
    ///
    /// When all strong instances of `handle` are dropped, the verification will be canceled.
    pub fn new(
        instance_id: InstanceId,
        domain: &str,
        token: &str,
        handle: &Arc<CancelVerificationsOnDrop>,
    ) -> Self {
        Self {
            instance_id,
            domain: domain.to_owned(),
            token: token.to_owned(),
            retry_at: Instant::now(),
            retry_backoff: None,
            handle: Arc::downgrade(handle),
        }
    }

    /// Calculate the next retry time using exponential backoff.
    ///
    /// Returns `true` if the retry was scheduled successfully, `false` if no further retries
    /// should be attempted.
    fn schedule_retry(&mut self) -> bool {
        let retry_backoff = self.retry_backoff.get_or_insert_with(|| {
            backoff::ExponentialBackoffBuilder::new()
                .with_max_elapsed_time(Some(Duration::from_secs(60)))
                .build()
        });

        match retry_backoff.next_backoff() {
            Some(next) => {
                self.retry_at = Instant::now() + next;
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

impl PartialEq for DomainVerification {
    fn eq(&self, other: &Self) -> bool {
        self.retry_at == other.retry_at
    }
}

impl Eq for DomainVerification {}

impl Ord for DomainVerification {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.retry_at.cmp(&self.retry_at)
    }
}

impl PartialOrd for DomainVerification {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
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
    notifier: Arc<dyn CustomDomainVerificationNotifier>,
    /// Logger for this verifier.
    logger: slog::Logger,
}

impl CustomDomainVerifier {
    /// Create a new custom domain verifier.
    pub fn new(
        workers: usize,
        notifier: Arc<dyn CustomDomainVerificationNotifier>,
        logger: slog::Logger,
    ) -> Self {
        let (work_tx, work_rx) = tokio_mpmc::channel(workers * 64);

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
        handle: &Arc<CancelVerificationsOnDrop>,
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
    async fn run(
        workers: usize,
        work_rx: tokio_mpmc::Receiver<DomainVerification>,
        work_tx: tokio_mpmc::Sender<DomainVerification>,
        notifier: Arc<dyn CustomDomainVerificationNotifier>,
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

        // Prepare DNS resolver.
        let mut name_servers = NameServerConfigGroup::google_https();
        name_servers.merge(NameServerConfigGroup::cloudflare_https());
        name_servers.merge(NameServerConfigGroup::quad9_https());

        let mut builder = hickory_resolver::Resolver::builder_with_config(
            hickory_resolver::config::ResolverConfig::from_parts(None, vec![], name_servers),
            hickory_resolver::name_server::TokioConnectionProvider::default(),
        );
        builder.options_mut().validate = true; // Enable DNSSEC validation.
        let resolver = builder.build();

        // Spawn domain verification workers.
        for _ in 0..workers {
            tasks.spawn(Self::run_worker(
                work_rx.clone(),
                retry_tx.clone(),
                notifier.clone(),
                resolver.clone(),
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
        let mut pending_retries = BinaryHeap::new();

        loop {
            tokio::select! {
                // Handle new retry requests.
                verification = retry_rx.recv() => {
                    match verification {
                        Some(mut verification) => {
                            if verification.is_cancelled() || !verification.schedule_retry() {
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
                    match pending_retries.peek().map(|v: &DomainVerification| v.retry_at) {
                        Some(retry_time) => {
                            tokio::time::sleep_until(retry_time).await;
                        }
                        None => {
                            // No pending retries, sleep indefinitely until new ones arrive.
                            std::future::pending::<()>().await;
                        }
                    }
                } => {
                    let _ = work_tx.send(pending_retries.pop().unwrap()).await;
                }
            }
        }

        Ok(())
    }

    /// Run a worker task that processes domain verifications.
    async fn run_worker(
        work_rx: tokio_mpmc::Receiver<DomainVerification>,
        retry_tx: mpsc::UnboundedSender<DomainVerification>,
        notifier: Arc<dyn CustomDomainVerificationNotifier>,
        resolver: hickory_resolver::TokioResolver,
        logger: slog::Logger,
    ) -> Result<()> {
        while let Some(verification) = work_rx.recv().await? {
            // Skip cancelled verifications.
            if verification.is_cancelled() {
                continue;
            }

            slog::info!(logger, "processing domain verification";
                "domain" => &verification.domain,
                "instance_id" => ?verification.instance_id,
            );

            // Perform a single verification attempt.
            match Self::verify_domain(&resolver, &verification).await {
                Ok(_) => {
                    slog::info!(logger, "domain verification successful";
                        "domain" => &verification.domain,
                        "instance_id" => ?verification.instance_id,
                    );

                    notifier
                        .verification_completed(verification.instance_id, &verification.domain)
                        .await;
                }
                Err(err) => {
                    slog::warn!(logger, "domain verification failed, scheduling retry";
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

        Err(anyhow::anyhow!("TXT record not found"))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_domain_verification_ordering() {
        let handle = Arc::new(CancelVerificationsOnDrop::new());
        let mut verification1 = DomainVerification::new(
            Default::default(),
            Default::default(),
            Default::default(),
            &handle,
        );
        verification1.retry_at = Instant::now() + Duration::from_secs(10);

        let mut verification2 = verification1.clone();
        verification2.retry_at = Instant::now() + Duration::from_secs(5);

        assert!(verification2 > verification1);

        let mut heap = BinaryHeap::new();
        heap.push(verification1.clone());
        heap.push(verification2.clone());

        assert!(heap.pop().unwrap() == verification2);
        assert!(heap.pop().unwrap() == verification1);
    }
}
