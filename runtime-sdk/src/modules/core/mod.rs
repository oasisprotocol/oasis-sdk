//! Core definitions module.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    error,
    module::{self, Module as _, QueryMethodInfo},
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

    #[error("batch gas overflow")]
    #[sdk_error(code = 9)]
    BatchGasOverflow,

    #[error("batch out of gas")]
    #[sdk_error(code = 10)]
    BatchOutOfGas,
}

/// Parameters for the core module.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "batch_gas")]
    pub batch_gas: u64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { batch_gas: 0 }
    }
}

impl module::Parameters for Parameters {
    type Error = std::convert::Infallible;
}

pub trait API {
    fn use_gas(ctx: &mut TxContext<'_, '_>, gas: u64) -> Result<(), Error>;
    fn batch_use_gas(ctx: &mut DispatchContext<'_>, gas: u64) -> Result<(), Error>;
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,
}

impl Default for Genesis {
    fn default() -> Self {
        Self {
            parameters: Default::default(),
        }
    }
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
}

pub struct Module;

const CONTEXT_KEY_GAS_USED: &str = "core.GasUsed";

impl Module {
    /// Initialize state from genesis.
    fn init(ctx: &mut DispatchContext<'_>, genesis: &Genesis) {
        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), &genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate(_ctx: &mut DispatchContext<'_>, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

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

        let batch_gas_limit = Self::params(ctx.runtime_state()).batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED);
        let batch_new_gas_used = match batch_gas_used.overflowing_add(gas) {
            (batch_new_gas_used, false) => batch_new_gas_used,
            (_, true) => return Err(Error::BatchGasOverflow),
        };
        if batch_new_gas_used > batch_gas_limit {
            return Err(Error::BatchOutOfGas);
        }

        let gas_used = ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED);
        *gas_used = new_gas_used;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED);
        *batch_gas_used = batch_new_gas_used;

        Ok(())
    }

    fn batch_use_gas(ctx: &mut DispatchContext<'_>, gas: u64) -> Result<(), Error> {
        let batch_gas_limit = Self::params(ctx.runtime_state()).batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED);
        let batch_new_gas_used = match batch_gas_used.overflowing_add(gas) {
            (batch_new_gas_used, false) => batch_new_gas_used,
            (_, true) => return Err(Error::BatchGasOverflow),
        };
        if batch_new_gas_used > batch_gas_limit {
            return Err(Error::BatchOutOfGas);
        }
        *batch_gas_used = batch_new_gas_used;

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
        ctx.with_simulation(|sim_ctx| {
            sim_ctx.with_tx(args, |mut tx_ctx, call| {
                (mi.handler)(&mi, &mut tx_ctx, call.body);
                // Warning: we don't report success or failure. If the call fails, we still report
                // how much gas it uses while it fails.
                Ok(*tx_ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED))
            })
        })
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
    type Parameters = Parameters;
}

impl module::AuthHandler for Module {}

impl module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate(
        ctx: &mut DispatchContext<'_>,
        meta: &mut types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            Self::init(ctx, genesis);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Perform migration.
        Self::migrate(ctx, version)
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
