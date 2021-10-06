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
    protocol::HostInfo,
    storage::mkvs,
    transaction::{
        self,
        dispatcher::{ExecuteBatchResult, ExecuteTxResult},
        tags::Tags,
        types::TxnBatch,
    },
    types::{CheckTxMetadata, CheckTxResult, BATCH_WEIGHT_LIMIT_QUERY_METHOD},
    BUILD_INFO,
};

use crate::{
    callformat,
    context::{BatchContext, Context, RuntimeBatchContext, TxContext},
    error::{Error as _, RuntimeError},
    event::IntoTags,
    keymanager::{KeyManagerClient, KeyManagerError},
    module::{self, AuthHandler, BlockHandler, MethodHandler},
    modules,
    modules::core::API as _,
    runtime::Runtime,
    schedule_control::ScheduleControlHost,
    storage,
    storage::Prefix,
    types,
    types::transaction::{AuthProof, Transaction, TransactionWeight},
};

/// Unique module name.
const MODULE_NAME: &str = "dispatcher";

/// Error emitted by the dispatch process. Note that this indicates an error in the dispatch
/// process itself and should not be used for any transaction-related errors.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
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
pub struct DispatchResult {
    /// Transaction call result.
    pub result: module::CallResult,
    /// Transaction tags.
    pub tags: Tags,
    /// Transaction priority.
    pub priority: u64,
    /// Transaction weights.
    pub weights: BTreeMap<TransactionWeight, u64>,
    /// Call format metadata.
    pub call_format_metadata: callformat::Metadata,
}

impl DispatchResult {
    fn new(result: module::CallResult, call_format_metadata: callformat::Metadata) -> Self {
        Self {
            result,
            tags: Tags::new(),
            priority: 0,
            weights: BTreeMap::new(),
            call_format_metadata,
        }
    }
}

impl From<module::CallResult> for DispatchResult {
    fn from(result: module::CallResult) -> Self {
        Self::new(result, callformat::Metadata::Empty)
    }
}

