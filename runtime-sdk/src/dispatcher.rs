//! Transaction dispatcher.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    marker::PhantomData,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::anyhow;
use slog::error;
use thiserror::Error;

use oasis_core_runtime::{
    self,
    common::crypto::hash::Hash,
    consensus::{roothash, verifier::Verifier},
    protocol::HostInfo,
    transaction::{
        self,
        dispatcher::{ExecuteBatchResult, ExecuteTxResult},
        tags::Tags,
        types::TxnBatch,
    },
    types::{CheckTxMetadata, CheckTxResult},
};

use crate::{
    callformat,
    context::{BatchContext, Context, Mode, RuntimeBatchContext, TransactionWithMeta, TxContext},
    crypto::random::RootRng,
    error::{Error as _, RuntimeError},
    event::IntoTags,
    keymanager::{KeyManagerClient, KeyManagerError},
    module::{self, BlockHandler, MethodHandler, TransactionHandler},
    modules,
    modules::core::API as _,
    runtime::Runtime,
    schedule_control::ScheduleControlHost,
    sender::SenderMeta,
    storage::{self, current::TransactionResult, CurrentStore, Prefix},
    types,
    types::transaction::{AuthProof, Transaction},
};

/// Unique module name.
const MODULE_NAME: &str = "dispatcher";

/// Error emitted by the dispatch process. Note that this indicates an error in the dispatch
/// process itself and should not be used for any transaction-related errors.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
#[sdk_error(abort_self)]
pub enum Error {
    #[error("dispatch aborted")]
    #[sdk_error(code = 1)]
    Aborted,

    #[error("malformed transaction in batch: {0}")]
    #[sdk_error(code = 2)]
    MalformedTransactionInBatch(#[source] anyhow::Error),

    #[error("query aborted: {0}")]
    #[sdk_error(code = 3)]
    QueryAborted(String),

    #[error("key manager failure: {0}")]
    #[sdk_error(code = 4)]
    KeyManagerFailure(#[from] KeyManagerError),

    #[error("batch out of gas")]
    #[sdk_error(code = 5)]
    BatchOutOfGas,
}

/// Result of dispatching a transaction.
#[derive(Debug)]
pub struct DispatchResult {
    /// Transaction call result.
    pub result: module::CallResult,
    /// Transaction tags.
    pub tags: Tags,
    /// Transaction priority.
    pub priority: u64,
    /// Transaction sender metadata.
    pub sender_metadata: SenderMeta,
    /// Call format metadata.
    pub call_format_metadata: callformat::Metadata,
}

impl DispatchResult {
    fn new(
        result: module::CallResult,
        tags: Tags,
        call_format_metadata: callformat::Metadata,
    ) -> Self {
        Self {
            result,
            tags,
            priority: 0,
            sender_metadata: Default::default(),
            call_format_metadata,
        }
    }
}

impl From<module::CallResult> for DispatchResult {
    fn from(result: module::CallResult) -> Self {
        Self::new(result, vec![], callformat::Metadata::Empty)
    }
}

/// Additional options for dispatch operations.
#[derive(Default)]
pub struct DispatchOptions<'a> {
    /// Transaction size.
    pub tx_size: u32,
    /// Transaction index within the batch.
    pub tx_index: usize,
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Optionally only allow methods for which the provided authorizer closure returns true.
    pub method_authorizer: Option<&'a dyn Fn(&str) -> bool>,
    /// Optionally skip authentication.
    pub skip_authentication: bool,
}

/// The runtime dispatcher.
pub struct Dispatcher<R: Runtime> {
    host_info: HostInfo,
    key_manager: Option<Arc<KeyManagerClient>>,
    consensus_verifier: Arc<dyn Verifier>,
    schedule_control_host: Arc<dyn ScheduleControlHost>,
    _runtime: PhantomData<R>,
}

impl<R: Runtime> Dispatcher<R> {
    /// Create a new instance of the dispatcher for the given runtime.
    ///
    /// Note that the dispatcher is fully static and the constructor is only needed so that the
    /// instance can be used directly with the dispatcher system provided by Oasis Core.
    pub(super) fn new(
        host_info: HostInfo,
        key_manager: Option<Arc<KeyManagerClient>>,
        consensus_verifier: Arc<dyn Verifier>,
        schedule_control_host: Arc<dyn ScheduleControlHost>,
    ) -> Self {
        Self {
            host_info,
            key_manager,
            consensus_verifier,
            schedule_control_host,
            _runtime: PhantomData,
        }
    }

