use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use anyhow::{anyhow, Context as _};
use oasis_runtime_sdk::{
    core::{
        common::{crypto::hash::Hash, logger::get_logger},
        host::bundle_manager,
    },
    modules::rofl::app::prelude::*,
};
use oasis_runtime_sdk_rofl_market as market;
use rand::distributions::{Alphanumeric, DistString};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time,
};
use tokio_util::compat::FuturesAsyncWriteCompatExt;

use super::{
    config::{LocalConfig, Resources},
    types, SchedulerApp,
};

/// Metadata key used to configure the offer identifier.
const METADATA_KEY_OFFER: &str = "net.oasis.scheduler.offer";
/// Metadata key used to configure the deployment ORC bundle location.
const METADATA_KEY_DEPLOYMENT_ORC_REF: &str = "net.oasis.deployment.orc.ref";

/// Name of the label used to store the deployment hash.
const LABEL_DEPLOYMENT_HASH: &str = "net.oasis.scheduler.deployment_hash";

/// Instance manager.
pub struct Manager {
    env: Environment<SchedulerApp>,
    cfg: LocalConfig,

    logger: slog::Logger,
}

struct LocalState {
    /// A list of our bundles running locally.
    running: BTreeMap<market::types::InstanceId, bundle_manager::BundleInfo>,
    /// A list of deployments that should already be running but are not.
    pending_start: Vec<(market::types::InstanceId, market::types::Deployment)>,
    /// A list of instance identifiers that should have no running deployments.
    pending_stop: Vec<market::types::InstanceId>,
    /// A list of instance identifiers that should be accepted.
    accept: Vec<market::types::InstanceId>,
    /// A list of not-accepted instance identifiers and timestamps that should maybe be removed.
    maybe_remove: Vec<(market::types::InstanceId, u64)>,
    /// Amounts of resources used.
    resources_used: Resources,
}

impl Manager {
    /// Create a new manager instance.
    pub fn new(env: Environment<SchedulerApp>, cfg: LocalConfig) -> Self {
        Self {
            env,
            cfg,
            logger: get_logger("scheduler/manager"),
        }
    }

    /// Main loop of the ROFL scheduler.
    pub async fn run(self) {
        let mgr = Arc::new(self);

        loop {
            // Wait a bit before doing another pass.
            time::sleep(time::Duration::from_secs(1)).await;

            // Discover local state.
            let mut local_state = match mgr.discover().await {
                Ok(local_state) => local_state,
                Err(err) => {
                    slog::error!(mgr.logger, "failed to discover bundles"; "err" => ?err);
                    continue;
                }
            };

            // TODO: Process commands for existing instances.

            // Process any pending bundles.
            if let Err(err) = mgr.process_pending(&mut local_state).await {
                slog::error!(mgr.logger, "failed to process pending instances"; "err" => ?err);
                continue;
            }

            slog::info!(mgr.logger, "instance status";
                "running" => local_state.running.len(),
                "pending_start" => local_state.pending_start.len(),
                "pending_stop" => local_state.pending_stop.len(),
                "resources_used" => ?local_state.resources_used,
            );

            // TODO: Spawn tasks to complete all jobs: accept, remove, deploy, stop, wipe.
            if let Err(err) = mgr.process_jobs(&mut local_state).await {
                slog::error!(mgr.logger, "failed to process jobs"; "err" => ?err);
                continue;
            }
        }
    }

