//! Core definitions module.
use std::collections::BTreeMap;

use thiserror::Error;

pub use oasis_core_keymanager_api_common::KeyManagerError;

use crate::{
    context::{BatchContext, Context, TxContext},
    dispatcher, error,
    module::{self, InvariantHandler as _, Module as _},
    types::transaction::{
        self, AddressSpec, AuthProof, Call, TransactionWeight, UnverifiedTransaction,
    },
    Runtime,
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
pub const MODULE_NAME: &str = "core";

/// Errors emitted by the core module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("malformed transaction: {0}")]
    #[sdk_error(code = 1)]
    MalformedTransaction(#[source] anyhow::Error),

    #[error("invalid transaction: {0}")]
    #[sdk_error(code = 2)]
    InvalidTransaction(#[from] transaction::Error),

    #[error("invalid method: {0}")]
    #[sdk_error(code = 3)]
    InvalidMethod(String),

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

    #[error("invalid argument: {0}")]
    #[sdk_error(code = 10)]
    InvalidArgument(#[source] anyhow::Error),

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

    #[error("invariant violation: {0}")]
    #[sdk_error(code = 17)]
    InvariantViolation(String),

    #[error("key manager error")]
    #[sdk_error(code = 18)]
    KeyManagerError(#[source] KeyManagerError),

    #[error("invalid call format: {0}")]
    #[sdk_error(code = 19)]
    InvalidCallFormat(#[source] anyhow::Error),
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub auth_signature: u64,
    pub auth_multisig_signer: u64,
}

/// Parameters for the core module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub max_batch_gas: u64,
    pub max_tx_signers: u32,
    pub max_multisig_signers: u32,
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = std::convert::Infallible;
}

pub trait API {
    /// Attempt to use gas. If the gas specified would cause either total used to exceed
    /// its limit, fails with Error::OutOfGas or Error::BatchOutOfGas, and neither gas usage is
    /// increased.
    fn use_batch_gas<C: Context>(ctx: &mut C, gas: u64) -> Result<(), Error>;

    /// Attempt to use gas. If the gas specified would cause either total used to exceed
    /// its limit, fails with Error::OutOfGas or Error::BatchOutOfGas, and neither gas usage is
    /// increased.
    fn use_tx_gas<C: TxContext>(ctx: &mut C, gas: u64) -> Result<(), Error>;

    /// Return the remaining gas.
    fn remaining_tx_gas<C: TxContext>(ctx: &mut C) -> u64;

    /// Increase transaction priority for the provided amount.
    fn add_priority<C: Context>(ctx: &mut C, priority: u64) -> Result<(), Error>;

    /// Increase the specific transaction weight for the provided amount.
    fn add_weight<C: TxContext>(
        ctx: &mut C,
        weight: TransactionWeight,
        val: u64,
    ) -> Result<(), Error>;

    /// Takes the stored transaction weight.
    fn take_weights<C: Context>(ctx: &mut C) -> BTreeMap<TransactionWeight, u64>;

    /// Takes and returns the stored transaction priority.
    fn take_priority<C: Context>(ctx: &mut C) -> u64;
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
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
const CONTEXT_KEY_PRIORITY: &str = "core.Priority";
const CONTEXT_KEY_WEIGHTS: &str = "core.Weights";

const GAS_WEIGHT_NAME: &str = "gas";

impl Module {
    /// Initialize state from genesis.
    pub fn init<C: Context>(ctx: &mut C, genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate<C: Context>(_ctx: &mut C, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

impl API for Module {
    fn use_batch_gas<C: Context>(ctx: &mut C, gas: u64) -> Result<(), Error> {
        // Do not enforce batch limits for check-tx.
        if ctx.is_check_only() {
            return Ok(());
        }
        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        let batch_new_gas_used = batch_gas_used
            .checked_add(gas)
            .ok_or(Error::BatchGasOverflow)?;
        if batch_new_gas_used > batch_gas_limit {
            return Err(Error::BatchOutOfGas);
        }

        ctx.value::<u64>(CONTEXT_KEY_GAS_USED)
            .set(batch_new_gas_used);

        Ok(())
    }

    fn use_tx_gas<C: TxContext>(ctx: &mut C, gas: u64) -> Result<(), Error> {
        let gas_limit = ctx.tx_auth_info().fee.gas;
        let gas_used = ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        let new_gas_used = {
            let sum = gas_used.checked_add(gas).ok_or(Error::GasOverflow)?;
            if sum > gas_limit {
                return Err(Error::OutOfGas);
            }
            sum
        };

        Self::use_batch_gas(ctx, gas)?;

        *ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default() = new_gas_used;

        Self::add_weight(ctx, GAS_WEIGHT_NAME.into(), gas)?;

        Ok(())
    }

    fn remaining_tx_gas<C: TxContext>(ctx: &mut C) -> u64 {
        let gas_limit = ctx.tx_auth_info().fee.gas;
        let gas_used = ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        gas_limit.saturating_sub(*gas_used)
    }

    fn add_priority<C: Context>(ctx: &mut C, priority: u64) -> Result<(), Error> {
        let p = ctx.value::<u64>(CONTEXT_KEY_PRIORITY).or_default();
        let added_p = p.checked_add(priority).unwrap_or(u64::MAX);

        ctx.value::<u64>(CONTEXT_KEY_PRIORITY).set(added_p);

        Ok(())
    }

    fn add_weight<C: TxContext>(
        ctx: &mut C,
        weight: TransactionWeight,
        val: u64,
    ) -> Result<(), Error> {
        let weights = ctx
            .value::<BTreeMap<TransactionWeight, u64>>(CONTEXT_KEY_WEIGHTS)
            .or_default();

        let w = weights.remove(&weight).unwrap_or_default();
        let added_w = w.checked_add(val).unwrap_or(u64::MAX);
        weights.insert(weight, added_w);

        Ok(())
    }

    fn take_priority<C: Context>(ctx: &mut C) -> u64 {
        ctx.value::<u64>(CONTEXT_KEY_PRIORITY)
            .take()
            .unwrap_or_default()
    }

    fn take_weights<C: Context>(ctx: &mut C) -> BTreeMap<TransactionWeight, u64> {
        ctx.value::<BTreeMap<TransactionWeight, u64>>(CONTEXT_KEY_WEIGHTS)
            .take()
            .unwrap_or_default()
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
                dispatcher::Dispatcher::<C::Runtime>::dispatch_tx_call(&mut tx_ctx, call);
                // Warning: we don't report success or failure. If the call fails, we still report
                // how much gas it uses while it fails.
                Ok(*tx_ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default())
            })
        })
    }

    /// Check invariants of all modules in the runtime.
    fn query_check_invariants<C: Context>(ctx: &mut C) -> Result<(), Error> {
        <C::Runtime as Runtime>::Modules::check_invariants(ctx)
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

    fn before_handle_call<C: TxContext>(ctx: &mut C, _call: &Call) -> Result<(), Error> {
        let mut num_signature: u64 = 0;
        let mut num_multisig_signer: u64 = 0;
        for si in &ctx.tx_auth_info().signer_info {
            match &si.address_spec {
                AddressSpec::Signature(_) => {
                    num_signature = num_signature.checked_add(1).ok_or(Error::GasOverflow)?;
                }
                AddressSpec::Multisig(config) => {
                    num_multisig_signer = num_multisig_signer
                        .checked_add(config.signers.len() as u64)
                        .ok_or(Error::GasOverflow)?;
                }
                AddressSpec::Internal(_) => {}
            }
        }
        let params = Self::params(ctx.runtime_state());
        let total = (|| {
            let signature_cost = num_signature.checked_mul(params.gas_costs.auth_signature)?;
            let multisig_signer_cost =
                num_multisig_signer.checked_mul(params.gas_costs.auth_multisig_signer)?;
            let sum = signature_cost.checked_add(multisig_signer_cost)?;
            Some(sum)
        })()
        .ok_or(Error::GasOverflow)?;
        Self::use_tx_gas(ctx, total)?;

        // Attempt to limit the maximum number of consensus messages and add appropriate weights.
        let consensus_messages = ctx.tx_auth_info().fee.consensus_messages;
        ctx.limit_max_messages(consensus_messages)?;
        Self::add_weight(
            ctx,
            TransactionWeight::ConsensusMessages,
            consensus_messages as u64,
        )?;

        Ok(())
    }
}

impl module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut types::Metadata,
        genesis: Self::Genesis,
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
                let args = cbor::from_value(args).map_err(|e| Error::InvalidArgument(e.into()))?;
                Ok(cbor::to_value(Self::query_estimate_gas(ctx, args)?))
            })()),
            "core.CheckInvariants" => module::DispatchResult::Handled((|| {
                let _ = Self::query_check_invariants(ctx)?;
                Ok(cbor::to_value(true))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl module::BlockHandler for Module {
    fn get_block_weight_limits<C: Context>(ctx: &mut C) -> BTreeMap<TransactionWeight, u64> {
        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;

        let mut res = BTreeMap::new();
        res.insert(GAS_WEIGHT_NAME.into(), batch_gas_limit);

        res
    }
}

impl module::InvariantHandler for Module {}
