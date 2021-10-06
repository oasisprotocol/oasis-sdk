//! Runtime.
use std::sync::Arc;

use oasis_core_runtime::{
    common::version,
    config::Config,
    consensus::verifier::TrustRoot,
    rak::RAK,
    start_runtime,
    types::{FeatureScheduleControl, Features},
    Protocol, RpcDemux, RpcDispatcher, TxnDispatcher,
};

use crate::{
    config,
    context::Context,
    crypto, dispatcher,
    keymanager::{KeyManagerClient, TrustedPolicySigners},
    module::{AuthHandler, BlockHandler, InvariantHandler, MethodHandler, MigrationHandler},
    modules, storage,
};

/// A runtime.
pub trait Runtime {
    /// Runtime version.
    const VERSION: version::Version;
    /// State version.
    const STATE_VERSION: u32 = 0;

    /// Prefetch limit. To enable prefetch set it to a non-zero value.
    const PREFETCH_LIMIT: u16 = 0;

    /// Whether the runtime should take control of transaction scheduling.
    const SCHEDULE_CONTROL: Option<config::ScheduleControl> = None;

    /// Module that provides the core API.
    type Core: modules::core::API;

    /// Supported modules.
    type Modules: AuthHandler + MigrationHandler + MethodHandler + BlockHandler + InvariantHandler;

    /// Return the trusted policy signers for this runtime; if `None`, a key manager connection will
    /// not be established on startup.
    fn trusted_policy_signers() -> Option<TrustedPolicySigners> {
        None
    }

    /// Return the consensus layer trust root for this runtime; if `None`, consensus layer integrity
    /// verification will not be performed.
    fn consensus_trust_root() -> Option<TrustRoot> {
        None
    }

    /// Genesis state for the runtime.
    fn genesis_state() -> <Self::Modules as MigrationHandler>::Genesis;

    /// Perform runtime-specific state migration. This method is only called when the recorded
    /// state version does not match `STATE_VERSION`.
    fn migrate_state<C: Context>(_ctx: &mut C) {
        // Default implementation doesn't perform any migration.
    }

    /// Perform state migrations if required.
    fn migrate<C: Context>(ctx: &mut C) {
        let store = storage::TypedStore::new(storage::PrefixStore::new(
            ctx.runtime_state(),
            &modules::core::MODULE_NAME,
        ));
        let mut metadata: modules::core::types::Metadata = store
            .get(modules::core::state::METADATA)
            .unwrap_or_default();

        // Perform state migrations/initialization on all modules.
        let mut has_changes =
            Self::Modules::init_or_migrate(ctx, &mut metadata, Self::genesis_state());

        // Check if we need to also apply any global state updates.
        let global_version = metadata
            .versions
            .get(modules::core::types::VERSION_GLOBAL_KEY)
            .copied()
            .unwrap_or_default();
        if global_version != Self::STATE_VERSION {
            assert!(
                global_version == Self::STATE_VERSION - 1,
                "inconsistent existing state version (expected: {} got: {})",
                Self::STATE_VERSION - 1,
                global_version
            );

            Self::migrate_state(ctx);

            // Update metadata.
            metadata.versions.insert(
                modules::core::types::VERSION_GLOBAL_KEY.to_string(),
                Self::STATE_VERSION,
            );
            has_changes = true;
        }

        // If there are any changes, update metadata.
        if has_changes {
            let mut store = storage::TypedStore::new(storage::PrefixStore::new(
                ctx.runtime_state(),
                &modules::core::MODULE_NAME,
            ));
            store.insert(modules::core::state::METADATA, metadata);
        }
    }

    /// Start the runtime.
    fn start()
    where
        Self: Sized + Send + Sync + 'static,
    {
        // Initializer.
        let init = |protocol: &Arc<Protocol>,
                    rak: &Arc<RAK>,
                    _rpc_demux: &mut RpcDemux,
                    rpc: &mut RpcDispatcher|
         -> Option<Box<dyn TxnDispatcher>> {
            // Fetch host information and configure domain separation context.
            let hi = protocol.get_host_info();
            crypto::signature::context::set_chain_context(
                hi.runtime_id,
                &hi.consensus_chain_context,
            );

            // Cobble together a keymanager client.
            let key_manager = Self::trusted_policy_signers().map(|signers| {
                Arc::new(KeyManagerClient::new(
                    hi.runtime_id,
                    protocol.clone(),
                    rak.clone(),
                    rpc,
                    4096,
                    signers,
                ))
            });

            // Register runtime's methods.
            let dispatcher = dispatcher::Dispatcher::<Self>::new(hi, key_manager, protocol.clone());
            Some(Box::new(dispatcher))
        };

        // Configure the runtime features.
        let mut features = Features::default();
        if let Some(cfg) = Self::SCHEDULE_CONTROL {
            features.schedule_control = Some(FeatureScheduleControl {
                initial_batch_size: cfg.initial_batch_size,
            });
        }

        // Start the runtime.
        start_runtime(
            Box::new(init),
            Config {
                version: Self::VERSION,
                trust_root: Self::consensus_trust_root(),
                features: Some(features),
                ..Default::default()
            },
        );
    }
}