    /// Discover local state.
    async fn discover(&self) -> Result<LocalState> {
        let local_node_id = self.env.host().identity().await?;

        // Discover local bundles.
        let rsp = self
            .env
            .host()
            .bundle_manager()
            .bundle_list(bundle_manager::BundleListRequest {
                labels: BTreeMap::new(), // We want all our bundles.
            })
            .await?;

        let running: BTreeMap<market::types::InstanceId, bundle_manager::BundleInfo> = rsp
            .bundles
            .into_iter()
            .filter_map(|bi| {
                // We assume all bundles have been labeled by us and a third party should never
                // modify our bundles (as they are isolated by origin). Still, we skip bundles with
                // malformed labels just in case.
                let instance_id: market::types::InstanceId = bi
                    .labels
                    .get(bundle_manager::LABEL_INSTANCE_ID)?
                    .parse()
                    .ok()?;
                Some((instance_id, bi))
            })
            .collect();

        // Discover desired instance state.
        let client = self.env.client();
        let round = client.latest_round().await?;
        let instances: Vec<market::types::Instance> = client
            .query(
                round,
                "roflmarket.Instances",
                market::types::ProviderQuery {
                    provider: self.cfg.provider_address,
                },
            )
            .await?;

        let mut pending_start: Vec<(market::types::InstanceId, market::types::Deployment)> =
            Vec::new();
        let mut pending_stop: Vec<market::types::InstanceId> = Vec::new();
        let mut resources_used: Resources = Default::default();

        for instance in instances {
            match instance.status {
                market::types::InstanceStatus::Created => {
                    // Instance has not yet been accepted, nothing to do.
                    continue;
                }
                market::types::InstanceStatus::Cancelled => {
                    // Instance has been cancelled, make sure it is stopped if running.
                    if running.contains_key(&instance.id) {
                        pending_stop.push(instance.id);
                    }
                    continue;
                }
                market::types::InstanceStatus::Accepted => {
                    // Instance has been accepted, check if we should be hosting it.
                    if instance.node_id.unwrap() != local_node_id {
                        continue;
                    }
                }
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
            let desired = cmds.into_iter().fold(instance.deployment, |acc, qc| {
                let cmd = match cbor::from_slice::<types::Command>(&qc.cmd) {
                    Ok(cmd) => cmd,
                    Err(_) => return acc,
                };

                match cmd.method.as_str() {
                    types::METHOD_DEPLOY => {
                        match cbor::from_value::<types::DeployRequest>(cmd.args) {
                            Ok(deploy) => Some(deploy.deployment),
                            Err(_) => acc,
                        }
                    }
                    types::METHOD_TERMINATE => None,
                    _ => acc,
                }
            });

            let actual = running.get(&instance.id);
            match (desired, actual) {
                (Some(desired), Some(actual)) => {
                    // Instance is running and should be running. Determine whether it is running
                    // the correct deployment by comparing its hash.
                    let actual_hash = actual
                        .labels
                        .get(LABEL_DEPLOYMENT_HASH)
                        .cloned()
                        .unwrap_or_default();
                    let desired_hash = deployment_hash(&desired);

                    if actual_hash != desired_hash {
                        pending_start.push((instance.id, desired));
                    }
                }
                (Some(desired), None) => {
                    // Instance is not running and should be started.
                    pending_start.push((instance.id, desired));
                }
                (None, Some(actual)) => {
                    // Instance is running and should be stopped.
                    pending_stop.push(instance.id);
                }
                (None, None) => {
                    // Instance is not running and should be stopped. Nothing to do.
                }
            }
        }

        slog::info!(self.logger, "discovered instances";
            "running" => running.len(),
            "pending_start" => pending_start.len(),
            "pending_stop" => pending_stop.len(),
            "resources_used" => ?resources_used,
        );

        Ok(LocalState {
            running,
            pending_start,
            pending_stop,
            accept: Vec::new(),
            maybe_remove: Vec::new(),
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
        let instances: Vec<market::types::Instance> = client
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
            if instance.status != market::types::InstanceStatus::Created {
                local_state
                    .maybe_remove
                    .push((instance.id, instance.created_at));
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

            // Instance seems acceptable.
            local_state.accept.push(instance.id);
            local_state.resources_used.add(&instance.resources);
            if let Some(deployment) = instance.deployment {
                local_state.pending_start.push((instance.id, deployment));
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
        // TODO: Storage wipe.
        let start_jobs: Vec<_> = local_state
            .pending_start
            .iter()
            .map(|(id, deployment)| self.clone().start_instance(*id, deployment.clone(), false))
            .collect();

        // Prepare jobs to stop instances.
        let stop_jobs: Vec<_> = local_state
            .pending_stop
            .iter()
            .map(|id| self.clone().stop_instance(*id))
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

        slog::info!(self.logger, "running jobs"; "num_jobs" => jobs.len());

        for job in jobs {
            match job.await {
                Err(err) => {
                    slog::error!(self.logger, "job task panicked"; "err" => ?err);
                }
                Ok(Err(err)) => {
                    slog::error!(self.logger, "job task failed"; "err" => ?err);
                }
                Ok(Ok(_)) => {
                    // Ok.
                }
            }
        }

        slog::info!(self.logger, "jobs completed");

        Ok(())
    }

    async fn accept_instances(self: Arc<Self>, ids: Vec<market::types::InstanceId>) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceAccept",
            market::types::InstanceAccept {
                provider: self.cfg.provider_address,
                ids,
                metadata: BTreeMap::new(),
            },
        );
        let response = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    async fn remove_instance(
        self: Arc<Self>,
        instance: market::types::InstanceId,
        ts: u64,
    ) -> Result<()> {
        // TODO: Only remove instances if they are X old. Where X is fuzzy.
        Ok(())
    }

    /// Start the given instance with the provided deployment.
    async fn start_instance(
        self: Arc<Self>,
        instance_id: market::types::InstanceId,
        deployment: market::types::Deployment,
        wipe_storage: bool,
    ) -> Result<()> {
        // Wipe storage if needed.
        if wipe_storage {
            self.wipe_instance_storage(instance_id)
                .await
                .context("failed to wipe instance storage")?;
        }

        // Remove any existing bundles for this instance.
        self.clone()
            .stop_instance(instance_id)
            .await
            .context("failed to stop existing instance")?;

        // TODO: Add timeout for provisioning to avoid problems!

        slog::info!(self.logger, "resolving instance image";
            "instance_id" => ?instance_id,
        );

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
        let (manifest, digest, config) = client
            .pull_manifest_and_config(&bundle_ref, &auth)
            .await
            .context("failed to pull manifest and config")?;

        // TODO: Validate config.
        // TODO: Validate layers.

        // Generate a temporary bundle name. Use instance ID to ensure temporary bundles from
        // retries don't pile up.
        let temporary_name = format!("instance-{:x}", instance_id);

        slog::info!(self.logger, "pulling OCI image layers";
            "ref" => %bundle_ref,
            "instance_id" => ?instance_id,
            "temporary_name" => &temporary_name,
        );

        let (mut reader, writer) = tokio::io::simplex(128 * 1024);

        // Start task to pull images and generate an ORC archive.
        let mgr = self.clone();
        let layer_pull_task = tokio::spawn(async move {
            let mut zip_writer = async_zip::base::write::ZipFileWriter::with_tokio(writer);

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

                let opts =
                    async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Deflate);
                let mut entry_writer = zip_writer.write_entry_stream(opts).await?.compat_write();
                client
                    .pull_blob(&bundle_ref, &layer, &mut entry_writer)
                    .await?;
                entry_writer.into_inner().close().await?;
            }

            let mut writer = zip_writer.close().await?;
            writer.into_inner().shutdown().await?;

            slog::info!(mgr.logger, "all OCI layers pulled";
                "ref" => %bundle_ref,
                "instance_id" => ?instance_id,
            );

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

        tokio::try_join!(flatten(layer_pull_task), flatten(bundle_stream_task))?;

        // Deploy bundle.
        slog::info!(self.logger, "deploying bundle";
            "id" => ?instance_id,
            "temporary_name" => &temporary_name,
        );

        let mut labels = labels_for_instance(instance_id);
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
            "id" => ?instance_id,
        );

        Ok(())
    }

    /// Wipe storage of the given instance.
    async fn wipe_instance_storage(&self, instance_id: market::types::InstanceId) -> Result<()> {
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
    async fn stop_instance(self: Arc<Self>, instance_id: market::types::InstanceId) -> Result<()> {
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
}

fn labels_for_instance(id: market::types::InstanceId) -> BTreeMap<String, String> {
    BTreeMap::from([(
        bundle_manager::LABEL_INSTANCE_ID.to_string(),
        format!("{:x}", id),
    )])
}

fn deployment_hash(deployment: &market::types::Deployment) -> String {
    format!(
        "{:x}",
        Hash::digest_bytes(&cbor::to_vec(deployment.clone()))
    )
}
