//! Exposed EnclaveRPC methods.
use std::{marker::PhantomData, sync::Arc};

use anyhow::{anyhow, bail, Result};

use crate::{
    context::RuntimeBatchContext,
    core::{
        consensus::{
            roothash::Header,
            state::{
                beacon::ImmutableState as BeaconState, registry::ImmutableState as RegistryState,
                roothash::ImmutableState as RoothashState,
            },
            verifier::Verifier,
        },
        enclave_rpc::{
            dispatcher::{
                Dispatcher as RpcDispatcher, Method as RpcMethod,
                MethodDescriptor as RpcMethodDescriptor,
            },
            types::Kind as RpcKind,
            Context as RpcContext,
        },
        future::block_on,
        protocol::{HostInfo, Protocol},
        storage::mkvs,
    },
    dispatcher,
    keymanager::KeyManagerClient,
    module::MethodHandler,
    state::{self, CurrentState},
    storage::HostStore,
    Runtime,
};

/// Name of the `query` method.
pub const METHOD_QUERY: &str = "runtime-sdk/query";

/// Arguments for the `query` method.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct QueryRequest {
    pub round: u64,
    pub method: String,
    pub args: Vec<u8>,
}

/// EnclaveRPC dispatcher wrapper.
pub(crate) struct Wrapper<R: Runtime> {
    host_info: HostInfo,
    host: Arc<Protocol>,
    key_manager: Option<Arc<KeyManagerClient>>,
    consensus_verifier: Arc<dyn Verifier>,
    _runtime: PhantomData<R>,
}

impl<R> Wrapper<R>
where
    R: Runtime + Send + Sync + 'static,
{
    pub(crate) fn wrap(
        rpc: &mut RpcDispatcher,
        host: Arc<Protocol>,
        host_info: HostInfo,
        key_manager: Option<Arc<KeyManagerClient>>,
        consensus_verifier: Arc<dyn Verifier>,
    ) {
        let wrapper = Box::leak(Box::new(Self {
            host_info,
            host,
            key_manager,
            consensus_verifier,
            _runtime: PhantomData,
        }));
        rpc.add_methods(wrapper.methods());
    }

    fn methods(&'static self) -> Vec<RpcMethod> {
        vec![RpcMethod::new(
            RpcMethodDescriptor {
                name: METHOD_QUERY.to_string(),
                kind: RpcKind::NoiseSession,
            },
            move |ctx: &_, req: &_| self.rpc_query(ctx, req),
        )]
    }

    fn ensure_session_endorsed(&self, ctx: &RpcContext) -> Result<()> {
        let endorsed_by = ctx
            .session_info
            .as_ref()
            .ok_or(anyhow!("not authorized"))?
            .endorsed_by
            .ok_or(anyhow!("not endorsed by host"))?;
        let host_identity = self
            .host
            .get_identity()
            .ok_or(anyhow!("local identity not available"))?
            .node_identity()
            .ok_or(anyhow!("node identity not available"))?;
        if endorsed_by != host_identity {
            bail!("not endorsed by host");
        }
        Ok(())
    }

    fn rpc_query(&self, ctx: &RpcContext, req: &QueryRequest) -> Result<Vec<u8>> {
        self.ensure_session_endorsed(ctx)?;

        // Determine whether the method is allowed to access confidential state and provide an
        // appropriately scoped instance of the key manager client.
        let is_confidential_allowed = R::Modules::is_allowed_private_km_query(&req.method)
            && R::is_allowed_private_km_query(&req.method);
        let key_manager = self.key_manager.as_ref().map(|mgr| {
            if is_confidential_allowed {
                mgr.with_private_context()
            } else {
                mgr.with_context()
            }
        });

        // Fetch latest consensus layer state.
        let state = block_on(self.consensus_verifier.latest_state())?;
        let roothash = RoothashState::new(&state);
        let roots = roothash
            .round_roots(self.host_info.runtime_id, req.round)?
            .ok_or(anyhow!("root not found"))?;
        let beacon = BeaconState::new(&state);
        let epoch = beacon.epoch()?;
        let registry = RegistryState::new(&state);
        let runtime = registry
            .runtime(&self.host_info.runtime_id)?
            .ok_or(anyhow!("runtime not found"))?;

        // Prepare dispatch context.
        let history = self.consensus_verifier.clone();
        let root = HostStore::new(
            self.host.clone(),
            mkvs::Root {
                namespace: self.host_info.runtime_id,
                version: req.round,
                root_type: mkvs::RootType::State,
                hash: roots.state_root,
            },
        );
        // TODO: This is currently limited as we have no nice way of getting a good known header. We
        // need to expose more stuff in roothash and then limit the query to latest round. Until
        // then any queries requiring access to features like timestamp will fail as we need to
        // ensure we use safe values for these arguments.
        let header = Header {
            namespace: self.host_info.runtime_id,
            round: req.round,
            io_root: roots.io_root,
            state_root: roots.state_root,
            ..Default::default()
        };
        let round_results = Default::default();
        let max_messages = runtime.executor.max_messages;

        let ctx = RuntimeBatchContext::<'_, R>::new(
            &self.host_info,
            key_manager,
            &header,
            &round_results,
            &state,
            &history,
            epoch,
            max_messages,
        );

        CurrentState::enter_opts(
            state::Options::new()
                .with_mode(state::Mode::Check)
                .with_rng_local_entropy(), // Mix in local (private) entropy for queries.
            root,
            || dispatcher::Dispatcher::<R>::dispatch_query(&ctx, &req.method, req.args.clone()),
        )
        .map_err(Into::into)
    }
}
