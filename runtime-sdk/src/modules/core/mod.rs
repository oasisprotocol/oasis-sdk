//! Core definitions module.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    context::Context,
    core::common::cbor,
    dispatcher, error,
    module::{self, Module as _},
    types::transaction::{self, AddressSpec, AuthProof, Call, UnverifiedTransaction},
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
pub const MODULE_NAME: &str = "core";

/// Errors emitted by the core module.
#[derive(Error, Debug, PartialEq, oasis_runtime_sdk_macros::Error)]
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

    #[error("out of message slots")]
    #[sdk_error(code = 6)]
    OutOfMessageSlots,

    #[error("message handler not invoked")]
    #[sdk_error(code = 8)]
    MessageHandlerNotInvoked,

    #[error("missing message handler")]
    #[sdk_error(code = 9)]
    MessageHandlerMissing(u32),

    #[error("invalid argument")]
    #[sdk_error(code = 10)]
    InvalidArgument,

    #[error("gas overflow")]
    #[sdk_error(code = 11)]
    GasOverflow,

    #[error("out of gas")]
    #[sdk_error(code = 12)]
    OutOfGas,

    #[error("batch gas overflow")]
    #[sdk_error(code = 13)]
    BatchGasOverflow,

    #[error("batch out of gas")]
    #[sdk_error(code = 14)]
    BatchOutOfGas,

    #[error("too many authentication slots")]
    #[sdk_error(code = 15)]
    TooManyAuth,

    #[error("multisig too many signers")]
    #[sdk_error(code = 16)]
    MultisigTooManySigners,
}

/// Gas costs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasCosts {
    #[serde(rename = "auth_signature")]
    pub auth_signature: u64,
    #[serde(rename = "auth_multisig_signer")]
    pub auth_multisig_signer: u64,
}

/// Parameters for the core module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "max_batch_gas")]
    pub max_batch_gas: u64,
    #[serde(rename = "max_tx_signers")]
    pub max_tx_signers: u32,
    #[serde(rename = "max_multisig_signers")]
    pub max_multisig_signers: u32,
    #[serde(rename = "gas_costs")]
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = std::convert::Infallible;
}

pub trait API {
    /// Attempt to use gas. Gas limits are per-batch (max_batch_gas in Parameters) and
    /// per-transaction (.ai.fee.gas). If the gas specified would cause either total used to exceed
    /// its limit, fails with Error::OutOfGas or Error::BatchOutOfGas, and neither gas usage is
    /// increased. Per-transaction gas is not assessed when C is not a transaction processing
    /// context.
    fn use_gas<C: Context>(ctx: &mut C, gas: u64) -> Result<(), Error>;
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
    /// Map of message idx to message handlers for messages emitted in previous round.
    pub const MESSAGE_HANDLERS: &[u8] = &[0x02];
}

pub struct Module;

const CONTEXT_KEY_GAS_USED: &str = "core.GasUsed";

impl Module {
    /// Initialize state from genesis.
    fn init<C: Context>(ctx: &mut C, genesis: &Genesis) {
        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), &genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate<C: Context>(_ctx: &mut C, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

impl API for Module {
    fn use_gas<C: Context>(ctx: &mut C, gas: u64) -> Result<(), Error> {
        let new_gas_used = match (
            ctx.tx_auth_info().map(|ai| ai.fee.gas),
            ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).copied(),
        ) {
            (Some(gas_limit), Some(gas_used)) => {
                let sum = gas_used.checked_add(gas).ok_or(Error::GasOverflow)?;
                if sum > gas_limit {
                    return Err(Error::OutOfGas);
                }
                Some(sum)
            }
            (None, None) => None,
            _ => panic!("inconsistent tx availability"),
        };

        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED);
        let batch_new_gas_used = batch_gas_used
            .checked_add(gas)
            .ok_or(Error::BatchGasOverflow)?;
        if batch_new_gas_used > batch_gas_limit {
            println!("batch gas limit: {:?}", batch_gas_limit);
            return Err(Error::BatchOutOfGas);
        }

        match (new_gas_used, ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED)) {
            (Some(sum), Some(gas_used)) => *gas_used = sum,
            (None, None) => {}
            _ => panic!("inconsistent tx availability"),
        }
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED);
        *batch_gas_used = batch_new_gas_used;

        Ok(())
    }
}

impl Module {
    /// Run a transaction in simulation and return how much gas it uses. This looks up the method
    /// in the context's method registry. Transactions that fail still use gas, and this query will
    /// estimate that and return successfully, so do not use this query to see if a transaction will
    /// succeed. Failure due to OutOfGas are included, so it's best to set the query argument
    /// transaction's gas to something high.
    fn query_estimate_gas<C: Context>(
        ctx: &mut C,
        args: transaction::Transaction,
    ) -> Result<u64, Error> {
        ctx.with_simulation(|mut sim_ctx| {
            sim_ctx.with_tx(args, |mut tx_ctx, call| {
                let _ = dispatcher::Dispatcher::<C::Runtime>::dispatch_call(&mut tx_ctx, call);
                // Warning: we don't report success or failure. If the call fails, we still report
                // how much gas it uses while it fails.
                Ok(*tx_ctx.value::<u64>(CONTEXT_KEY_GAS_USED))
            })
        })
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = ();
    type Parameters = Parameters;
}

impl module::AuthHandler for Module {
    fn approve_unverified_tx<C: Context>(
        ctx: &mut C,
        utx: &UnverifiedTransaction,
    ) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        if utx.1.len() > params.max_tx_signers as usize {
            return Err(Error::TooManyAuth);
        }
        for auth_proof in &utx.1 {
            if let AuthProof::Multisig(config) = auth_proof {
                if config.len() > params.max_multisig_signers as usize {
                    return Err(Error::MultisigTooManySigners);
                }
            }
        }
        Ok(())
    }

    fn before_handle_call<C: Context>(ctx: &mut C, _call: &Call) -> Result<(), Error> {
        let mut num_signature: u64 = 0;
        let mut num_multisig_signer: u64 = 0;
        for si in &ctx.tx_auth_info().unwrap().signer_info {
            match &si.address_spec {
                AddressSpec::Signature(_) => {
                    num_signature = num_signature.checked_add(1).ok_or(Error::GasOverflow)?;
                }
                AddressSpec::Multisig(config) => {
                    num_multisig_signer = num_multisig_signer
                        .checked_add(config.signers.len() as u64)
                        .ok_or(Error::GasOverflow)?;
                }
            }
        }
        let params = Self::params(ctx.runtime_state());
        let total = num_signature
            .checked_mul(params.gas_costs.auth_signature)
            .ok_or(Error::GasOverflow)?
            .checked_add(
                num_multisig_signer
                    .checked_mul(params.gas_costs.auth_multisig_signer)
                    .ok_or(Error::GasOverflow)?,
            )
            .ok_or(Error::GasOverflow)?;
        Self::use_gas(ctx, total)?;
        Ok(())
    }
}

impl module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
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

impl module::MethodHandler for Module {
    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, Result<cbor::Value, error::RuntimeError>> {
        match method {
            "core.EstimateGas" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_estimate_gas(ctx, args)?))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl module::BlockHandler for Module {}
