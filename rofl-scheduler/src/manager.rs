use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};

use anyhow::{anyhow, Context as _};
use backoff::backoff::Backoff;
use bytes::BufMut;
use oasis_runtime_sdk::{
    core::{
        common::{crypto::hash::Hash, logger::get_logger},
        host::bundle_manager,
    },
    modules::rofl::app::prelude::*,
};
use oasis_runtime_sdk_rofl_market::{
    self as market,
    types::{Deployment, Instance, InstanceId, InstanceStatus},
};
use rand::Rng;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time,
};
use tokio_util::compat::FuturesAsyncWriteCompatExt;

use super::{
    config::{LocalConfig, Resources},
    manifest::Manifest,
    types, SchedulerApp,
};

/// Interval on which the manager will do its processing.
const MANAGER_RUN_INTERVAL_SECS: u64 = 3;
/// Interval on which the manager will claim payment for an instance.
const CLAIM_PAYMENT_INTERVAL_SECS: u64 = 24 * 3600; // 24 hours

/// Metadata key used to configure the offer identifier.
const METADATA_KEY_OFFER: &str = "net.oasis.scheduler.offer";
/// Metadata key used to configure the deployment ORC bundle location.
const METADATA_KEY_DEPLOYMENT_ORC_REF: &str = "net.oasis.deployment.orc.ref";
/// Metadata key used to report errors.
const METADATA_KEY_ERROR: &str = "net.oasis.error";
/// Maximum length of the error message.
const METADATA_VALUE_ERROR_MAX_SIZE: usize = 1024;

/// Name of the label used to store the deployment hash.
const LABEL_DEPLOYMENT_HASH: &str = "net.oasis.scheduler.deployment_hash";

/// OCI media type for ORC config descriptors.
const OCI_TYPE_ORC_CONFIG: &str = "application/vnd.oasis.orc.config.v1+json";
/// OCI media type for ORC layer descriptors.
const OCI_TYPE_ORC_LAYER: &str = "application/vnd.oasis.orc.layer.v1";

/// Timeout for pulling images during deployment (in seconds).
const DEPLOY_PULL_TIMEOUT_SECS: u64 = 60;
/// Average number of seconds after which to remove instances that are not accepted. The scheduler
/// will randomize the value to minimize the chance of multiple schedulers removing at once.
const REMOVE_INSTANCE_AFTER_SECS: u64 = 1800;
/// Maximum size of the JSON-encoded ORC manifest.
const MAX_ORC_MANIFEST_SIZE: i64 = 16 * 1024; // 16 KiB
/// Maximum size of an ORC layer.
const MAX_ORC_LAYER_SIZE: i64 = 128 * 1024 * 1024; // 128 MiB
/// Maximum size of all ORC layers.
const MAX_ORC_TOTAL_SIZE: i64 = 128 * 1024 * 1024; // 128 MiB

/// Instance manager.
pub struct Manager {
    env: Environment<SchedulerApp>,
    cfg: LocalConfig,

    instance_state: Mutex<BTreeMap<InstanceId, InstanceState>>,

    logger: slog::Logger,
}

#[derive(Clone, Default)]
struct InstanceUpdates {
    complete_cmds: Option<market::types::CommandId>,
    deployment: Option<Option<market::types::Deployment>>,
    metadata: Option<BTreeMap<String, String>>,
}

struct LocalState {
    /// A map of all accepted instances.
    accepted: BTreeMap<InstanceId, Instance>,
    /// A list of our bundles running locally.
    running: BTreeMap<InstanceId, bundle_manager::BundleInfo>,
    /// A list of deployments that should already be running but are not.
    pending_start: Vec<(Instance, Deployment, bool)>,
    /// A list of instance identifiers that should have no running deployments.
    pending_stop: Vec<(InstanceId, bool)>,
    /// A map of instance updates.
    instance_updates: BTreeMap<InstanceId, InstanceUpdates>,
    /// A list of instance identifiers that should be accepted.
    accept: Vec<InstanceId>,
    /// A list of not-accepted instance identifiers and timestamps that should maybe be removed.
    maybe_remove: Vec<(InstanceId, u64)>,
    /// A list of instances to claim payment for.
    claim_payment: Vec<InstanceId>,
    /// Amounts of resources used.
    resources_used: Resources,
}

#[derive(Default)]
struct InstanceState {
    /// Last deployment.
    last_deployment: Option<Deployment>,
    /// Last error message corresponding to deploying `last_deployment`.
    last_error: Option<String>,
    // Whether to ignore instance start until the given time elapses.
    ignore_start_until: Option<Instant>,
    /// Backoff associated with ignoring instance start.
    ignore_start_backoff: Option<backoff::ExponentialBackoff>,
}

