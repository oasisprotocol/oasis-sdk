//! Runtime.
use std::sync::Arc;

use oasis_core_runtime::{
    common::version,
    config::Config,
    consensus::verifier::TrustRoot,
    dispatcher::{PostInitState, PreInitState},
    start_runtime,
    types::{FeatureScheduleControl, Features},
};

use crate::{
    config,
    context::Context,
    crypto, dispatcher,
    keymanager::{KeyManagerClient, TrustedPolicySigners},
    module::{
        BlockHandler, InvariantHandler, MethodHandler, MigrationHandler, ModuleInfoHandler,
        TransactionHandler,
    },
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

    /// Runtime schedule control configuration.
    const SCHEDULE_CONTROL: config::ScheduleControl = config::ScheduleControl::default();

    /// Module that provides the core API.
    type Core: modules::core::API;

    /// Supported modules.
    type Modules: TransactionHandler
        + MigrationHandler
        + MethodHandler
        + BlockHandler
        + InvariantHandler
        + ModuleInfoHandler;

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

    /// Whether a given query method is allowed to be invoked.
    fn is_allowed_query(_method: &str) -> bool {
        true
    }

    /// Whether a given query method is allowed to access private key manager state.
    ///
    /// Note that even if this returns `true` for a method, the method also needs to be tagged as
    /// being allowed to access private key manager state (e.g. with `allow_private_km`).
    fn is_allowed_private_km_query(_method: &str) -> bool {
        true
    }

    /// Whether a given call is allowed to be invoked interactively.
    ///
    /// Note that even if this returns `true` for a method, the method also needs to be tagged as
    /// being allowed to be executed interactively (e.g. with `allow_interactive`)
    fn is_allowed_interactive_call(_method: &str) -> bool {
        true
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
        if global_version != Self::STATE_VERSION && !ctx.is_check_only() {
            assert!(
                // There should either be no state, or it should be the previous version.
                global_version == 0 || global_version == Self::STATE_VERSION - 1,
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
        let init = |state: PreInitState<'_>| -> PostInitState {
            // Fetch host information and configure domain separation context.
            let hi = state.protocol.get_host_info();
            crypto::signature::context::set_chain_context(
                hi.runtime_id,
                &hi.consensus_chain_context,
            );

            // Cobble together a keymanager client.
            let key_manager = Self::trusted_policy_signers().map(|signers| {
                Arc::new(KeyManagerClient::new(
                    hi.runtime_id,
                    state.protocol.clone(),
                    state.consensus_verifier.clone(),
                    state.rak.clone(),
                    state.rpc_dispatcher,
                    4096,
                    signers,
                ))
            });

            // Register runtime's methods.
            let dispatcher = dispatcher::Dispatcher::<Self>::new(
                hi,
                key_manager,
                state.consensus_verifier.clone(),
                state.protocol.clone(),
            );

            PostInitState {
                txn_dispatcher: Some(Box::new(dispatcher)),
            }
        };

        // Configure the runtime features.
        let features = Features {
            schedule_control: Some(FeatureScheduleControl {
                initial_batch_size: Self::SCHEDULE_CONTROL.initial_batch_size,
            }),
        };

        // Start the runtime.
        start_runtime(
            Box::new(init),
            Config {
                version: Self::VERSION,
                trust_root: Self::consensus_trust_root(),
                features: Some(features),
                persist_check_tx_state: false,
                ..Default::default()
            },
        );
    }
}
