//! Core definitions module.
use std::collections::BTreeMap;

use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    context::Mode,
    error,
    module::{self, QueryMethodInfo},
    types::transaction,
    DispatchContext, TxContext,
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
pub const MODULE_NAME: &str = "core";

/// Errors emitted by the core module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("malformed transaction")]
    #[sdk_error(code = 1)]
    MalformedTransaction,

    #[error("invalid transaction: {0}")]
    #[sdk_error(code = 2)]
    InvalidTransaction(#[from] transaction::Error),

    #[error("invalid method")]
    #[sdk_error(code = 3)]
    InvalidMethod,

    #[error("invalid nonce")]
    #[sdk_error(code = 4)]
    InvalidNonce,

    #[error("insufficient balance to pay fees")]
    #[sdk_error(code = 5)]
    InsufficientFeeBalance,

    #[error("invalid argument")]
    #[sdk_error(code = 6)]
    InvalidArgument,

    #[error("gas overflow")]
    #[sdk_error(code = 7)]
    GasOverflow,

    #[error("out of gas")]
    #[sdk_error(code = 8)]
    OutOfGas,
}

pub trait API {
    fn use_gas(ctx: &mut TxContext<'_, '_>, gas: u64) -> Result<(), Error>;
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
}

pub struct Module;

const CONTEXT_KEY_GAS_USED: &str = "core.GasUsed";

impl API for Module {
    fn use_gas(ctx: &mut TxContext<'_, '_>, gas: u64) -> Result<(), Error> {
        let gas_limit = ctx.tx_auth_info().fee.gas;
        let gas_used = ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED);
        let new_gas_used = match gas_used.overflowing_add(gas) {
            (new_gas_used, false) => new_gas_used,
            (_, true) => return Err(Error::GasOverflow),
        };
        if new_gas_used > gas_limit {
            return Err(Error::OutOfGas);
        }
        *gas_used = new_gas_used;
        Ok(())
    }
}

impl Module {
    fn query_estimate_gas(
        ctx: &mut DispatchContext<'_>,
        args: transaction::Transaction,
    ) -> Result<u64, Error> {
        let mi = ctx
            .methods
            .lookup_callable(&args.call.method)
            .ok_or(Error::InvalidMethod)?;
        let mut sim_ctx = DispatchContext {
            mode: Mode::SimulateTx,
            runtime_header: ctx.runtime_header,
            runtime_round_results: ctx.runtime_round_results,
            runtime_storage: ctx.runtime_storage,
            io_ctx: ctx.io_ctx.clone(),
            methods: ctx.methods,
            max_messages: ctx.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
        };
        Ok(sim_ctx.with_tx(args, |mut tx_ctx, call| {
            (mi.handler)(&mi, &mut tx_ctx, call.body);
            // Warning: we don't report success or failure. If the call fails, we still report how
            // much gas it uses while it fails.
            *tx_ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED)
        }))
    }
}

impl Module {
    fn _query_estimate_gas_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext<'_>,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_estimate_gas(ctx, args)?))
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = ();
    type Parameters = ();
}

impl module::AuthHandler for Module {}

impl module::MigrationHandler for Module {
    type Genesis = ();

    fn init_or_migrate(
        _ctx: &mut DispatchContext<'_>,
        _meta: &mut types::Metadata,
        _genesis: &Self::Genesis,
    ) -> bool {
        false
    }
}

impl module::MethodRegistrationHandler for Module {
    fn register_methods(methods: &mut module::MethodRegistry) {
        // Queries.
        methods.register_query(module::QueryMethodInfo {
            name: "core.EstimateGas",
            handler: Self::_query_estimate_gas_handler,
        });
    }
}

impl module::BlockHandler for Module {}
