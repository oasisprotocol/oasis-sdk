use std::collections::{BTreeMap, BTreeSet};

use anyhow::anyhow;
use oasis_runtime_sdk::{
    core::{
        common::{crypto::hash::Hash, logger::get_logger},
        host::bundle_manager,
    },
    modules::rofl::app::prelude::*,
};
use oasis_runtime_sdk_rofl_market as market;
use rand::distributions::{Alphanumeric, DistString};
use tokio::time;

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
    pub async fn run(&self) {
        loop {
            // Wait a bit before doing another pass.
            time::sleep(time::Duration::from_secs(1)).await;

            // Discover local state.
            let mut local_state = match self.discover().await {
                Ok(local_state) => local_state,
                Err(err) => {
                    slog::error!(self.logger, "failed to discover bundles"; "err" => ?err);
                    continue;
                }
            };

            // TODO: Process commands for existing instances.

            // Process any pending bundles.
            if let Err(err) = self.process_pending(&mut local_state).await {
                slog::error!(self.logger, "failed to process pending instances"; "err" => ?err);
                continue;
            }

            // TODO: Spawn tasks to complete all jobs: accept, remove, deploy, stop, wipe.
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
                    let desired_hash =
                        format!("{:x}", Hash::digest_bytes(&cbor::to_vec(desired.clone())));

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
    async fn process_pending(&self, local_state: &mut LocalState) -> Result<()> {
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

            if self.cfg.capacity.can_allocate(&new_resource_use) {
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

        // TODO: Consider removing old instances.
        // TODO: Randomize probability so multiple instances won't submit the exact same removal tx.

        Ok(())
    }

    /// Deploy the given deployment into an instance.
    async fn deploy_instance(
        &self,
        instance: market::types::Instance,
        deployment: market::types::Deployment,
        wipe_storage: bool,
    ) -> Result<()> {
        // Wipe storage if needed.
        if wipe_storage {
            self.wipe_instance_storage(instance.id).await?;
        }

        // Remove any existing bundles for this instance.
        self.stop_instance(instance.id).await?;

        slog::info!(self.logger, "resolving instance image";
            "id" => ?instance.id,
        );

        let mut client = oci_client::Client::new(oci_client::client::ClientConfig {
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

        // Pull manifest and config.
        let (manifest, digest, config) = client
            .pull_manifest_and_config(&bundle_ref, &auth)
            .await
            .map_err(|_| anyhow!("failed to pull manifest and config"))?;

        // TODO: Validate config.
        // TODO: Validate layers.

        // Generate a random temporary bundle name.
        let temporary_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);

        // TODO: Pull from registry and stream to host via bundle_write. Also need to ZIP!

        // Deploy bundle.
        let _ = self
            .env
            .host()
            .bundle_manager()
            .bundle_add(bundle_manager::BundleAddRequest {
                temporary_name,
                manifest_hash: deployment.manifest_hash,
                labels: labels_for_instance(instance.id),
            })
            .await?;

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
    async fn stop_instance(&self, instance_id: market::types::InstanceId) -> Result<()> {
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