    /// Decode a runtime transaction.
    pub fn decode_tx<C: Context>(
        ctx: &mut C,
        tx: &[u8],
    ) -> Result<types::transaction::Transaction, modules::core::Error> {
        // Perform any checks before decoding.
        R::Modules::approve_raw_tx(ctx, tx)?;

        // Deserialize transaction.
        let utx: types::transaction::UnverifiedTransaction = cbor::from_slice(tx)
            .map_err(|e| modules::core::Error::MalformedTransaction(e.into()))?;

        // Perform any checks before signature verification.
        R::Modules::approve_unverified_tx(ctx, &utx)?;

        match utx.1.as_slice() {
            [AuthProof::Module(scheme)] => {
                R::Modules::decode_tx(ctx, scheme, &utx.0)?.ok_or_else(|| {
                    modules::core::Error::MalformedTransaction(anyhow!(
                        "module-controlled transaction decoding scheme {} not supported",
                        scheme
                    ))
                })
            }
            _ => utx
                .verify()
                .map_err(|e| modules::core::Error::MalformedTransaction(e.into())),
        }
    }

    /// Run the dispatch steps inside a transaction context. This includes the before call hooks,
    /// the call itself and after call hooks. The after call hooks are called regardless if the call
    /// succeeds or not.
    pub fn dispatch_tx_call<C: TxContext>(
        ctx: &mut C,
        call: types::transaction::Call,
        opts: &DispatchOptions<'_>,
    ) -> (module::CallResult, callformat::Metadata) {
        let read_only = call.read_only;

        // Dispatch the call.
        let (result, metadata) = Self::_dispatch_tx_call(ctx, call, opts);

        // Unconditionally call after handle call hook.
        let result = match R::Modules::after_handle_call(ctx, result) {
            Ok(result) => result,
            Err(e) => {
                // If the call failed, return the error.
                return (e.into_call_result(), metadata);
            }
        };

        // Make sure that a read-only call did not result in any modifications.
        if read_only && CurrentStore::has_pending_updates() {
            return (
                modules::core::Error::ReadOnlyTransaction.into_call_result(),
                metadata,
            );
        }

        (result, metadata)
    }

    fn _dispatch_tx_call<C: TxContext>(
        ctx: &mut C,
        call: types::transaction::Call,
        opts: &DispatchOptions<'_>,
    ) -> (module::CallResult, callformat::Metadata) {
        if let Err(e) = R::Modules::before_handle_call(ctx, &call) {
            return (e.into_call_result(), callformat::Metadata::Empty);
        }

        // Decode call based on specified call format.
        let (call, call_format_metadata) = match callformat::decode_call(ctx, call, ctx.tx_index())
        {
            Ok(Some(result)) => result,
            Ok(None) => {
                return (
                    module::CallResult::Ok(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
                    callformat::Metadata::Empty,
                )
            }
            Err(err) => return (err.into_call_result(), callformat::Metadata::Empty),
        };

        // Apply optional method authorization.
        if let Some(method_authorizer) = opts.method_authorizer {
            if !method_authorizer(&call.method) {
                return (
                    modules::core::Error::Forbidden.into_call_result(),
                    call_format_metadata,
                );
            }
        }

        let result = match R::Modules::dispatch_call(ctx, &call.method, call.body) {
            module::DispatchResult::Handled(result) => result,
            module::DispatchResult::Unhandled(_) => {
                modules::core::Error::InvalidMethod(call.method).into_call_result()
            }
        };

        (result, call_format_metadata)
    }

    /// Dispatch a runtime transaction in the given context with the provided options.
    pub fn dispatch_tx_opts<C: BatchContext>(
        ctx: &mut C,
        tx: types::transaction::Transaction,
        opts: &DispatchOptions<'_>,
    ) -> Result<DispatchResult, Error> {
        // Run pre-processing hooks.
        if !opts.skip_authentication {
            if let Err(err) = R::Modules::authenticate_tx(ctx, &tx) {
                return Ok(err.into_call_result().into());
            }
        }
        let tx_auth_info = tx.auth_info.clone();
        let is_read_only = tx.call.read_only;

        let (result, messages) = CurrentStore::with_transaction(|| {
            ctx.with_tx(
                TransactionWithMeta {
                    tx,
                    tx_size: opts.tx_size,
                    tx_index: opts.tx_index,
                    tx_hash: opts.tx_hash,
                },
                |mut ctx, call| {
                    let (result, call_format_metadata) =
                        Self::dispatch_tx_call(&mut ctx, call, opts);
                    if !result.is_success() || is_read_only {
                        // Retrieve unconditional events by doing an explicit rollback.
                        let etags = ctx.rollback();

                        return TransactionResult::Rollback((
                            DispatchResult::new(result, etags.into_tags(), call_format_metadata),
                            Vec::new(),
                        ));
                    }

                    // Load priority.
                    let priority = R::Core::take_priority(&mut ctx);
                    // Load sender metadata.
                    let sender_metadata = R::Core::take_sender_meta(&mut ctx);

                    if ctx.is_check_only() {
                        // Rollback state during checks.
                        ctx.rollback();

                        TransactionResult::Rollback((
                            DispatchResult {
                                result,
                                tags: Vec::new(),
                                priority,
                                sender_metadata,
                                call_format_metadata,
                            },
                            Vec::new(),
                        ))
                    } else {
                        // Commit store and return emitted tags and messages.
                        let state = ctx.commit();
                        TransactionResult::Commit((
                            DispatchResult {
                                result,
                                tags: state.events.into_tags(),
                                priority,
                                sender_metadata,
                                call_format_metadata,
                            },
                            state.messages,
                        ))
                    }
                },
            )
        });

        // Run after dispatch hooks.
        R::Modules::after_dispatch_tx(ctx, &tx_auth_info, &result.result);

        // Propagate batch aborts.
        if let module::CallResult::Aborted(err) = result.result {
            return Err(err);
        }

        // Forward any emitted messages if we are not in check tx context.
        if !ctx.is_check_only() {
            ctx.emit_messages(messages)
                .expect("per-tx context has already enforced the limits");
        }

        Ok(result)
    }

    /// Dispatch a runtime transaction in the given context.
    pub fn dispatch_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx: types::transaction::Transaction,
        tx_index: usize,
    ) -> Result<DispatchResult, Error> {
        Self::dispatch_tx_opts(
            ctx,
            tx,
            &DispatchOptions {
                tx_size,
                tx_index,
                ..Default::default()
            },
        )
    }

