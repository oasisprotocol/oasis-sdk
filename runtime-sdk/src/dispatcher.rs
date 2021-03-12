//! Transaction dispatcher.
use std::{
    marker::PhantomData,
    sync::{atomic::AtomicBool, Arc},
};

use thiserror::Error;

use oasis_core_runtime::{
    self,
    common::cbor,
    storage::context::StorageContext,
    transaction::{
        self,
        dispatcher::{ExecuteBatchResult, ExecuteTxResult},
        tags::Tags,
        types::TxnBatch,
    },
    types::CheckTxResult,
};

use crate::{
    context::DispatchContext,
    error::{Error as _, RuntimeError},
    module::{AuthHandler, BlockHandler, MethodRegistry},
    modules,
    runtime::Runtime,
    types,
};

/// Error emitted by the dispatch process. Note that this indicates an error in the dispatch
/// process itself and should not be used for any transaction-related errors.
#[derive(Error, Debug)]
pub enum Error {
    #[error("dispatch aborted")]
    Aborted,
}

impl From<Error> for RuntimeError {
    fn from(err: Error) -> RuntimeError {
        RuntimeError::new("dispatcher", 1, &format!("{}", err))
    }
}

struct DispatchResult {
    result: types::transaction::CallResult,
    tags: Tags,
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
    /// Abort batch flag.
    abort_batch: Option<Arc<AtomicBool>>,
    /// Method registry.
    methods: MethodRegistry,

    _runtime: PhantomData<R>,
}

impl<R: Runtime> Dispatcher<R> {
    pub(super) fn new(methods: MethodRegistry) -> Self {
        Self {
            abort_batch: None,
            methods,
            _runtime: PhantomData,
        }
    }

    fn decode_tx(
        &self,
        tx: &[u8],
    ) -> Result<types::transaction::Transaction, modules::core::Error> {
        // TODO: Check against transaction size limit.

        // Deserialize transaction.
        let utx: types::transaction::UnverifiedTransaction =
            cbor::from_slice(&tx).map_err(|_| modules::core::Error::MalformedTransaction)?;

        // Verify transaction signatures.
        // TODO: Support signature verification of the whole transaction batch.
        utx.verify()
            .map_err(|_| modules::core::Error::MalformedTransaction)
    }

    fn dispatch_tx(
        &self,
        ctx: &mut DispatchContext<'_>,
        tx: types::transaction::Transaction,
    ) -> Result<DispatchResult, Error> {
        // Run pre-processing hooks.
        if let Err(err) = R::Modules::authenticate_tx(ctx, &tx) {
            return Ok(err.to_call_result().into());
        }

        // Perform transaction method lookup.
        let mi = match self.methods.lookup_callable(&tx.call.method) {
            Some(mi) => mi,
            None => {
                // Method not found.
                return Ok(modules::core::Error::InvalidMethod.to_call_result().into());
            }
        };

        let (result, messages) = ctx.with_tx(tx, |mut ctx, call| {
            let result = (mi.handler)(&mi, &mut ctx, call.body);
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

    fn check_tx(&self, ctx: &mut DispatchContext<'_>, tx: &[u8]) -> Result<CheckTxResult, Error> {
        let tx = match self.decode_tx(&tx) {
            Ok(tx) => tx,
            Err(err) => {
                return Ok(CheckTxResult {
                    error: RuntimeError {
                        module: err.module().to_string(),
                        code: err.code(),
                        message: err.to_string(),
                    },
                    meta: None,
                })
            }
        };

        match self.dispatch_tx(ctx, tx)?.result {
            types::transaction::CallResult::Ok(value) => Ok(CheckTxResult {
                error: Default::default(),
                meta: Some(value),
            }),

            types::transaction::CallResult::Failed { module, code } => Ok(CheckTxResult {
                error: RuntimeError {
                    module,
                    code,
                    message: Default::default(),
                },
                meta: None,
            }),
        }
    }

    fn execute_tx(
        &self,
        ctx: &mut DispatchContext<'_>,
        tx: &[u8],
    ) -> Result<ExecuteTxResult, Error> {
        let tx = match self.decode_tx(&tx) {
            Ok(tx) => tx,
            Err(err) => {
                return Ok(ExecuteTxResult {
                    output: cbor::to_vec(&err.to_call_result()),
                    tags: Tags::new(),
                })
            }
        };

        let dispatch_result = self.dispatch_tx(ctx, tx)?;

        Ok(ExecuteTxResult {
            output: cbor::to_vec(&dispatch_result.result),
            tags: dispatch_result.tags,
        })
    }

    fn maybe_init_state(&self, ctx: &mut DispatchContext<'_>) {
        R::migrate(ctx)
    }
}

impl<R: Runtime> transaction::dispatcher::Dispatcher for Dispatcher<R> {
    fn execute_batch(
        &self,
        ctx: transaction::Context<'_>,
        batch: &TxnBatch,
    ) -> Result<ExecuteBatchResult, RuntimeError> {
        // TODO: Get rid of StorageContext (pass mkvs in ctx).
        StorageContext::with_current(|mkvs, _| {
            // Prepare dispatch context.
            let mut ctx = DispatchContext::from_runtime(&ctx, mkvs);
            // Perform state migrations if required.
            self.maybe_init_state(&mut ctx);

            // Run begin block hooks.
            R::Modules::begin_block(&mut ctx);

            // Execute the batch.
            let mut results = Vec::with_capacity(batch.len());
            for tx in batch.iter() {
                results.push(self.execute_tx(&mut ctx, &tx)?);
            }

            // Run end block hooks.
            R::Modules::end_block(&mut ctx);

            // Commit the context and retrieve the emitted messages.
            let messages = ctx.commit();

            Ok(ExecuteBatchResult { results, messages })
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
            let mut ctx = DispatchContext::from_runtime(&ctx, mkvs);
            // Perform state migrations if required.
            self.maybe_init_state(&mut ctx);

            // Check the batch.
            let mut results = Vec::with_capacity(batch.len());
            for tx in batch.iter() {
                results.push(self.check_tx(&mut ctx, &tx)?);
            }

            Ok(results)
        })
    }

    fn set_abort_batch_flag(&mut self, abort_batch: Arc<AtomicBool>) {
        self.abort_batch = Some(abort_batch);
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
            let mut ctx = DispatchContext::from_runtime(&ctx, mkvs);

            // Execute the query.
            let mi = self
                .methods
                .lookup_query(method)
                .ok_or(modules::core::Error::InvalidMethod)?;
            (mi.handler)(&mi, &mut ctx, args)
        })
    }
}