impl Manager {
    /// Create a new manager instance.
    pub fn new(env: Environment<SchedulerApp>, cfg: LocalConfig) -> Self {
        Self {
            env,
            cfg,
            instance_state: Mutex::new(BTreeMap::new()),
            logger: get_logger("scheduler/manager"),
        }
    }

    /// Main loop of the ROFL scheduler.
    pub async fn run(self) {
        let mgr = Arc::new(self);

        loop {
            // Wait a bit before doing another pass.
            time::sleep(time::Duration::from_secs(MANAGER_RUN_INTERVAL_SECS)).await;

            // Discover local state.
            let mut local_state = match mgr.discover().await {
                Ok(local_state) => local_state,
                Err(err) => {
                    slog::error!(mgr.logger, "failed to discover bundles"; "err" => ?err);
                    continue;
                }
            };

            // Process any pending instances.
            if let Err(err) = mgr.process_pending(&mut local_state).await {
                slog::error!(mgr.logger, "failed to process pending instances"; "err" => ?err);
                continue;
            }

            slog::info!(mgr.logger, "instance status";
                "accepted" => local_state.accepted.len(),
                "running" => local_state.running.len(),
                "pending_start" => local_state.pending_start.len(),
                "pending_stop" => local_state.pending_stop.len(),
                "instance_updates" => local_state.instance_updates.len(),
                "maybe_remove" => local_state.maybe_remove.len(),
                "claim_payment" => local_state.claim_payment.len(),
                "resources_used" => ?local_state.resources_used,
            );

            // Spawn tasks to process all jobs.
            if let Err(err) = mgr.process_jobs(&mut local_state).await {
                slog::error!(mgr.logger, "failed to process jobs"; "err" => ?err);
                continue;
            }
        }
    }