    /// Check whether the given transaction is valid.
    pub fn check_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx: Transaction,
    ) -> Result<CheckTxResult, Error> {
        let dispatch = ctx.with_child(Mode::CheckTx, |mut ctx| {
            Self::dispatch_tx(&mut ctx, tx_size, tx, usize::MAX)
        })?;
        match dispatch.result {
            module::CallResult::Ok(_) => Ok(CheckTxResult {
                error: Default::default(),
                meta: Some(CheckTxMetadata {
                    priority: dispatch.priority,
                    sender: dispatch.sender_metadata.id(),
                    sender_seq: dispatch.sender_metadata.tx_nonce,
                    sender_state_seq: dispatch.sender_metadata.state_nonce,
                }),
            }),

            module::CallResult::Failed {
                module,
                code,
                message,
            } => Ok(CheckTxResult {
                error: RuntimeError {
                    module,
                    code,
                    message,
                },
                meta: None,
            }),

            module::CallResult::Aborted(err) => Err(err),
        }
    }

    /// Execute the given transaction, returning unserialized results.
    pub fn execute_tx_opts<C: BatchContext>(
        ctx: &mut C,
        tx: Transaction,
        opts: &DispatchOptions<'_>,
    ) -> Result<(types::transaction::CallResult, Tags), Error> {
        let dispatch_result = Self::dispatch_tx_opts(ctx, tx, opts)?;
        let output: types::transaction::CallResult = callformat::encode_result(
            ctx,
            dispatch_result.result,
            dispatch_result.call_format_metadata,
        );

        Ok((output, dispatch_result.tags))
    }

