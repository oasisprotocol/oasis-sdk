//! Runtime.
use std::sync::Arc;

use oasis_core_runtime::{
    common::version, rak::RAK, start_runtime, Protocol, RpcDemux, RpcDispatcher, TxnDispatcher,
};

use crate::{
    context::Context,
    crypto, dispatcher,
    module::{AuthHandler, BlockHandler, InvariantHandler, MethodHandler, MigrationHandler},
    modules, storage,
};

/// A runtime.
pub trait Runtime {
    /// Runtime version.
    const VERSION: version::Version;
    /// State version.
    const STATE_VERSION: u32 = 0;

    type Modules: AuthHandler + MigrationHandler + MethodHandler + BlockHandler + InvariantHandler;

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
            modules::core::MODULE_NAME.as_bytes(),
        ));
        let mut metadata: modules::core::types::Metadata = store
            .get(modules::core::state::METADATA)
            .unwrap_or_default();

        // Perform state migrations/initialization on all modules.
        let mut has_changes =
            Self::Modules::init_or_migrate(ctx, &mut metadata, &Self::genesis_state());

        // Check if we need to also apply any global state updates.
        let global_version = metadata
            .versions
            .get(modules::core::types::VERSION_GLOBAL_KEY)
            .copied()
            .unwrap_or_default();
        if global_version != Self::STATE_VERSION {
            if global_version != Self::STATE_VERSION - 1 {
                panic!(
                    "inconsistent existing state version (expected: {} got: {})",
                    Self::STATE_VERSION - 1,
                    global_version
                );
            }

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
                modules::core::MODULE_NAME.as_bytes(),
            ));
            store.insert(modules::core::state::METADATA, &metadata);
        }
    }

    /// Start the runtime.
    fn start()
    where
        Self: Sized + 'static,
    {
        // Initializer.
        let init = |protocol: &Arc<Protocol>,
                    _rak: &Arc<RAK>,
                    _rpc_demux: &mut RpcDemux,
                    _rpc: &mut RpcDispatcher|
         -> Option<Box<dyn TxnDispatcher>> {
            // Fetch host information and configure domain separation context.
            let hi = protocol.get_host_info();
            crypto::signature::context::set_chain_context(
                hi.runtime_id,
                &hi.consensus_chain_context,
            );

            // Register runtime's methods.
            let dispatcher = dispatcher::Dispatcher::<Self>::new(hi);
            Some(Box::new(dispatcher))
        };

        // Start the runtime.
        start_runtime(Box::new(init), Self::VERSION);
    }
}
