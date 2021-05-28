//! Transaction dispatcher.
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    sync::{atomic::AtomicBool, Arc},
};

use slog::error;
use thiserror::Error;

use oasis_core_runtime::{
    self,
    common::cbor,
    storage::{context::StorageContext, mkvs},
    transaction::{
        self,
        dispatcher::{ExecuteBatchResult, ExecuteTxResult},
        tags::Tags,
        types::TxnBatch,
    },
    types::CheckTxResult,
};

use crate::{
    context::{BatchContext, Context, RuntimeBatchContext},
    error::{Error as _, RuntimeError},
    module::{self, AuthHandler, BlockHandler, MethodHandler},
    modules,
    runtime::Runtime,
    storage, types, TxContext,
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
    MalformedTransactionInBatch(#[source] modules::core::Error),
}

pub struct DispatchResult {
    pub result: types::transaction::CallResult,
    pub tags: Tags,
}

impl From<types::transaction::CallResult> for DispatchResult {
    fn from(v: types::transaction::CallResult) -> Self {
        Self {
            result: v,
            tags: Tags::new(),
        }
    }
}

pub struct Dispatcher<R: Runtime> {
    _runtime: PhantomData<R>,
}

impl<R: Runtime> Dispatcher<R> {
    pub(super) fn new() -> Self {
        Self {
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
        let utx: types::transaction::UnverifiedTransaction =
            cbor::from_slice(&tx).map_err(|_| modules::core::Error::MalformedTransaction)?;

        // Perform any checks before signature verification.
        R::Modules::approve_unverified_tx(ctx, &utx)?;

        // Verify transaction signatures.
        // TODO: Support signature verification of the whole transaction batch.
        utx.verify()
            .map_err(|_| modules::core::Error::MalformedTransaction)
    }

    /// Run the dispatch steps inside a transaction context. This includes the before call hooks
    /// and the call itself.
    pub fn dispatch_tx_call<C: TxContext>(
        ctx: &mut C,
        call: types::transaction::Call,
    ) -> types::transaction::CallResult {
        if let Err(e) = R::Modules::before_handle_call(ctx, &call) {
            return e.to_call_result();
        }

        match R::Modules::dispatch_call(ctx, &call.method, call.body) {
            module::DispatchResult::Handled(result) => result,
            module::DispatchResult::Unhandled(_) => {
                modules::core::Error::InvalidMethod.to_call_result()
            }
        }
    }

    /// Dispatch a runtime transaction in the given context.
    pub fn dispatch_tx<C: BatchContext>(
        ctx: &mut C,
        tx: types::transaction::Transaction,
    ) -> Result<DispatchResult, Error> {
        // Run pre-processing hooks.
        if let Err(err) = R::Modules::authenticate_tx(ctx, &tx) {
            return Ok(err.to_call_result().into());
        }

        let (result, messages) = ctx.with_tx(tx, |mut ctx, call| {
            let result = Self::dispatch_tx_call(&mut ctx, call);
            if !result.is_success() {
                return (result.into(), Vec::new());
            }

            // Commit store and return emitted tags and messages.
            let (tags, messages) = ctx.commit();

            (DispatchResult { result, tags }, messages)
        });

        // Forward any emitted messages.
        ctx.emit_messages(messages)
            .expect("per-tx context has already enforced the limits");

        Ok(result)
    }

    /// Check whether the given transaction is valid.
    pub fn check_tx<C: BatchContext>(ctx: &mut C, tx: &[u8]) -> Result<CheckTxResult, Error> {
        let tx = match Self::decode_tx(ctx, &tx) {
            Ok(tx) => tx,
            Err(err) => {
                return Ok(CheckTxResult {
                    error: RuntimeError {
                        module: err.module_name().to_string(),
                        code: err.code(),
                        message: err.to_string(),
                    },
                    meta: None,
                })
            }
        };

        match Self::dispatch_tx(ctx, tx)?.result {
            types::transaction::CallResult::Ok(value) => Ok(CheckTxResult {
                error: Default::default(),
                meta: Some(value),
            }),

            types::transaction::CallResult::Failed {
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
        }
    }

    /// Execute the given transaction.
    pub fn execute_tx<C: BatchContext>(ctx: &mut C, tx: &[u8]) -> Result<ExecuteTxResult, Error> {
        // It is an error to include a malformed transaction in a batch. So instead of only
        // reporting a failed execution result, we fail the whole batch. This will make the compute
        // node vote for failure and the round will fail.
        //
        // Correct proposers should only include transactions which have passed check_tx.
        let tx = Self::decode_tx(ctx, &tx).map_err(Error::MalformedTransactionInBatch)?;

        let dispatch_result = Self::dispatch_tx(ctx, tx)?;

        Ok(ExecuteTxResult {
            output: cbor::to_vec(&dispatch_result.result),
            tags: dispatch_result.tags,
        })
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

            R::Modules::dispatch_message_result(
                ctx,
                &handler.hook_name,
                types::message::MessageResult {
                    event,
                    context: handler.payload,
                },
            )
            .ok_or(modules::core::Error::InvalidMethod)?;
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
        store.insert(&modules::core::state::MESSAGE_HANDLERS, &message_handlers);
    }

    fn maybe_init_state<C: Context>(ctx: &mut C) {
        R::migrate(ctx)
    }
}

impl<R: Runtime> transaction::dispatcher::Dispatcher for Dispatcher<R> {
    fn execute_batch(
        &self,
        rt_ctx: transaction::Context<'_>,
        batch: &TxnBatch,
    ) -> Result<ExecuteBatchResult, RuntimeError> {
        // TODO: Get rid of StorageContext (pass mkvs in ctx).
        StorageContext::with_current(|mkvs, _| {
            // Prepare dispatch context.
            let mut ctx =
                RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                    &rt_ctx, mkvs,
                );
            // Perform state migrations if required.
            Self::maybe_init_state(&mut ctx);

            // Handle last round message results.
            Self::handle_last_round_messages(&mut ctx)?;

            // Run begin block hooks.
            R::Modules::begin_block(&mut ctx);

            // Execute the batch.
            let mut results = Vec::with_capacity(batch.len());
            for tx in batch.iter() {
                results.push(Self::execute_tx(&mut ctx, &tx)?);
            }

            // Run end block hooks.
            R::Modules::end_block(&mut ctx);

            // Commit the context and retrieve the emitted messages.
            let (block_tags, messages) = ctx.commit();
            let (messages, handlers) = messages.into_iter().unzip();

            let state = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), mkvs);
            Self::save_emitted_message_handlers(state, handlers);

            Ok(ExecuteBatchResult {
                results,
                messages,
                block_tags,
            })
        })
    }

    fn check_batch(
        &self,
        ctx: transaction::Context<'_>,
        batch: &TxnBatch,
    ) -> Result<Vec<CheckTxResult>, RuntimeError> {
        // TODO: Get rid of StorageContext (pass mkvs in ctx).
        StorageContext::with_current(|mkvs, _| {
            // Prepare dispatch context.
            let mut ctx =
                RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                    &ctx, mkvs,
                );
            // Perform state migrations if required.
            Self::maybe_init_state(&mut ctx);

            // Check the batch.
            let mut results = Vec::with_capacity(batch.len());
            for tx in batch.iter() {
                results.push(Self::check_tx(&mut ctx, &tx)?);
            }

            Ok(results)
        })
    }

    fn set_abort_batch_flag(&mut self, _abort_batch: Arc<AtomicBool>) {
        // TODO: Implement support for graceful batch aborts (oasis-sdk#129).
    }

    fn query(
        &self,
        ctx: transaction::Context<'_>,
        method: &str,
        args: cbor::Value,
    ) -> Result<cbor::Value, RuntimeError> {
        // TODO: Get rid of StorageContext (pass mkvs in ctx).
        StorageContext::with_current(|mkvs, _| {
            // Prepare dispatch context.
            let mut ctx =
                RuntimeBatchContext::<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>>::from_runtime(
                    &ctx, mkvs,
                );
            // Perform state migrations if required.
            Self::maybe_init_state(&mut ctx);

            // Execute the query.
            R::Modules::dispatch_query(&mut ctx, method, args)
                .ok_or(modules::core::Error::InvalidMethod)?
        })
    }
}