    /// Execute the given transaction.
    pub fn execute_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx_hash: Hash,
        tx: Transaction,
        tx_index: usize,
    ) -> Result<ExecuteTxResult, Error> {
        let (output, tags) = Self::execute_tx_opts(
            ctx,
            tx,
            &DispatchOptions {
                tx_size,
                tx_index,
                tx_hash,
                ..Default::default()
            },
        )?;

        Ok(ExecuteTxResult {
            output: cbor::to_vec(output),
            tags,
        })
    }

    /// Prefetch prefixes for the given transaction.
    pub fn prefetch_tx(
        prefixes: &mut BTreeSet<Prefix>,
        tx: types::transaction::Transaction,
    ) -> Result<(), RuntimeError> {
        match R::Modules::prefetch(prefixes, &tx.call.method, tx.call.body, &tx.auth_info) {
            module::DispatchResult::Handled(r) => r,
            module::DispatchResult::Unhandled(_) => Ok(()), // Unimplemented prefetch is allowed.
        }
    }

    fn handle_last_round_messages<C: Context>(ctx: &mut C) -> Result<(), modules::core::Error> {
        let message_events = ctx.runtime_round_results().messages.clone();

        let mut handlers = CurrentStore::with(|store| {
            let store = storage::TypedStore::new(storage::PrefixStore::new(
                store,
                &modules::core::MODULE_NAME,
            ));
            let handlers: BTreeMap<u32, types::message::MessageEventHookInvocation> = store
                .get(modules::core::state::MESSAGE_HANDLERS)
                .unwrap_or_default();

            handlers
        });

        for event in message_events {
            let handler = handlers
                .remove(&event.index)
                .ok_or(modules::core::Error::MessageHandlerMissing(event.index))?;
            let hook_name = handler.hook_name.clone();

            R::Modules::dispatch_message_result(
                ctx,
                &hook_name,
                types::message::MessageResult {
                    event,
                    context: handler.payload,
                },
            )
            .ok_or(modules::core::Error::InvalidMethod(hook_name))?;
        }

        if !handlers.is_empty() {
            error!(ctx.get_logger("dispatcher"), "message handler not invoked"; "unhandled" => ?handlers);
            return Err(modules::core::Error::MessageHandlerNotInvoked);
        }

        Ok(())
    }

    fn save_emitted_message_handlers(handlers: Vec<types::message::MessageEventHookInvocation>) {
        let message_handlers: BTreeMap<u32, types::message::MessageEventHookInvocation> = handlers
            .into_iter()
            .enumerate()
            .map(|(idx, h)| (idx as u32, h))
            .collect();

        CurrentStore::with(|store| {
            let mut store = storage::TypedStore::new(storage::PrefixStore::new(
                store,
                &modules::core::MODULE_NAME,
            ));
            store.insert(modules::core::state::MESSAGE_HANDLERS, message_handlers);
        });
    }

    /// Process the given runtime query.
    pub fn dispatch_query<C: BatchContext>(
        ctx: &mut C,
        method: &str,
        args: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let args = cbor::from_slice(&args)
            .map_err(|err| modules::core::Error::InvalidArgument(err.into()))?;

        CurrentStore::with_transaction(|| {
            // Catch any panics that occur during query dispatch.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Perform state migrations if required.
                R::migrate(ctx);

                if !R::is_allowed_query(method) || !ctx.is_allowed_query::<R>(method) {
                    return Err(modules::core::Error::Forbidden.into());
                }

                R::Modules::dispatch_query(ctx, method, args)
                    .ok_or_else(|| modules::core::Error::InvalidMethod(method.into()))?
            }));

            // Always rollback any changes to storage. Note that this is usually a no-op because
            // Oasis Core would rollback any storage changes related to queries, but this makes it
            // explicit to ensure this remains the case regardless of upstream changes.
            TransactionResult::Rollback(result)
        })
        .map_err(|err| -> RuntimeError { Error::QueryAborted(format!("{err:?}")).into() })?
        .map(cbor::to_vec)
    }

    fn execute_batch_common<F>(
        &self,
        mut rt_ctx: transaction::Context<'_>,
        f: F,
    ) -> Result<ExecuteBatchResult, RuntimeError>
    where
        F: FnOnce(&mut RuntimeBatchContext<'_, R>) -> Result<Vec<ExecuteTxResult>, RuntimeError>,
    {
        // Prepare dispatch context.
        let key_manager = self
            .key_manager
            .as_ref()
            // NOTE: We are explicitly allowing private key operations during execution.
            .map(|mgr| mgr.with_private_context(rt_ctx.io_ctx.clone()));
        let history = self.consensus_verifier.clone();
        let rng = RootRng::new();

        let root = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), &mut rt_ctx.runtime_state);
        let mut ctx = RuntimeBatchContext::<'_, R>::new(
            Mode::ExecuteTx,
            &self.host_info,
            key_manager,
            rt_ctx.header,
            rt_ctx.round_results,
            &rt_ctx.consensus_state,
            &history,
            rt_ctx.epoch,
            &rng,
            rt_ctx.io_ctx.clone(),
            rt_ctx.max_messages,
        );

        CurrentStore::enter(root, || {
            // Perform state migrations if required.
            R::migrate(&mut ctx);

            // Handle last round message results.
            Self::handle_last_round_messages(&mut ctx)?;

            // Run begin block hooks.
            R::Modules::begin_block(&mut ctx);

            let results = f(&mut ctx)?;

            // Run end block hooks.
            R::Modules::end_block(&mut ctx);

            // Commit the context and retrieve the emitted messages.
            let state = ctx.commit();
            let (messages, handlers) = state.messages.into_iter().unzip();
            Self::save_emitted_message_handlers(handlers);

            Ok(ExecuteBatchResult {
                results,
                messages,
                block_tags: state.events.into_tags(),
                tx_reject_hashes: vec![],
                in_msgs_count: 0, // TODO: Support processing incoming messages.
            })
        })
    }
}