/// The runtime dispatcher.
pub struct Dispatcher<R: Runtime> {
    host_info: HostInfo,
    key_manager: Option<Arc<KeyManagerClient>>,
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
        schedule_control_host: Arc<dyn ScheduleControlHost>,
    ) -> Self {
        Self {
            host_info,
            key_manager,
            schedule_control_host,
            _runtime: PhantomData,
        }
    }

    /// Decode a runtime transaction.
    pub fn decode_tx<C: Context>(
        ctx: &mut C,
        tx: &[u8],
    ) -> Result<types::transaction::Transaction, modules::core::Error> {
        // TODO: Check against transaction size limit.

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

    /// Run the dispatch steps inside a transaction context. This includes the before call hooks
    /// and the call itself.
    pub fn dispatch_tx_call<C: TxContext>(
        ctx: &mut C,
        call: types::transaction::Call,
    ) -> module::CallResult {
        if let Err(e) = R::Modules::before_handle_call(ctx, &call) {
            return e.into_call_result();
        }

        match R::Modules::dispatch_call(ctx, &call.method, call.body) {
            module::DispatchResult::Handled(result) => result,
            module::DispatchResult::Unhandled(_) => {
                modules::core::Error::InvalidMethod(call.method).into_call_result()
            }
        }
    }

    /// Dispatch a runtime transaction in the given context.
    pub fn dispatch_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx: types::transaction::Transaction,
        index: usize,
    ) -> Result<DispatchResult, Error> {
        // Run pre-processing hooks.
        if let Err(err) = R::Modules::authenticate_tx(ctx, &tx) {
            return Ok(err.into_call_result().into());
        }

        let (result, messages) = ctx.with_tx(tx_size, tx, |mut ctx, call| {
            // Decode call based on specified call format.
            let (call, call_format_metadata) = match callformat::decode_call(&ctx, call, index) {
                Ok(Some(result)) => result,
                Ok(None) => {
                    return (
                        module::CallResult::Ok(cbor::Value::Simple(cbor::SimpleValue::NullValue))
                            .into(),
                        vec![],
                    )
                }
                Err(err) => return (err.into_call_result().into(), vec![]),
            };

            let result = Self::dispatch_tx_call(&mut ctx, call);
            if !result.is_success() {
                return (
                    DispatchResult::new(result, call_format_metadata),
                    Vec::new(),
                );
            }

            // Load priority, weights.
            let priority = R::Core::take_priority(&mut ctx);
            let weights = R::Core::take_weights(&mut ctx);

            // Commit store and return emitted tags and messages.
            let (etags, messages) = ctx.commit();
            (
                DispatchResult {
                    result,
                    tags: etags.into_tags(),
                    priority,
                    weights,
                    call_format_metadata,
                },
                messages,
            )
        });

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

    /// Check whether the given transaction is valid.
    pub fn check_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx: Transaction,
    ) -> Result<CheckTxResult, Error> {
        let dispatch = Self::dispatch_tx(ctx, tx_size, tx, usize::MAX)?;
        match dispatch.result {
            module::CallResult::Ok(_) => Ok(CheckTxResult {
                error: Default::default(),
                meta: Some(CheckTxMetadata {
                    priority: dispatch.priority,
                    weights: Some(dispatch.weights),
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

    /// Execute the given transaction.
    pub fn execute_tx<C: BatchContext>(
        ctx: &mut C,
        tx_size: u32,
        tx: Transaction,
        index: usize,
    ) -> Result<ExecuteTxResult, Error> {
        let dispatch_result = Self::dispatch_tx(ctx, tx_size, tx, index)?;
        let output: types::transaction::CallResult = callformat::encode_result(
            ctx,
            dispatch_result.result,
            dispatch_result.call_format_metadata,
        );

        Ok(ExecuteTxResult {
            output: cbor::to_vec(output),
            tags: dispatch_result.tags,
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

        let store = storage::TypedStore::new(storage::PrefixStore::new(
            ctx.runtime_state(),
            &modules::core::MODULE_NAME,
        ));
        let mut handlers: BTreeMap<u32, types::message::MessageEventHookInvocation> = store
            .get(&modules::core::state::MESSAGE_HANDLERS)
            .unwrap_or_default();

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

    fn save_emitted_message_handlers<S: storage::Store>(
        store: S,
        handlers: Vec<types::message::MessageEventHookInvocation>,
    ) {
        let message_handlers: BTreeMap<u32, types::message::MessageEventHookInvocation> = handlers
            .into_iter()
            .enumerate()
            .map(|(idx, h)| (idx as u32, h))
            .collect();

        let mut store = storage::TypedStore::new(storage::PrefixStore::new(
            store,
            &modules::core::MODULE_NAME,
        ));
        store.insert(&modules::core::state::MESSAGE_HANDLERS, message_handlers);
    }

    /// Process the given runtime query.
    pub fn dispatch_query<C: BatchContext>(
        ctx: &mut C,
        method: &str,
        args: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        let args = cbor::from_slice(&args)
            .map_err(|err| modules::core::Error::InvalidArgument(err.into()))?;

        // Catch any panics that occur during query dispatch.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Perform state migrations if required.
            R::migrate(ctx);

            // Execute the query.
            match method {
                // Internal methods.
                BATCH_WEIGHT_LIMIT_QUERY_METHOD => {
                    let block_weight_limits = R::Modules::get_block_weight_limits(ctx);
                    Ok(cbor::to_value(block_weight_limits))
                }
                // Runtime methods.
                _ => {
                    // Even though queries are not allowed to perform any private key ops, we
                    // reduce the attack surface by completely disallowing them in secure builds.
                    //
                    // If one needs to perform runtime queries, she should use a regular build of
                    // the runtime for that.
                    if BUILD_INFO.is_secure {
                        return Err(modules::core::Error::ForbiddenInSecureBuild.into());
                    }

                    R::Modules::dispatch_query(ctx, method, args)
                        .ok_or_else(|| modules::core::Error::InvalidMethod(method.into()))?
                }
            }
        }))
        .map_err(|err| -> RuntimeError { Error::QueryAborted(format!("{:?}", err)).into() })?
        .map(cbor::to_vec)
    }

    fn execute_batch_common<F>(
        &self,
        mut rt_ctx: transaction::Context<'_>,
        f: F,
    ) -> Result<ExecuteBatchResult, RuntimeError>
    where
        F: FnOnce(
            &mut RuntimeBatchContext<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>,
        ) -> Result<Vec<ExecuteTxResult>, RuntimeError>,
    {
        // Prepare dispatch context.
        let key_manager = self
            .key_manager
            .as_ref()
            // NOTE: We are explicitly allowing private key operations during execution.
            .map(|mgr| mgr.with_private_context(rt_ctx.io_ctx.clone()));
        let mut ctx =
            RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                &mut rt_ctx,
                &self.host_info,
                key_manager,
            );

        // Perform state migrations if required.
        R::migrate(&mut ctx);

        // Handle last round message results.
        Self::handle_last_round_messages(&mut ctx)?;

        // Run begin block hooks.
        R::Modules::begin_block(&mut ctx);

        let results = f(&mut ctx)?;

        // Run end block hooks.
        R::Modules::end_block(&mut ctx);

        // Query block weight limits for next round.
        let block_weight_limits = R::Modules::get_block_weight_limits(&mut ctx);

        // Commit the context and retrieve the emitted messages.
        let (block_tags, messages) = ctx.commit();
        let (messages, handlers) = messages.into_iter().unzip();

        let state = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), &mut rt_ctx.runtime_state);
        Self::save_emitted_message_handlers(state, handlers);

        Ok(ExecuteBatchResult {
            results,
            messages,
            block_tags: block_tags.into_tags(),
            batch_weight_limits: Some(block_weight_limits),
            tx_reject_hashes: vec![],
        })
    }
}

impl<R: Runtime + Send + Sync> transaction::dispatcher::Dispatcher for Dispatcher<R> {
    fn execute_batch(
        &self,
        rt_ctx: transaction::Context<'_>,
        batch: &TxnBatch,
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
                    // It is an error to include a malformed transaction in a batch. So instead of only
                    // reporting a failed execution result, we fail the whole batch. This will make the compute
                    // node vote for failure and the round will fail.
                    //
                    // Correct proposers should only include transactions which have passed check_tx.
                    let tx = Self::decode_tx(ctx, tx)
                        .map_err(|err| Error::MalformedTransactionInBatch(err.into()))?;
                    txs.push((tx_size, tx.clone()));

                    if prefetch_enabled {
                        Self::prefetch_tx(&mut prefixes, tx)?;
                    }
                }
                if prefetch_enabled {
                    ctx.runtime_state()
                        .prefetch_prefixes(prefixes.into_iter().collect(), R::PREFETCH_LIMIT);
                }

                // Execute the batch.
                let mut results = Vec::with_capacity(batch.len());
                for (index, (tx_size, tx)) in txs.into_iter().enumerate() {
                    results.push(Self::execute_tx(ctx, tx_size, tx, index)?);
                }

                Ok(results)
            },
        )
    }

    fn schedule_and_execute_batch(
        &self,
        rt_ctx: transaction::Context<'_>,
        batch: &mut TxnBatch,
    ) -> Result<ExecuteBatchResult, RuntimeError> {
        let cfg = R::SCHEDULE_CONTROL.unwrap(); // Must succeed otherwise we wouldn't be here.
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
                        let tx = match Self::decode_tx(ctx, &raw_tx) {
                            Ok(tx) => tx,
                            Err(_) => {
                                // Transaction is malformed, make sure it gets removed from the
                                // queue and don't include it in a block.
                                tx_reject_hashes.push(Hash::digest_bytes(&raw_tx));
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
                        let index = new_batch.len();

                        new_batch.push(raw_tx);
                        results.push(Self::execute_tx(ctx, tx_size, tx, index)?);
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
        mut ctx: transaction::Context<'_>,
        batch: &TxnBatch,
    ) -> Result<Vec<CheckTxResult>, RuntimeError> {
        // If prefetch limit is set enable prefetch.
        let prefetch_enabled = R::PREFETCH_LIMIT > 0;

        // Prepare dispatch context.
        let key_manager = self
            .key_manager
            .as_ref()
            .map(|mgr| mgr.with_context(ctx.io_ctx.clone()));
        let mut ctx =
            RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                &mut ctx,
                &self.host_info,
                key_manager,
            );

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
            ctx.runtime_state()
                .prefetch_prefixes(prefixes.into_iter().collect(), R::PREFETCH_LIMIT);
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
    }

    fn set_abort_batch_flag(&mut self, _abort_batch: Arc<AtomicBool>) {
        // TODO: Implement support for graceful batch aborts (oasis-sdk#129).
    }

    fn query(
        &self,
        mut ctx: transaction::Context<'_>,
        method: &str,
        args: Vec<u8>,
    ) -> Result<Vec<u8>, RuntimeError> {
        // Prepare dispatch context.
        let key_manager = self
            .key_manager
            .as_ref()
            .map(|mgr| mgr.with_context(ctx.io_ctx.clone()));
        let mut ctx =
            RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                &mut ctx,
                &self.host_info,
                key_manager,
            );

        Self::dispatch_query(&mut ctx, method, args)
    }
}