    /// Discover local state.
    async fn discover(&self) -> Result<LocalState> {
        let local_node_id = self.env.host().identity().await?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Discover local bundles.
        let rsp = self
            .env
            .host()
            .bundle_manager()
            .bundle_list(bundle_manager::BundleListRequest {
                labels: BTreeMap::new(), // We want all our bundles.
            })
            .await?;

        let running: BTreeMap<InstanceId, bundle_manager::BundleInfo> = rsp
            .bundles
            .into_iter()
            .filter_map(|bi| {
                // We assume all bundles have been labeled by us and a third party should never
                // modify our bundles (as they are isolated by origin). Still, we skip bundles with
                // malformed labels just in case.
                let instance_id: InstanceId = bi
                    .labels
                    .get(bundle_manager::LABEL_INSTANCE_ID)?
                    .parse()
                    .ok()?;
                Some((instance_id, bi))
            })
            .collect();
        let mut running_unknown = BTreeSet::from_iter(running.keys());

        // Discover desired instance state.
        let client = self.env.client();
        let round = client.latest_round().await?;
        let instances: Vec<Instance> = client
            .query(
                round,
                "roflmarket.Instances",
                market::types::ProviderQuery {
                    provider: self.cfg.provider_address,
                },
            )
            .await?;

        let mut accepted: BTreeMap<InstanceId, Instance> = BTreeMap::new();
        let mut pending_start: Vec<(Instance, Deployment, bool)> = Vec::new();
        let mut pending_stop: Vec<(InstanceId, bool)> = Vec::new();
        let mut instance_updates: BTreeMap<InstanceId, InstanceUpdates> = BTreeMap::new();
        let mut maybe_remove: Vec<(InstanceId, u64)> = Vec::new();
        let mut claim_payment: Vec<InstanceId> = Vec::new();
        let mut resources_used: Resources = Default::default();

        for instance in instances {
            // Remove known instances.
            running_unknown.remove(&instance.id);

            match instance.status {
                InstanceStatus::Created => {
                    // Instance has not yet been accepted, nothing to do.
                    continue;
                }
                InstanceStatus::Cancelled => {
                    // Instance has been cancelled, make sure it is stopped if running.
                    if running.contains_key(&instance.id) {
                        pending_stop.push((instance.id, true));
                    }
                    maybe_remove.push((instance.id, instance.updated_at));
                    continue;
                }
                InstanceStatus::Accepted => {
                    // Instance has been accepted, check if we should be hosting it.
                    if instance.node_id.unwrap() != local_node_id {
                        continue;
                    }
                }
            }

            accepted.insert(instance.id, instance.clone());

            // Check if the instance is still paid for. If not, we immediately stop it and schedule
            // its removal.
            if instance.paid_until < now {
                slog::info!(self.logger, "instance not paid for, stopping";
                    "id" => ?instance.id,
                );

                if running.contains_key(&instance.id) {
                    pending_stop.push((instance.id, true));
                }
                maybe_remove.push((instance.id, instance.paid_until));
                continue;
            }

            // Compute total provisioned resources.
            resources_used.add(&instance.resources);

            // Discover any pending commands to see if there is a "deploy" command somewhere in
            // there. This allows us to immediately deploy the right thing instead of first
            // deploying an old version and then immediately upgrading.
            let cmds: Vec<market::types::QueuedCommand> = client
                .query(
                    round,
                    "roflmarket.InstanceCommands",
                    market::types::InstanceQuery {
                        provider: instance.provider,
                        id: instance.id,
                    },
                )
                .await?;

            // Derive the desired instance state.
            let mut wipe_storage = false;
            let mut force_restart = false;
            let mut last_processed_cmd = Default::default();
            let mut desired = instance.deployment.clone();

            for qc in &cmds {
                last_processed_cmd = qc.id;

                let cmd = match cbor::from_slice::<types::Command>(&qc.cmd) {
                    Ok(cmd) => cmd,
                    Err(_) => continue,
                };

                match cmd.method.as_str() {
                    types::METHOD_DEPLOY => {
                        match cbor::from_value::<types::DeployRequest>(cmd.args) {
                            Ok(deploy) => {
                                desired = Some(deploy.deployment);
                                wipe_storage = wipe_storage || deploy.wipe_storage;
                            }
                            Err(_) => continue,
                        }
                    }
                    types::METHOD_TERMINATE => {
                        match cbor::from_value::<types::TerminateRequest>(cmd.args) {
                            Ok(terminate) => {
                                desired = None;
                                wipe_storage = wipe_storage || terminate.wipe_storage;
                            }
                            Err(_) => continue,
                        }
                    }
                    types::METHOD_RESTART => {
                        match cbor::from_value::<types::RestartRequest>(cmd.args) {
                            Ok(restart) => {
                                wipe_storage = wipe_storage || restart.wipe_storage;
                                force_restart = true;
                            }
                            Err(_) => continue,
                        }
                    }
                    _ => continue,
                }
            }

            if cmds.len() > 0 {
                instance_updates
                    .entry(instance.id)
                    .or_default()
                    .complete_cmds = Some(last_processed_cmd);
            }

            // Make sure that metadata is updated after processing all the commands.
            if instance.deployment != desired {
                instance_updates.entry(instance.id).or_default().deployment = Some(desired.clone());
            }

            // If the instance has been running for a while, make sure to claim payment. Use a fuzzy
            // interval to distribute claims a bit.
            let timeout = rand::distributions::Uniform::new(75, 125);
            let max_delta =
                (CLAIM_PAYMENT_INTERVAL_SECS * rand::thread_rng().sample(timeout)) / 100;
            if now.saturating_sub(instance.paid_from) > max_delta {
                claim_payment.push(instance.id);
            }

            let actual = running.get(&instance.id);
            match (actual, desired) {
                (Some(actual), Some(desired)) => {
                    // Instance is running and should be running. Determine whether it is running
                    // the correct deployment by comparing its hash.
                    let actual_hash = actual
                        .labels
                        .get(LABEL_DEPLOYMENT_HASH)
                        .cloned()
                        .unwrap_or_default();
                    let desired_hash = deployment_hash(&desired);

                    if actual_hash != desired_hash || force_restart {
                        pending_start.push((instance, desired, wipe_storage));
                    }
                }
                (None, Some(desired)) => {
                    // Instance is not running and should be started.
                    pending_start.push((instance, desired, wipe_storage));
                }
                (Some(_), None) => {
                    // Instance is running and should be stopped.
                    pending_stop.push((instance.id, wipe_storage));
                }
                (None, None) => {
                    // Instance is not running and should be stopped. Nothing to do.
                }
            }
        }

        // Stop any unknown instances.
        for instance_id in running_unknown {
            slog::info!(self.logger, "stopping unknown instance";
                "id" => ?instance_id,
            );

            pending_stop.push((*instance_id, true));
        }

        slog::info!(self.logger, "discovered instances";
            "accepted" => accepted.len(),
            "running" => running.len(),
            "pending_start" => pending_start.len(),
            "pending_stop" => pending_stop.len(),
            "instance_updates" => instance_updates.len(),
            "maybe_remove" => maybe_remove.len(),
            "claim_payment" => claim_payment.len(),
            "resources_used" => ?resources_used,
        );

        Ok(LocalState {
            accepted,
            running,
            pending_start,
            pending_stop,
            instance_updates,
            accept: Vec::new(),
            maybe_remove,
            claim_payment,
            resources_used,
        })
    }