impl<R: Runtime + Send + Sync> transaction::dispatcher::Dispatcher for Dispatcher<R> {
    fn execute_batch(
        &self,
        rt_ctx: transaction::Context<'_>,
        batch: &TxnBatch,
        _in_msgs: &[roothash::IncomingMessage],
    ) -> Result<ExecuteBatchResult, RuntimeError> {
        self.execute_batch_common(
            rt_ctx,
            |ctx| -> Result<Vec<ExecuteTxResult>, RuntimeError> {
                // If prefetch limit is set enable prefetch.
                let prefetch_enabled = R::PREFETCH_LIMIT > 0;

                let mut txs = Vec::with_capacity(batch.len());
                let mut prefixes: BTreeSet<Prefix> = BTreeSet::new();
                for tx in batch.iter() {
                    let tx_size = tx.len().try_into().map_err(|_| {
                        Error::MalformedTransactionInBatch(anyhow!("transaction too large"))
                    })?;
                    let tx_hash = Hash::digest_bytes(tx);
                    // It is an error to include a malformed transaction in a batch. So instead of only
                    // reporting a failed execution result, we fail the whole batch. This will make the compute
                    // node vote for failure and the round will fail.
                    //
                    // Correct proposers should only include transactions which have passed check_tx.
                    let tx = Self::decode_tx(ctx, tx)
                        .map_err(|err| Error::MalformedTransactionInBatch(err.into()))?;
                    txs.push((tx_size, tx_hash, tx.clone()));

                    if prefetch_enabled {
                        Self::prefetch_tx(&mut prefixes, tx)?;
                    }
                }
                if prefetch_enabled {
                    CurrentStore::with(|store| {
                        store.prefetch_prefixes(prefixes.into_iter().collect(), R::PREFETCH_LIMIT);
                    })
                }

                // Execute the batch.
                let mut results = Vec::with_capacity(batch.len());
                for (index, (tx_size, tx_hash, tx)) in txs.into_iter().enumerate() {
                    results.push(Self::execute_tx(ctx, tx_size, tx_hash, tx, index)?);
                }

                Ok(results)
            },
        )
    }

    fn schedule_and_execute_batch(
        &self,
        rt_ctx: transaction::Context<'_>,
        batch: &mut TxnBatch,
        _in_msgs: &[roothash::IncomingMessage],
    ) -> Result<ExecuteBatchResult, RuntimeError> {
        let cfg = R::SCHEDULE_CONTROL;
        let mut tx_reject_hashes = Vec::new();

        let mut result = self.execute_batch_common(
            rt_ctx,
            |ctx| -> Result<Vec<ExecuteTxResult>, RuntimeError> {
                // Schedule and execute the batch.
                //
                // The idea is to keep scheduling transactions as long as we have some space
                // available in the block as determined by gas use.
                let mut new_batch = Vec::new();
                let mut results = Vec::with_capacity(batch.len());
                let mut requested_batch_len = cfg.initial_batch_size;
                'batch: loop {
                    // Remember length of last batch.
                    let last_batch_len = batch.len();
                    let last_batch_tx_hash = batch.last().map(|raw_tx| Hash::digest_bytes(raw_tx));

                    for raw_tx in batch.drain(..) {
                        // If we don't have enough gas for processing even the cheapest transaction
                        // we are done. Same if we reached the runtime-imposed maximum tx count.
                        let remaining_gas = R::Core::remaining_batch_gas(ctx);
                        if remaining_gas < cfg.min_remaining_gas
                            || new_batch.len() >= cfg.max_tx_count
                        {
                            break 'batch;
                        }

                        // Decode transaction.
                        let tx_hash = Hash::digest_bytes(&raw_tx);
                        let tx = match Self::decode_tx(ctx, &raw_tx) {
                            Ok(tx) => tx,
                            Err(_) => {
                                // Transaction is malformed, make sure it gets removed from the
                                // queue and don't include it in a block.
                                tx_reject_hashes.push(tx_hash);
                                continue;
                            }
                        };
                        let tx_size = raw_tx.len().try_into().unwrap();

                        // If we don't have enough gas remaining to process this transaction, just
                        // skip it.
                        if tx.auth_info.fee.gas > remaining_gas {
                            continue;
                        }
                        // Same if we don't have enough consensus message slots.
                        if tx.auth_info.fee.consensus_messages > ctx.remaining_messages() {
                            continue;
                        }

                        // Determine the current transaction index.
                        let tx_index = new_batch.len();

                        // First run the transaction in check tx mode in a separate subcontext. If
                        // that fails, skip and (sometimes) reject transaction.
                        let skip = CurrentStore::with_transaction(|| {
                            let result = ctx.with_pre_schedule(|mut ctx| -> Result<_, Error> {
                                // First authenticate the transaction to get any nonce related errors.
                                match R::Modules::authenticate_tx(&mut ctx, &tx) {
                                    Err(modules::core::Error::FutureNonce) => {
                                        // Only skip transaction as it may become valid in the future.
                                        return Ok(true);
                                    }
                                    Err(_) => {
                                        // Skip and reject the transaction.
                                    }
                                    Ok(_) => {
                                        // Run additional checks on the transaction.
                                        let check_result = Self::dispatch_tx_opts(
                                            &mut ctx,
                                            tx.clone(),
                                            &DispatchOptions {
                                                tx_size,
                                                tx_index,
                                                tx_hash,
                                                skip_authentication: true, // Already done.
                                                ..Default::default()
                                            },
                                        )?;
                                        if check_result.result.is_success() {
                                            // Checks successful, execute transaction as usual.
                                            return Ok(false);
                                        }
                                    }
                                }

                                // Skip and reject the transaction.
                                tx_reject_hashes.push(tx_hash);
                                Ok(true)
                            });

                            TransactionResult::Rollback(result) // Always rollback storage changes.
                        })?;
                        if skip {
                            continue;
                        }

                        new_batch.push(raw_tx);
                        results.push(Self::execute_tx(ctx, tx_size, tx_hash, tx, tx_index)?);
                    }

                    // If there's more room in the block and we got the maximum number of
                    // transactions, request more transactions.
                    if last_batch_tx_hash.is_some()
                        && last_batch_len >= requested_batch_len as usize
                    {
                        if let Some(fetched_batch) = self
                            .schedule_control_host
                            .fetch_tx_batch(last_batch_tx_hash, cfg.batch_size)?
                        {
                            *batch = fetched_batch;
                            requested_batch_len = cfg.batch_size;
                            continue;
                        }
                        // No more transactions, let's just finish.
                    }
                    break;
                }

                // Replace input batch with newly generated batch.
                *batch = new_batch.into();

                Ok(results)
            },
        )?;

