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
    consensus::roothash::MessageEvent,
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
    context::{Context, DispatchContext, TxContext},
    error::{Error as _, RuntimeError},
    module::{AuthHandler, BlockHandler, MessageHandlerRegistry, MethodRegistry},
    modules,
    modules::core::API as _,
    runtime::Runtime,
    storage, types,
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
    /// Method registry.
    methods: MethodRegistry,
    /// Handlers registered for consensus messages.
    consensus_message_handlers: MessageHandlerRegistry,

    _runtime: PhantomData<R>,
}

impl<R: Runtime> Dispatcher<R> {
    pub(super) fn new(
        methods: MethodRegistry,
        consensus_message_handlers: MessageHandlerRegistry,
    ) -> Self {
        Self {
            methods,
            consensus_message_handlers,
            _runtime: PhantomData,
        }
    }

    fn decode_tx(
        &self,
        ctx: &mut DispatchContext<'_>,
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

    fn dispatch_call(
        &self,
        ctx: &mut TxContext<'_, '_>,
        call: types::transaction::Call,
    ) -> types::transaction::CallResult {
        // Use up gas for authenticating the transaction.
        if let Err(e) = modules::core::Module::use_gas_for_auth(ctx) {
            return e.to_call_result();
        }

        // Perform transaction method lookup.
        let method_info = match self.methods.lookup_callable(&call.method) {
            Some(method_info) => method_info,
            None => {
                // Method not found.
                return modules::core::Error::InvalidMethod.to_call_result();
            }
        };

        (method_info.handler)(&method_info, ctx, call.body)
    }

    fn estimate_gas(
        &self,
        ctx: &mut DispatchContext<'_>,
        tx: types::transaction::Transaction,
    ) -> Result<u64, modules::core::Error> {
        ctx.with_tx_simulation(tx, |mut tx_ctx, call| {
            self.dispatch_call(&mut tx_ctx, call);
            // Warning: we don't report success or failure. If the call fails, we still report
            // how much gas it uses while it fails.
            Ok(modules::core::Module::tx_gas_used(&mut tx_ctx))
        })
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

        let (result, messages) = ctx.with_tx(tx, |mut ctx, call| {
            let result = self.dispatch_call(&mut ctx, call);
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
        let tx = match self.decode_tx(ctx, &tx) {
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

        match self.dispatch_tx(ctx, tx)?.result {
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

    fn execute_tx(
        &self,
        ctx: &mut DispatchContext<'_>,
        tx: &[u8],
    ) -> Result<ExecuteTxResult, Error> {
        // It is an error to include a malformed transaction in a batch. So instead of only
        // reporting a failed execution result, we fail the whole batch. This will make the compute
        // node vote for failure and the round will fail.
        //
        // Correct proposers should only include transactions which have passed check_tx.
        let tx = self
            .decode_tx(ctx, &tx)
            .map_err(Error::MalformedTransactionInBatch)?;

        let dispatch_result = self.dispatch_tx(ctx, tx)?;

        Ok(ExecuteTxResult {
            output: cbor::to_vec(&dispatch_result.result),
            tags: dispatch_result.tags,
        })
    }

    fn dispatch_message(
        &self,
        ctx: &mut DispatchContext<'_>,
        handler_name: String,
        message_event: MessageEvent,
        handler_ctx: cbor::Value,
    ) -> Result<(), modules::core::Error> {
        // Perform message handler lookup.
        let method_info = self
            .consensus_message_handlers
            .lookup_handler(&handler_name)
            .ok_or(modules::core::Error::InvalidMethod)?;

        (method_info.handler)(&method_info, ctx, message_event, handler_ctx);

        Ok(())
    }

    fn handle_last_round_messages(
        &self,
        ctx: &mut DispatchContext<'_>,
    ) -> Result<(), modules::core::Error> {
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
            self.dispatch_message(ctx, handler.hook_name, event, handler.payload)?;
        }

        if !handlers.is_empty() {
            error!(ctx.get_logger("dispatcher"), "message handler not invoked"; "unhandled" => ?handlers);
            return Err(modules::core::Error::MessageHandlerNotInvoked);
        }

        Ok(())
    }

    fn save_emitted_message_handlers<S: storage::Store>(
        &self,
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

    fn maybe_init_state(&self, ctx: &mut DispatchContext<'_>) {
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
            let mut ctx = DispatchContext::from_runtime(&rt_ctx, mkvs);
            // Perform state migrations if required.
            self.maybe_init_state(&mut ctx);

            // Handle last round message results.
            self.handle_last_round_messages(&mut ctx)?;

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
            let (block_tags, messages) = ctx.commit();
            let (messages, handlers) = messages.into_iter().unzip();

            let state = storage::MKVSStore::new(rt_ctx.io_ctx.clone(), mkvs);
            self.save_emitted_message_handlers(state, handlers);

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
            let mut ctx = DispatchContext::from_runtime(&ctx, mkvs);
            // Perform state migrations if required.
            self.maybe_init_state(&mut ctx);

            if method == "core.EstimateGas" {
                // Run a transaction in simulation and return how much gas it uses. This looks up the method
                // in the context's method registry. Transactions that fail still use gas, and this query will
                // estimate that and return successfully, so do not use this query to see if a transaction will
                // succeed. Failure due to OutOfGas are included, so it's best to set the query argument
                // transaction's gas to something high.
                let tx =
                    cbor::from_value(args).map_err(|_| modules::core::Error::InvalidArgument)?;
                let gas = self.estimate_gas(&mut ctx, tx)?;
                return Ok(cbor::to_value(gas));
            }

            // Execute the query.
            let method_info = self
                .methods
                .lookup_query(method)
                .ok_or(modules::core::Error::InvalidMethod)?;
            (method_info.handler)(&method_info, &mut ctx, args)
        })
    }
}

#[cfg(test)]
mod test {
    use oasis_core_runtime::common::cbor;

    use crate::{
        context,
        crypto::multisig,
        module,
        modules::core::{self, Module as Core, API as _},
        testing::{keys, mock},
        types::{token, transaction},
        Runtime, Version,
    };

    use super::Dispatcher;

    const AUTH_SIGNATURE_GAS: u64 = 1;
    const AUTH_MULTISIG_GAS: u64 = 10;
    const CALL_GAS: u64 = 100;

    struct Runtime1;

    impl Runtime for Runtime1 {
        const VERSION: Version = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        type Modules = (Core,);

        fn genesis_state() -> (core::Genesis,) {
            (core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: u64::MAX,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: core::GasCosts {
                        auth_signature: AUTH_SIGNATURE_GAS,
                        auth_multisig_signer: AUTH_MULTISIG_GAS,
                    },
                },
            },)
        }
    }

    #[test]
    fn test_query_estimate_gas() {
        const METHOD_WASTE_GAS: &str = "test.WasteGas";
        let mut methods = module::MethodRegistry::new();
        methods.register_callable(module::CallableMethodInfo {
            name: METHOD_WASTE_GAS,
            handler: |_mi, ctx, _args| {
                Core::use_gas(ctx, CALL_GAS).expect("use_gas should succeed");
                transaction::CallResult::Ok(cbor::Value::Null)
            },
        });
        let consensus_message_handlers = module::MessageHandlerRegistry::new();
        let dispatcher = Dispatcher::<Runtime1>::new(methods, consensus_message_handlers);

        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();
        ctx.mode = context::Mode::CheckTx;
        dispatcher.maybe_init_state(&mut ctx);

        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                method: METHOD_WASTE_GAS.to_owned(),
                body: cbor::Value::Null,
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![
                    transaction::SignerInfo::new(keys::alice::pk(), 0),
                    transaction::SignerInfo::new_multisig(
                        multisig::Config {
                            signers: vec![multisig::Signer {
                                public_key: keys::bob::pk(),
                                weight: 1,
                            }],
                            threshold: 1,
                        },
                        0,
                    ),
                ],
                fee: transaction::Fee {
                    amount: token::BaseUnits::new(0.into(), token::Denomination::NATIVE),
                    gas: u64::MAX,
                },
            },
        };

        let est = dispatcher
            .estimate_gas(&mut ctx, tx)
            .expect("estimate_gas should succeed");
        let reference_gas = AUTH_SIGNATURE_GAS + AUTH_MULTISIG_GAS + CALL_GAS;
        assert_eq!(est, reference_gas, "estimated gas should be correct");
    }
}