    /// Process pending instances.
    async fn process_pending(self: &Arc<Self>, local_state: &mut LocalState) -> Result<()> {
        let client = self.env.client();

        let round = client.latest_round().await?;
        let provider_query = market::types::ProviderQuery {
            provider: self.cfg.provider_address,
        };
        let offers: Vec<market::types::Offer> = client
            .query(round, "roflmarket.Offers", provider_query.clone())
            .await?;
        let instances: Vec<Instance> = client
            .query(round, "roflmarket.Instances", provider_query)
            .await?;

        let acceptable_offers: BTreeSet<market::types::OfferId> = offers
            .into_iter()
            .filter_map(|offer| {
                let offer_key = offer.metadata.get(METADATA_KEY_OFFER)?;

                if self.cfg.offers.contains(offer_key) {
                    Some(offer.id)
                } else {
                    None
                }
            })
            .collect();

        for instance in instances {
            if instance.status != InstanceStatus::Created {
                continue;
            }

            slog::info!(self.logger, "evaluating instance";
                "id" => ?instance.id,
                "status" => ?instance.status,
            );

            // Check if creator is among the allowed creators.
            if !self.cfg.allowed_creators.is_empty()
                && !self.cfg.allowed_creators.contains(&instance.creator)
            {
                slog::info!(self.logger, "creator not allowed";
                    "id" => ?instance.id,
                    "creator" => instance.creator,
                );
                local_state
                    .maybe_remove
                    .push((instance.id, instance.created_at));
                continue;
            }

            // Check if offer is among the configured offers.
            if !acceptable_offers.contains(&instance.offer) {
                slog::info!(self.logger, "offer not acceptable for this instance";
                    "id" => ?instance.id,
                    "offer" => ?instance.offer,
                );
                local_state
                    .maybe_remove
                    .push((instance.id, instance.created_at));
                continue;
            }

            // Check if we have enough local capacity.
            let mut new_resource_use = local_state.resources_used.clone();
            new_resource_use.add(&instance.resources);

            if !self.cfg.capacity.can_allocate(&new_resource_use) {
                slog::info!(self.logger, "no more capacity for offer";
                    "id" => ?instance.id,
                    "offer" => ?instance.offer,
                );
                local_state
                    .maybe_remove
                    .push((instance.id, instance.created_at));
                continue;
            }

            slog::info!(self.logger, "instance seems acceptable";
                "id" => ?instance.id,
                "offer" => ?instance.offer,
            );

            // Instance seems acceptable.
            local_state.accept.push(instance.id);
            local_state.resources_used.add(&instance.resources);
            if let Some(deployment) = instance.deployment.clone() {
                local_state
                    .pending_start
                    .push((instance, deployment.clone(), true));
            }
        }

        Ok(())
    }

    /// Process queued jobs.
    async fn process_jobs(self: &Arc<Self>, local_state: &mut LocalState) -> Result<()> {
        // Prepare job to accept instances.
        let accept_jobs: Vec<_> = local_state
            .accept
            .chunks(16)
            .map(|ids| self.clone().accept_instances(ids.to_vec()))
            .collect();

        // Prepare jobs to remove instances.
        let remove_jobs: Vec<_> = local_state
            .maybe_remove
            .iter()
            .map(|(id, ts)| self.clone().remove_instance(*id, *ts))
            .collect();

        // Prepare jobs to start instances.
        let start_jobs: Vec<_> = local_state
            .pending_start
            .iter()
            .map(|(instance, deployment, wipe_storage)| {
                self.clone()
                    .start_instance(instance.clone(), deployment.clone(), *wipe_storage)
            })
            .collect();

        // Prepare jobs to stop instances.
        let stop_jobs: Vec<_> = local_state
            .pending_stop
            .iter()
            .map(|(id, wipe_storage)| self.clone().stop_instance(*id, *wipe_storage))
            .collect();

        // Prepare jobs to claim payments.
        let claim_payment_jobs: Vec<_> = local_state
            .claim_payment
            .chunks(16)
            .map(|chunk| self.clone().claim_payment(chunk.to_vec()))
            .collect();

        // Execute all jobs in parallel.
        let mut jobs = Vec::new();
        for job in accept_jobs {
            jobs.push(tokio::spawn(job));
        }
        for job in remove_jobs {
            jobs.push(tokio::spawn(job));
        }
        for job in start_jobs {
            jobs.push(tokio::spawn(job));
        }
        for job in stop_jobs {
            jobs.push(tokio::spawn(job));
        }
        for job in claim_payment_jobs {
            jobs.push(tokio::spawn(job));
        }

        slog::info!(self.logger, "running jobs"; "num_jobs" => jobs.len());

        for job in jobs {
            match job.await {
                Err(err) => {
                    slog::error!(self.logger, "task panicked"; "err" => ?err);
                }
                Ok(Err(err)) => {
                    slog::error!(self.logger, "task failed"; "err" => ?err);
                }
                Ok(Ok(_)) => {
                    // Ok.
                }
            }
        }

        slog::info!(self.logger, "running instance update jobs");

        // After all instance jobs have completed, collect additional instance update jobs as those
        // depend on last instance status.
        for job in self.collect_instance_update_jobs(local_state) {
            match job.await {
                Err(err) => {
                    slog::error!(self.logger, "instance update task panicked"; "err" => ?err);
                }
                Ok(Err(err)) => {
                    slog::error!(self.logger, "instance update task failed"; "err" => ?err);
                }
                Ok(Ok(_)) => {
                    // Ok.
                }
            }
        }

        slog::info!(self.logger, "all jobs completed");

        Ok(())
    }