        // Include rejected transaction hashes in the final result.
        result.tx_reject_hashes = tx_reject_hashes;

        Ok(result)
    }

    fn check_batch(
        &self,
        mut rt_ctx: transaction::Context<'_>,
        batch: &TxnBatch,
    ) -> Result<Vec<CheckTxResult>, RuntimeError> {
        // If prefetch limit is set enable prefetch.
        let prefetch_enabled = R::PREFETCH_LIMIT > 0;

        // Prepare dispatch context.
        let key_manager = self
            .key_manager
            .as_ref()
            .map(|mgr| mgr.with_context(rt_ctx.io_ctx.clone()));
        let history = self.consensus_verifier.clone();
        let rng = RootRng::new();

        let root = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), &mut rt_ctx.runtime_state);
        let mut ctx = RuntimeBatchContext::<'_, R>::new(
            Mode::CheckTx,
            &self.host_info,
            key_manager,
            rt_ctx.header,
            rt_ctx.round_results,
            &rt_ctx.consensus_state,
            &history,
            rt_ctx.epoch,
            &rng,
            rt_ctx.io_ctx.clone(),
            rt_ctx.max_messages,
        );

        CurrentStore::enter(root, || {
            // Perform state migrations if required.
            R::migrate(&mut ctx);

            // Prefetch.
            let mut txs: Vec<Result<_, RuntimeError>> = Vec::with_capacity(batch.len());
            let mut prefixes: BTreeSet<Prefix> = BTreeSet::new();
            for tx in batch.iter() {
                let tx_size = tx.len().try_into().map_err(|_| {
                    Error::MalformedTransactionInBatch(anyhow!("transaction too large"))
                })?;
                let res = match Self::decode_tx(&mut ctx, tx) {
                    Ok(tx) => {
                        if prefetch_enabled {
                            Self::prefetch_tx(&mut prefixes, tx.clone()).map(|_| (tx_size, tx))
                        } else {
                            Ok((tx_size, tx))
                        }
                    }
                    Err(err) => Err(err.into()),
                };
                txs.push(res);
            }
            if prefetch_enabled {
                CurrentStore::with(|store| {
                    store.prefetch_prefixes(prefixes.into_iter().collect(), R::PREFETCH_LIMIT);
                });
            }

            // Check the batch.
            let mut results = Vec::with_capacity(batch.len());
            for tx in txs.into_iter() {
                match tx {
                    Ok((tx_size, tx)) => results.push(Self::check_tx(&mut ctx, tx_size, tx)?),
                    Err(err) => results.push(CheckTxResult {
                        error: err,
                        meta: None,
                    }),
                }
            }

            Ok(results)
        })
    }

    fn set_abort_batch_flag(&mut self, _abort_batch: Arc<AtomicBool>) {
        // TODO: Implement support for graceful batch aborts (oasis-sdk#129).
    }

    fn query(
        &self,
        mut rt_ctx: transaction::Context<'_>,
        method: &str,
        args: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        // Determine whether the method is allowed to access confidential state and provide an
        // appropriately scoped instance of the key manager client.
        let is_confidential_allowed = R::Modules::is_allowed_private_km_query(method)
            && R::is_allowed_private_km_query(method);
        if is_confidential_allowed {
            // Perform consensus layer state integrity verification for any queries that allow
            // access to confidential state.
            self.consensus_verifier.verify_for_query(
                rt_ctx.consensus_block.clone(),
                rt_ctx.header.clone(),
                rt_ctx.epoch,
            )?;
            // Ensure the runtime is still ready to process requests.
            rt_ctx.protocol.ensure_initialized()?;
        }
        let key_manager = self.key_manager.as_ref().map(|mgr| {
            if is_confidential_allowed {
                mgr.with_private_context(rt_ctx.io_ctx.clone())
            } else {
                mgr.with_context(rt_ctx.io_ctx.clone())
            }
        });

        // Initialize the root RNG. For queries which don't need to be deterministic as they are
        // node-local we mix in local (private) entropy.
        let rng = RootRng::new();
        rng.append_local_entropy();

        // Prepare dispatch context.
        let history = self.consensus_verifier.clone();

        let root = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), &mut rt_ctx.runtime_state);
        let mut ctx = RuntimeBatchContext::<'_, R>::new(
            Mode::CheckTx,
            &self.host_info,
            key_manager,
            rt_ctx.header,
            rt_ctx.round_results,
            &rt_ctx.consensus_state,
            &history,
            rt_ctx.epoch,
            &rng,
            rt_ctx.io_ctx.clone(),
            rt_ctx.max_messages,
        );

        CurrentStore::enter(root, || Self::dispatch_query(&mut ctx, method, args))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        handler,
        module::Module,
        modules::core,
        sdk_derive,
        storage::{CurrentStore, Store},
        testing::{configmap, keys, mock::Mock},
        types::{token, transaction},
        Version,
    };
    use cbor::Encode as _;

    struct CoreConfig;
    impl core::Config for CoreConfig {}
    type Core = core::Module<CoreConfig>;

    #[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
    enum AlphabetError {
        #[error("{0}")]
        #[sdk_error(transparent, abort)]
        Core(#[source] core::Error),
    }

    /// A module with multiple no-op methods; intended for testing routing.
    struct AlphabetModule;

    #[sdk_derive(Module)]
    impl AlphabetModule {
        const NAME: &'static str = "alphabet";
        const VERSION: u32 = 42;
        type Error = AlphabetError;
        type Event = ();
        type Parameters = ();
        type Genesis = ();

        #[handler(call = "alphabet.ReadOnly")]
        fn read_only<C: TxContext>(_ctx: &mut C, _args: ()) -> Result<u64, AlphabetError> {
            CurrentStore::with(|store| {
                let _ = store.get(b"key"); // Read something and ignore result.
            });
            Ok(42)
        }

        #[handler(call = "alphabet.NotReadOnly")]
        fn not_read_only<C: TxContext>(_ctx: &mut C, _args: ()) -> Result<u64, AlphabetError> {
            CurrentStore::with(|store| {
                store.insert(b"key", b"value");
            });
            Ok(10)
        }

        #[handler(call = "alphabet.Aborting")]
        fn aborting<C: TxContext>(_ctx: &mut C, _args: ()) -> Result<(), AlphabetError> {
            // Use a deeply nested abort to make sure this is handled correctly.
            Err(AlphabetError::Core(core::Error::Abort(Error::Aborted)))
        }

        #[handler(query = "alphabet.Alpha")]
        fn alpha<C: Context>(_ctx: &mut C, _args: ()) -> Result<(), AlphabetError> {
            Ok(())
        }

        #[handler(query = "alphabet.Omega", expensive)]
        fn expensive<C: Context>(_ctx: &mut C, _args: ()) -> Result<(), AlphabetError> {
            // Nothing actually expensive here. We're just pretending for testing purposes.
            Ok(())
        }
    }

    impl module::BlockHandler for AlphabetModule {}
    impl module::TransactionHandler for AlphabetModule {}
    impl module::InvariantHandler for AlphabetModule {}

    struct AlphabetRuntime;

    impl Runtime for AlphabetRuntime {
        const VERSION: Version = Version::new(0, 0, 0);
        type Core = Core;
        type Modules = (Core, AlphabetModule);

        fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
            (
                core::Genesis {
                    parameters: core::Parameters {
                        max_batch_gas: u64::MAX,
                        max_tx_size: 32 * 1024,
                        max_tx_signers: 1,
                        max_multisig_signers: 8,
                        gas_costs: core::GasCosts {
                            tx_byte: 0,
                            auth_signature: 0,
                            auth_multisig_signer: 0,
                            callformat_x25519_deoxysii: 0,
                        },
                        min_gas_price: BTreeMap::from([(token::Denomination::NATIVE, 0)]),
                    },
                },
                (),
            )
        }
    }

    #[test]
    fn test_allowed_queries_defaults() {
        let mut mock = Mock::with_local_config(BTreeMap::new());
        let mut ctx = mock.create_ctx_for_runtime::<AlphabetRuntime>(Mode::CheckTx, false);

        Dispatcher::<AlphabetRuntime>::dispatch_query(
            &mut ctx,
            "alphabet.Alpha",
            cbor::to_vec(().into_cbor_value()),
        )
        .expect("alphabet.Alpha is an inexpensive query, allowed by default");

        Dispatcher::<AlphabetRuntime>::dispatch_query(
            &mut ctx,
            "alphabet.Omega",
            cbor::to_vec(().into_cbor_value()),
        )
        .expect_err("alphabet.Omega is an expensive query, disallowed by default");
    }

    #[test]
    fn test_allowed_queries_custom() {
        let local_config = configmap! {
            // Allow expensive gas estimation and expensive queries so they can be tested.
            "estimate_gas_by_simulating_contracts" => true,
            "allowed_queries" => vec![
                configmap! {"alphabet.Alpha" => false},
                configmap! {"all_expensive" => true},
                configmap! {"all" => true}  // should have no effect on Alpha
            ],
        };
        let mut mock = Mock::with_local_config(local_config);
        // For queries, oasis-core always generates a `CheckTx` context; test with that.
        let mut ctx = mock.create_ctx_for_runtime::<AlphabetRuntime>(Mode::CheckTx, false);

        Dispatcher::<AlphabetRuntime>::dispatch_query(
            &mut ctx,
            "alphabet.Alpha",
            cbor::to_vec(().into_cbor_value()),
        )
        .expect_err("alphabet.Alpha is a disallowed query");

        Dispatcher::<AlphabetRuntime>::dispatch_query(
            &mut ctx,
            "alphabet.Omega",
            cbor::to_vec(().into_cbor_value()),
        )
        .expect("alphabet.Omega is an expensive query and expensive queries are allowed");
    }

    #[test]
    fn test_dispatch_read_only_call() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx_for_runtime::<AlphabetRuntime>(Mode::ExecuteTx, false);

        AlphabetRuntime::migrate(&mut ctx);

        let mut tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                format: transaction::CallFormat::Plain,
                method: "alphabet.ReadOnly".to_owned(),
                read_only: true,
                ..Default::default()
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![transaction::SignerInfo::new_sigspec(
                    keys::alice::sigspec(),
                    0,
                )],
                fee: transaction::Fee {
                    amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                    gas: 1000,
                    consensus_messages: 0,
                },
                ..Default::default()
            },
        };

        // Dispatch read-only transaction.
        let dispatch_result =
            Dispatcher::<AlphabetRuntime>::dispatch_tx(&mut ctx, 1024, tx.clone(), 0)
                .expect("read only method dispatch should work");
        let result = dispatch_result.result.unwrap();
        let result: u64 = cbor::from_value(result).unwrap();
        assert_eq!(result, 42);

        // Dispatch read-only transaction of a method that writes.
        tx.call.method = "alphabet.NotReadOnly".to_owned();

        let dispatch_result = Dispatcher::<AlphabetRuntime>::dispatch_tx(&mut ctx, 1024, tx, 0)
            .expect("read only method dispatch should work");
        match dispatch_result.result {
            module::CallResult::Failed {
                module,
                code,
                message,
            } => {
                assert_eq!(&module, "core");
                assert_eq!(code, 25);
                assert_eq!(&message, "read-only transaction attempted modifications")
            }
            _ => panic!("not read only method execution did not fail"),
        }
    }

    #[test]
    fn test_dispatch_abort_forwarding() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx_for_runtime::<AlphabetRuntime>(Mode::ExecuteTx, false);

        AlphabetRuntime::migrate(&mut ctx);

        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                format: transaction::CallFormat::Plain,
                method: "alphabet.Aborting".to_owned(),
                ..Default::default()
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![transaction::SignerInfo::new_sigspec(
                    keys::alice::sigspec(),
                    0,
                )],
                fee: transaction::Fee {
                    amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                    gas: 1000,
                    consensus_messages: 0,
                },
                ..Default::default()
            },
        };

        // Dispatch transaction and make sure the abort gets propagated.
        let dispatch_result =
            Dispatcher::<AlphabetRuntime>::dispatch_tx(&mut ctx, 1024, tx.clone(), 0);
        assert!(matches!(dispatch_result, Err(Error::Aborted)));
    }
}
