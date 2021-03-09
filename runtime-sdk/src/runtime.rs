//! Runtime.
use std::sync::Arc;

use oasis_core_runtime::{
    common::version, rak::RAK, start_runtime, Protocol, RpcDemux, RpcDispatcher, TxnDispatcher,
};

use crate::{
    context::DispatchContext,
    dispatcher,
    module::{
        AuthHandler, BlockHandler, MethodRegistrationHandler, MethodRegistry, MigrationHandler,
    },
    modules, storage,
};

/// A runtime.
pub trait Runtime {
    /// Runtime version.
    const VERSION: version::Version;

    type Modules: AuthHandler + MigrationHandler + MethodRegistrationHandler + BlockHandler;

    /// Genesis state for the runtime.
    fn genesis_state() -> <Self::Modules as MigrationHandler>::Genesis;

    /// Perform state migrations if required.
    fn migrate(ctx: &mut DispatchContext<'_>) {
        let store = storage::TypedStore::new(storage::PrefixStore::new(
            ctx.runtime_state(),
            &modules::core::MODULE_NAME,
        ));
        let mut metadata: modules::core::types::Metadata = store
            .get(modules::core::state::METADATA)
            .unwrap_or_default();

        // Perform state migrations/initialization on all modules.
        // TODO: Could this be optimized to not needlessly generate genesis state?
        let has_changes =
            Self::Modules::init_or_migrate(ctx, &mut metadata, &Self::genesis_state());

        // If there are any changes, update metadata.
        if has_changes {
            let mut store = storage::TypedStore::new(storage::PrefixStore::new(
                ctx.runtime_state(),
                &modules::core::MODULE_NAME,
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
        let init = |_protocol: &Arc<Protocol>,
                    _rak: &Arc<RAK>,
                    _rpc_demux: &mut RpcDemux,
                    _rpc: &mut RpcDispatcher|
         -> Option<Box<dyn TxnDispatcher>> {
            // TODO: Initialize global context.
            // protocol.get_runtime_id();
            // protocol.get_consensus_chain_context();
            // crypto::context::set_chain_context(protocol.get_runtime_id(), protocol.get_consensus_chain_context());

            // Register runtime's methods.
            let mut methods = MethodRegistry::new();
            Self::Modules::register_methods(&mut methods);

            let dispatcher = dispatcher::Dispatcher::<Self>::new(methods);
            Some(Box::new(dispatcher))
        };

        // Start the runtime.
        start_runtime(Box::new(init), Self::VERSION);
    }
}