    /// Inspect all instances and generate jobs to update their metadata.
    fn collect_instance_update_jobs(
        self: &Arc<Self>,
        local_state: &mut LocalState,
    ) -> Vec<tokio::task::JoinHandle<Result<()>>> {
        let instance_state = self.instance_state.lock().unwrap();

        // Determine the set of instances that need to be updated. These are either ones that have
        // been explicitly requested by earlier phases or any that have errors set due to job
        // processing.
        let relevant_instances: BTreeSet<_> = local_state
            .instance_updates
            .keys()
            .copied()
            .chain(instance_state.keys().copied())
            .collect();

        // Iterate through all updates and fill in any unchanged fields from instances.
        const CHUNK_SIZE: usize = 16;
        let mut tasks = Vec::with_capacity(relevant_instances.len());
        let mut chunk = Vec::with_capacity(CHUNK_SIZE);
        let mut spawn_task_chunk = |chunk: &mut Vec<_>| {
            let chunk = std::mem::replace(chunk, Vec::with_capacity(CHUNK_SIZE));
            let task = tokio::spawn(self.clone().update_instance_metadata(chunk));
            tasks.push(task);
        };

        for instance_id in relevant_instances {
            let instance = local_state.accepted.get(&instance_id).unwrap(); // Must exist.
            let state = instance_state.get(&instance_id);
            let mut updates = local_state
                .instance_updates
                .remove(&instance_id)
                .unwrap_or_default();

            if updates.deployment.is_none() {
                updates.deployment = Some(instance.deployment.clone());
            }
            if updates.metadata.is_none() {
                updates.metadata = Some(instance.metadata.clone());
            }

            // Set last error metadata entry.
            if let Some(mut error) = state.and_then(|s| s.last_error.clone()) {
                error.truncate(METADATA_VALUE_ERROR_MAX_SIZE);

                updates
                    .metadata
                    .as_mut()
                    .unwrap()
                    .insert(METADATA_KEY_ERROR.to_string(), error);
            } else {
                updates
                    .metadata
                    .as_mut()
                    .unwrap()
                    .remove(METADATA_KEY_ERROR);
            }

            // Skip updates that don't change anything.
            let mut changed = false;
            if updates.complete_cmds.is_some() {
                changed = true;
            }
            if updates.deployment.as_ref().unwrap() != &instance.deployment {
                changed = true;
            }
            if updates.metadata.as_ref().unwrap() != &instance.metadata {
                changed = true;
            }

            if changed {
                chunk.push((instance.id, updates));

                if chunk.len() >= chunk.capacity() {
                    spawn_task_chunk(&mut chunk);
                }
            }
        }
        if !chunk.is_empty() {
            spawn_task_chunk(&mut chunk);
        }

        tasks
    }

    /// Accept the given instances.
    async fn accept_instances(self: Arc<Self>, ids: Vec<InstanceId>) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceAccept",
            market::types::InstanceAccept {
                provider: self.cfg.provider_address,
                ids,
                metadata: BTreeMap::new(),
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    /// Remove the given instances.
    async fn remove_instance(self: Arc<Self>, instance: InstanceId, ts: u64) -> Result<()> {
        // Determine whether the instance should be removed. We use a randomized interval to
        // minimize the chance of multiple schedulers removing the same instances.
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let timeout = rand::distributions::Uniform::new(75, 125);
        let max_delta = (REMOVE_INSTANCE_AFTER_SECS * rand::thread_rng().sample(timeout)) / 100;
        if now.saturating_sub(ts) < max_delta {
            return Ok(());
        }

        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceRemove",
            market::types::InstanceRemove {
                provider: self.cfg.provider_address,
                id: instance,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        let mut instance_state = self.instance_state.lock().unwrap();
        instance_state.remove(&instance);

        Ok(())
    }

    /// Start the given instance with the provided deployment.
    async fn start_instance(
        self: Arc<Self>,
        instance: Instance,
        deployment: Deployment,
        wipe_storage: bool,
    ) -> Result<()> {
        if !self.should_start_instance(instance.id, &deployment) {
            return Ok(());
        }

        // Remove any existing bundles for this instance.
        self.clone()
            .stop_instance(instance.id, wipe_storage)
            .await
            .context("failed to stop existing instance")?;

        self.set_next_instance_deployment(instance.id, &deployment);

        match self.pull_and_deploy_instance(&instance, &deployment).await {
            Ok(_) => {
                self.allow_instance_start(instance.id);
                Ok(())
            }
            Err(err) => {
                slog::error!(self.logger, "failed to deploy instance";
                    "id" => ?instance.id,
                    "err" => ?err,
                );
                self.ignore_instance_start(instance.id, err.to_string());
                Err(err)
            }
        }
    }

    fn set_next_instance_deployment(&self, instance_id: InstanceId, deployment: &Deployment) {
        let mut instance_state = self.instance_state.lock().unwrap();
        let state = instance_state.entry(instance_id).or_default();
        state.last_deployment = Some(deployment.clone());
        state.last_error = None;
    }

    fn ignore_instance_start(&self, instance_id: InstanceId, reason: String) {
        let mut instance_state = self.instance_state.lock().unwrap();
        let state = instance_state.entry(instance_id).or_default();
        if state.ignore_start_backoff.is_none() {
            state.ignore_start_backoff = Some(backoff::ExponentialBackoff {
                max_elapsed_time: None,
                ..Default::default()
            });
        }

        state.ignore_start_until = state
            .ignore_start_backoff
            .as_mut()
            .unwrap()
            .next_backoff()
            .and_then(|d| Instant::now().checked_add(d));

        state.last_error = Some(reason);
    }

    fn should_start_instance(&self, instance_id: InstanceId, deployment: &Deployment) -> bool {
        let mut instance_state = self.instance_state.lock().unwrap();
        let state = instance_state.entry(instance_id).or_default();

        if let Some(last_deployment) = &state.last_deployment {
            // In case the deployment has changed, allow immediate start as the new deployment could
            // fix startup and we should make sure to process it immediately.
            if deployment != last_deployment {
                return true;
            }
        }

        if let Some(ignore_start_until) = state.ignore_start_until {
            if Instant::now() < ignore_start_until {
                return false;
            }
        }
        true
    }

    fn allow_instance_start(&self, instance_id: InstanceId) {
        let mut instance_state = self.instance_state.lock().unwrap();
        if let Some(state) = instance_state.get_mut(&instance_id) {
            state.ignore_start_backoff = None;
            state.ignore_start_until = None;
            state.last_error = None;
        }
    }

    /// Pull the given deployment and deploy it into the given instance.
    async fn pull_and_deploy_instance(
        self: &Arc<Self>,
        instance: &Instance,
        deployment: &Deployment,
    ) -> Result<()> {
        let temporary_name = self
            .pull_and_validate_deployment(&instance, &deployment)
            .await?;
        self.deploy_instance(instance, deployment, temporary_name)
            .await
    }

    /// Deploy the given deployment on the given instance. Requires that the deployment has already
    /// been pulled and is available on the host under the given temporary name.
    async fn deploy_instance(
        &self,
        instance: &Instance,
        deployment: &Deployment,
        temporary_name: String,
    ) -> Result<()> {
        slog::info!(self.logger, "deploying bundle";
            "id" => ?instance.id,
            "temporary_name" => &temporary_name,
        );

        let mut labels = labels_for_instance(instance.id);
        labels.insert(
            LABEL_DEPLOYMENT_HASH.to_string(),
            deployment_hash(&deployment),
        );

        let _ = self
            .env
            .host()
            .bundle_manager()
            .bundle_add(bundle_manager::BundleAddRequest {
                temporary_name,
                manifest_hash: deployment.manifest_hash,
                labels,
            })
            .await?;

        slog::info!(self.logger, "bundle deployed";
            "id" => ?instance.id,
        );

        Ok(())
    }

    /// Pull bundle for given deployment and validate if the bundle is suitable for the instance.
    async fn pull_and_validate_deployment(
        self: &Arc<Self>,
        instance: &Instance,
        deployment: &Deployment,
    ) -> Result<String> {
        slog::info!(self.logger, "pulling deployment bundle";
            "instance_id" => ?instance.id,
        );

        // TODO: Perform local caching.
        let client = oci_client::Client::new(oci_client::client::ClientConfig {
            protocol: oci_client::client::ClientProtocol::Https,
            ..Default::default()
        });
        let bundle_ref: oci_client::Reference = deployment
            .metadata
            .get(METADATA_KEY_DEPLOYMENT_ORC_REF)
            .ok_or(anyhow!("bundle location not set"))?
            .parse()
            .map_err(|_| anyhow!("bad bundle location"))?;
        // TODO: Support other authentication methods (e.g. credentials in encrypted metadata).
        let auth = oci_client::secrets::RegistryAuth::Anonymous;

        slog::info!(self.logger, "pulling OCI image manifest";
            "ref" => %bundle_ref,
        );

        // Pull manifest and config.
        let (manifest, _digest, config) = client
            .pull_manifest_and_config(&bundle_ref, &auth)
            .await
            .context("failed to pull manifest and config")?;

        // Validate config and layers.
        let mut total_size: i64 = 0;
        if manifest.config.media_type != OCI_TYPE_ORC_CONFIG {
            return Err(anyhow!("invalid ORC config media type"));
        }
        if manifest.config.size > MAX_ORC_MANIFEST_SIZE {
            return Err(anyhow!("ORC manifest too big"));
        }
        total_size = total_size.saturating_add(manifest.config.size);
        for layer in &manifest.layers {
            if layer.media_type != OCI_TYPE_ORC_LAYER {
                return Err(anyhow!("invalid ORC layer media type"));
            }
            if layer.size > MAX_ORC_LAYER_SIZE {
                return Err(anyhow!("ORC layer too big"));
            }
            total_size = total_size.saturating_add(layer.size);
        }
        if total_size > MAX_ORC_TOTAL_SIZE {
            return Err(anyhow!("ORC bundle too big"));
        }

        // Parse the ORC manifest.
        slog::info!(self.logger, "got ORC manifest"; "manifest" => &config);
        let orc_manifest: Manifest = serde_json::from_str(&config).context("bad ORC manifest")?;
        orc_manifest.validate().context("invalid ORC manifest")?;

        // Validate resources in the manifest against resources of the instance.
        let mut qcow2_names = BTreeSet::new();
        let mut total_memory = 0u64;
        let mut total_cpus = 0u16;
        for component in orc_manifest.components {
            if let Some(_sgx) = component.sgx {
                if instance.resources.tee != market::types::TeeType::SGX {
                    return Err(anyhow!("ORC has incompatible TEE type"));
                }

                // TODO: Account for SGX resources, probably just threads, heap and stack size.
            }
            if let Some(tdx) = component.tdx {
                if instance.resources.tee != market::types::TeeType::TDX {
                    return Err(anyhow!("ORC has incompatible TEE type"));
                }

                total_memory = total_memory.saturating_add(tdx.resources.memory);
                total_cpus = total_cpus.saturating_add(tdx.resources.cpus);
                if tdx.stage2_image.is_empty() {
                    continue;
                }
                qcow2_names.insert(tdx.stage2_image.clone());
            }
        }
        if total_memory > instance.resources.memory {
            return Err(anyhow!("ORC exceeds instance memory resources"));
        }
        if total_cpus > instance.resources.cpus {
            return Err(anyhow!("ORC exceeds instance vCPU resources"));
        }
        let available_storage = instance.resources.storage * 1024 * 1024;

        // Generate a temporary bundle name. Use instance ID to ensure temporary bundles from
        // retries don't pile up.
        let temporary_name = format!("instance-{:x}", instance.id);

        slog::info!(self.logger, "pulling OCI image layers";
            "ref" => %bundle_ref,
            "instance_id" => ?instance.id,
            "temporary_name" => &temporary_name,
        );

        let (mut reader, writer) = tokio::io::simplex(128 * 1024);

        // Start task to pull images and generate an ORC archive.
        let mgr = self.clone();
        let instance_id = instance.id;
        let layer_pull_task = tokio::spawn(async move {
            let mut zip_writer = async_zip::base::write::ZipFileWriter::with_tokio(writer);
            let mut total_storage_size = 0u64;

            for layer in [&[manifest.config], manifest.layers.as_slice()].concat() {
                let name = layer
                    .annotations
                    .as_ref()
                    .ok_or(anyhow!("missing layer name"))?
                    .get(oci_client::annotations::ORG_OPENCONTAINERS_IMAGE_TITLE)
                    .ok_or(anyhow!("missing layer name"))?
                    .clone();

                slog::info!(mgr.logger, "pulling OCI layer";
                    "ref" => %bundle_ref,
                    "instance_id" => ?instance_id,
                    "layer_name" => &name,
                );

                let is_qcow2 = qcow2_names.contains(&name);
                let mut qcow2_hdr_buf = bytes::BytesMut::with_capacity(64 * 1024).writer();

                let opts =
                    async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Deflate);
                let mut entry_writer = zip_writer.write_entry_stream(opts).await?.compat_write();
                let mut inspect_writer =
                    tokio_util::io::InspectWriter::new(&mut entry_writer, |data| {
                        if !is_qcow2 || !qcow2_hdr_buf.get_ref().has_remaining_mut() {
                            return;
                        }
                        qcow2_hdr_buf.write(data).unwrap();
                    });
                client
                    .pull_blob(&bundle_ref, &layer, &mut inspect_writer)
                    .await?;
                entry_writer.into_inner().close().await?;

                // Validate qcow2 header.
                if is_qcow2 {
                    let qcow2_hdr_buf = qcow2_hdr_buf.into_inner();
                    let qcow2_hdr = qcow2_rs::meta::Qcow2Header::from_buf(&qcow2_hdr_buf[..])
                        .map_err(|_| anyhow!("malformed QCOW2 header"))?;
                    total_storage_size = total_storage_size.saturating_add(qcow2_hdr.size());
                }
            }

            let writer = zip_writer.close().await?;
            writer.into_inner().shutdown().await?;

            slog::info!(mgr.logger, "all OCI layers pulled";
                "ref" => %bundle_ref,
                "instance_id" => ?instance_id,
            );

            // Validate storage size.
            if total_storage_size > available_storage {
                return Err(anyhow!("ORC exceeds instance storage resources"));
            }

            Ok::<(), anyhow::Error>(())
        });

        // Start task to stream bundle to host.
        const CHUNK_SIZE: usize = 128 * 1024;
        let tmp_name = temporary_name.clone();
        let mgr = self.clone();
        let bundle_stream_task = tokio::spawn(async move {
            let mut create = true;
            let mut buffer = bytes::BytesMut::with_capacity(CHUNK_SIZE);

            loop {
                // Read from layer pull task.
                buffer.clear();
                while buffer.len() < CHUNK_SIZE {
                    let n = reader
                        .read_buf(&mut buffer)
                        .await
                        .context("failed to read bundle chunk")?;

                    if n == 0 {
                        break;
                    }
                }
                if buffer.is_empty() {
                    break;
                }

                // Write to host.
                let _ = mgr
                    .env
                    .host()
                    .bundle_manager()
                    .bundle_write(bundle_manager::BundleWriteRequest {
                        temporary_name: tmp_name.clone(), // This is suboptimal.
                        create,
                        data: buffer.to_vec(), // This is suboptimal.
                    })
                    .await
                    .context("failed to write bundle chunk to host")?;

                create = false;
            }

            Ok::<(), anyhow::Error>(())
        });

        // Wait for completion.
        async fn flatten(handle: tokio::task::JoinHandle<Result<()>>) -> Result<()> {
            handle.await?
        }

        time::timeout(
            time::Duration::from_secs(DEPLOY_PULL_TIMEOUT_SECS),
            async move { tokio::try_join!(flatten(layer_pull_task), flatten(bundle_stream_task)) },
        )
        .await
        .map_err(|_| anyhow!("timed out while pulling bundle"))??;

        Ok(temporary_name)
    }

    /// Wipe storage of the given instance.
    async fn wipe_instance_storage(&self, instance_id: InstanceId) -> Result<()> {
        let _ = self
            .env
            .host()
            .bundle_manager()
            .bundle_wipe_storage(bundle_manager::BundleWipeStorageRequest {
                labels: labels_for_instance(instance_id),
            })
            .await?;

        Ok(())
    }

    /// Stop an instance, removing all bundles associated with it.
    async fn stop_instance(
        self: Arc<Self>,
        instance_id: InstanceId,
        wipe_storage: bool,
    ) -> Result<()> {
        // Wipe storage if needed.
        if wipe_storage {
            self.wipe_instance_storage(instance_id)
                .await
                .context("failed to wipe instance storage")?;
        }

        let _ = self
            .env
            .host()
            .bundle_manager()
            .bundle_remove(bundle_manager::BundleRemoveRequest {
                labels: labels_for_instance(instance_id),
            })
            .await?;

        Ok(())
    }

    /// Update instance metadata.
    async fn update_instance_metadata(
        self: Arc<Self>,
        updates: Vec<(InstanceId, InstanceUpdates)>,
    ) -> Result<()> {
        let updates = updates
            .into_iter()
            .map(|(id, update)| market::types::Update {
                id,
                deployment: update.deployment,
                metadata: update.metadata,
                last_completed_cmd: update.complete_cmds,
                ..Default::default()
            })
            .collect();

        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceUpdate",
            market::types::InstanceUpdate {
                provider: self.cfg.provider_address,
                updates,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    /// Claim payment for a set of instances.
    async fn claim_payment(self: Arc<Self>, instances: Vec<InstanceId>) -> Result<()> {
        slog::info!(self.logger, "claiming payment for instances";
            "instances" => ?instances,
        );

        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceClaimPayment",
            market::types::InstanceClaimPayment {
                provider: self.cfg.provider_address,
                instances,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }
}

/// Generate labels for a given instance.
fn labels_for_instance(id: InstanceId) -> BTreeMap<String, String> {
    BTreeMap::from([(
        bundle_manager::LABEL_INSTANCE_ID.to_string(),
        format!("{:x}", id),
    )])
}

/// Generate deployment hash for a given deployment.
fn deployment_hash(deployment: &Deployment) -> String {
    format!(
        "{:x}",
        Hash::digest_bytes(&cbor::to_vec(deployment.clone()))
    )
}
