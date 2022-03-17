//! Core definitions module.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
};

use anyhow::anyhow;
use oasis_runtime_sdk_macros::{handler, sdk_derive};
use thiserror::Error;

pub use oasis_core_keymanager_api_common::KeyManagerError;

use crate::{
    callformat,
    context::{BatchContext, Context, TxContext},
    dispatcher,
    module::{self, InvariantHandler as _, Module as _, ModuleInfoHandler as _},
    types::{
        in_msg, token,
        transaction::{
            self, AddressSpec, AuthProof, Call, CallFormat, TransactionWeight,
            UnverifiedTransaction,
        },
    },
    Runtime,
};

use self::types::RuntimeInfoResponse;

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

    #[error("out of gas (limit: {0} wanted: {1})")]
    #[sdk_error(code = 12)]
    OutOfGas(u64, u64),

    #[error("too many authentication slots")]
    #[sdk_error(code = 15)]
    TooManyAuth,

    #[error("multisig too many signers")]
    #[sdk_error(code = 16)]
    MultisigTooManySigners,

    #[error("invariant violation: {0}")]
    #[sdk_error(code = 17)]
    InvariantViolation(String),

    #[error("invalid call format: {0}")]
    #[sdk_error(code = 18)]
    InvalidCallFormat(#[source] anyhow::Error),

    #[error("{0}")]
    #[sdk_error(transparent, abort)]
    Abort(#[source] dispatcher::Error),

    #[error("no module could authenticate the transaction")]
    #[sdk_error(code = 19)]
    NotAuthenticated,

    #[error("gas price too low")]
    #[sdk_error(code = 20)]
    GasPriceTooLow,

    #[error("forbidden in secure build")]
    #[sdk_error(code = 21)]
    ForbiddenInSecureBuild,

    #[error("malformed incoming message: {0}")]
    #[sdk_error(code = 22)]
    MalformedIncomingMessageData(u64, #[source] anyhow::Error),

    #[error("invalid incoming message: {0}")]
    #[sdk_error(code = 23)]
    InvalidIncomingMessage(#[from] in_msg::Error),
}

/// Events emitted by the core module.
#[derive(Debug, PartialEq, Eq, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    GasUsed { amount: u64 },
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_byte: u64,

    pub auth_signature: u64,
    pub auth_multisig_signer: u64,

    pub callformat_x25519_deoxysii: u64,
}

/// Parameters for the core module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub max_batch_gas: u64,
    pub max_in_msgs_gas: u64,
    pub max_tx_signers: u32,
    pub max_multisig_signers: u32,
    pub gas_costs: GasCosts,
    pub min_gas_price: BTreeMap<token::Denomination, u128>,
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

    /// Returns the remaining batch-wide gas.
    fn remaining_batch_gas<C: Context>(ctx: &mut C) -> u64;

    /// Returns the remaining batch-wide gas that can be used for roothash incoming messages.
    fn remaining_in_msgs_gas<C: Context>(ctx: &mut C) -> u64;

    /// Return the remaining tx-wide gas.
    fn remaining_tx_gas<C: TxContext>(ctx: &mut C) -> u64;

    /// Return the used tx-wide gas.
    fn used_tx_gas<C: TxContext>(ctx: &mut C) -> u64;

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

/// Local configuration that can be provided by the node operator.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct LocalConfig {
    /// Minimum gas price to accept.
    #[cbor(optional, default)]
    pub min_gas_price: BTreeMap<token::Denomination, u128>,
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
    /// Map of message idx to message handlers for messages emitted in previous round.
    pub const MESSAGE_HANDLERS: &[u8] = &[0x02];
}

/// Module configuration.
#[allow(clippy::declare_interior_mutable_const)]
pub trait Config: 'static {
    /// Default local minimum gas price configuration that is used in case no overrides are set in
    /// local per-node configuration.
    const DEFAULT_LOCAL_MIN_GAS_PRICE: once_cell::unsync::Lazy<
        BTreeMap<token::Denomination, u128>,
    > = once_cell::unsync::Lazy::new(BTreeMap::new);

    /// Methods which are exempt from minimum gas price requirements.
    const MIN_GAS_PRICE_EXEMPT_METHODS: once_cell::unsync::Lazy<BTreeSet<&'static str>> =
        once_cell::unsync::Lazy::new(BTreeSet::new);
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

const CONTEXT_KEY_GAS_USED: &str = "core.GasUsed";
const CONTEXT_KEY_PRIORITY: &str = "core.Priority";
const CONTEXT_KEY_WEIGHTS: &str = "core.Weights";

const GAS_WEIGHT_NAME: &str = "gas";

impl<Cfg: Config> Module<Cfg> {
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

impl<Cfg: Config> API for Module<Cfg> {
    fn use_batch_gas<C: Context>(ctx: &mut C, gas: u64) -> Result<(), Error> {
        // Do not enforce batch limits for check-tx.
        if ctx.is_check_only() {
            return Ok(());
        }
        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        // NOTE: Going over the batch limit should trigger an abort as the scheduler should never
        //       allow scheduling past the batch limit but a malicious proposer might include too
        //       many transactions. Make sure to vote for failure in this case.
        let batch_new_gas_used = batch_gas_used
            .checked_add(gas)
            .ok_or(Error::Abort(dispatcher::Error::BatchOutOfGas))?;
        if batch_new_gas_used > batch_gas_limit {
            return Err(Error::Abort(dispatcher::Error::BatchOutOfGas));
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
                return Err(Error::OutOfGas(gas_limit, sum));
            }
            sum
        };

        Self::use_batch_gas(ctx, gas)?;

        *ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default() = new_gas_used;

        Ok(())
    }

    fn remaining_batch_gas<C: Context>(ctx: &mut C) -> u64 {
        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        batch_gas_limit.saturating_sub(*batch_gas_used)
    }

    fn remaining_in_msgs_gas<C: Context>(ctx: &mut C) -> u64 {
        let in_msgs_gas_limit = Self::params(ctx.runtime_state()).max_in_msgs_gas;
        let batch_gas_used = ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        in_msgs_gas_limit.saturating_sub(*batch_gas_used)
    }

    fn remaining_tx_gas<C: TxContext>(ctx: &mut C) -> u64 {
        let gas_limit = ctx.tx_auth_info().fee.gas;
        let gas_used = ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
        let remaining_tx = gas_limit.saturating_sub(*gas_used);
        // Also check remaining batch gas limit and return the minimum of the two.
        let remaining_batch = Self::remaining_batch_gas(ctx);
        std::cmp::min(remaining_tx, remaining_batch)
    }

    fn used_tx_gas<C: TxContext>(ctx: &mut C) -> u64 {
        *ctx.tx_value::<u64>(CONTEXT_KEY_GAS_USED).or_default()
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

#[sdk_derive(MethodHandler)]
impl<Cfg: Config> Module<Cfg> {
    /// Run a transaction in simulation and return how much gas it uses. This looks up the method
    /// in the context's method registry. Transactions that fail still use gas, and this query will
    /// estimate that and return successfully, so do not use this query to see if a transaction will
    /// succeed.
    #[handler(query = "core.EstimateGas")]
    fn query_estimate_gas<C: Context>(
        ctx: &mut C,
        mut args: types::EstimateGasQuery,
    ) -> Result<u64, Error> {
        // Assume maximum amount of gas in a batch, a reasonable maximum fee and maximum amount of consensus messages.
        args.tx.auth_info.fee.gas = Self::params(ctx.runtime_state()).max_batch_gas;
        args.tx.auth_info.fee.amount =
            token::BaseUnits::new(u64::MAX.into(), token::Denomination::NATIVE);
        args.tx.auth_info.fee.consensus_messages = ctx.remaining_messages();
        // Estimate transaction size. Since the transaction given to us is not signed, we need to
        // estimate how large each of the auth proofs would be.
        let auth_proofs: Result<_, Error> = args
            .tx
            .auth_info
            .signer_info
            .iter()
            .map(|si| match si.address_spec {
                // For the signature address spec we assume a signature auth proof of 64 bytes.
                transaction::AddressSpec::Signature(_) => {
                    Ok(transaction::AuthProof::Signature(vec![0; 64].into()))
                }
                // For the multisig address spec assume all the signers sign with a 64-byte signature.
                transaction::AddressSpec::Multisig(ref cfg) => {
                    Ok(transaction::AuthProof::Multisig(
                        cfg.signers
                            .iter()
                            .map(|_| Some(vec![0; 64].into()))
                            .collect(),
                    ))
                }
                // Internal address specs should never appear as they are not serializable.
                transaction::AddressSpec::Internal(_) => Err(Error::MalformedTransaction(anyhow!(
                    "internal address spec used"
                ))),
            })
            .collect();
        let tx_envelope =
            transaction::UnverifiedTransaction(cbor::to_vec(args.tx.clone()), auth_proofs?);
        let tx_size: u32 = cbor::to_vec(tx_envelope)
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument(anyhow!("transaction too large")))?;

        // Update the address used within the transaction when caller address is passed.
        let mut extra_gas = 0;
        if let Some(caller) = args.caller.clone() {
            let address_spec = transaction::AddressSpec::Internal(caller);
            match args.tx.auth_info.signer_info.first_mut() {
                Some(si) => si.address_spec = address_spec,
                None => {
                    // If no existing auth info, push some.
                    args.tx.auth_info.signer_info.push(transaction::SignerInfo {
                        address_spec,
                        nonce: 0,
                    });
                }
            }

            // When passing an address we don't know what scheme is used for authenticating the
            // address so the estimate may be off. Assume a regular signature for now.
            let params = Self::params(ctx.runtime_state());
            extra_gas += params.gas_costs.auth_signature;
        }

        ctx.with_simulation(|mut sim_ctx| {
            sim_ctx.with_tx(tx_size, args.tx, |mut tx_ctx, call| {
                dispatcher::Dispatcher::<C::Runtime>::dispatch_tx_call(&mut tx_ctx, call);
                // Warning: we don't report success or failure. If the call fails, we still report
                // how much gas it uses while it fails.
                let gas_used = *tx_ctx.value::<u64>(CONTEXT_KEY_GAS_USED).or_default();
                Ok(gas_used + extra_gas)
            })
        })
    }

    /// Check invariants of all modules in the runtime.
    #[handler(query = "core.CheckInvariants")]
    fn query_check_invariants<C: Context>(ctx: &mut C, _args: ()) -> Result<(), Error> {
        if !ctx.are_expensive_queries_allowed() {
            return Err(Error::InvalidArgument(anyhow!("query not allowed")));
        }

        <C::Runtime as Runtime>::Modules::check_invariants(ctx)
    }

    /// Retrieve the public key for encrypting call data.
    #[handler(query = "core.CallDataPublicKey")]
    fn query_calldata_public_key<C: Context>(
        ctx: &mut C,
        _args: (),
    ) -> Result<types::CallDataPublicKeyQueryResponse, Error> {
        let key_manager = ctx
            .key_manager()
            .ok_or_else(|| Error::InvalidArgument(anyhow!("key manager not available")))?;
        let public_key = key_manager
            .get_public_key(callformat::get_key_pair_id(ctx))
            .map_err(|err| Error::Abort(err.into()))?
            .ok_or_else(|| Error::InvalidArgument(anyhow!("key not available")))?;

        Ok(types::CallDataPublicKeyQueryResponse { public_key })
    }

    /// Query the minimum gas price.
    #[handler(query = "core.MinGasPrice")]
    fn query_min_gas_price<C: Context>(
        ctx: &mut C,
        _args: (),
    ) -> Result<BTreeMap<token::Denomination, u128>, Error> {
        let params = Self::params(ctx.runtime_state());

        // Generate a combined view with local overrides.
        let mut mgp = params.min_gas_price;
        for (denom, price) in mgp.iter_mut() {
            let local_mgp = Self::get_local_min_gas_price(ctx, denom);
            if local_mgp > *price {
                *price = local_mgp;
            }
        }

        Ok(mgp)
    }

    /// Return basic information about the module and the containing runtime.
    #[handler(query = "core.RuntimeInfo")]
    fn query_runtime_info<C: Context>(
        ctx: &mut C,
        _args: (),
    ) -> Result<RuntimeInfoResponse, Error> {
        Ok(RuntimeInfoResponse {
            runtime_version: <C::Runtime as Runtime>::VERSION,
            state_version: <C::Runtime as Runtime>::STATE_VERSION,
            modules: <C::Runtime as Runtime>::Modules::module_info(ctx),
        })
    }
}

impl<Cfg: Config> Module<Cfg> {
    fn get_local_min_gas_price<C: Context>(ctx: &mut C, denom: &token::Denomination) -> u128 {
        #[allow(clippy::borrow_interior_mutable_const)]
        ctx.local_config(MODULE_NAME)
            .as_ref()
            .map(|cfg: &LocalConfig| cfg.min_gas_price.get(denom).copied())
            .unwrap_or_else(|| Cfg::DEFAULT_LOCAL_MIN_GAS_PRICE.get(denom).copied())
            .unwrap_or_default()
    }

    fn enforce_min_gas_price<C: TxContext>(ctx: &mut C, call: &Call) -> Result<(), Error> {
        // If the method is exempt from min gas price requirements, checks always pass.
        #[allow(clippy::borrow_interior_mutable_const)]
        if Cfg::MIN_GAS_PRICE_EXEMPT_METHODS.contains(call.method.as_str()) {
            return Ok(());
        }

        let params = Self::params(ctx.runtime_state());
        let fee = ctx.tx_auth_info().fee.clone();
        let denom = fee.amount.denomination();

        match params.min_gas_price.get(denom) {
            // If the denomination is not among the global set, reject.
            None => return Err(Error::GasPriceTooLow),

            // Otherwise, allow overrides during local checks.
            Some(min_gas_price) => {
                if ctx.is_check_only() {
                    let local_mgp = Self::get_local_min_gas_price(ctx, denom);

                    // Reject during local checks.
                    if fee.gas_price() < local_mgp {
                        return Err(Error::GasPriceTooLow);
                    }
                }

                if &fee.gas_price() < min_gas_price {
                    return Err(Error::GasPriceTooLow);
                }
            }
        }

        Ok(())
    }
}

impl<Cfg: Config> module::Module for Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {
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

    fn before_handle_call<C: TxContext>(ctx: &mut C, call: &Call) -> Result<(), Error> {
        // Ensure that specified gas limit is not greater than batch gas limit.
        let params = Self::params(ctx.runtime_state());
        let gas = ctx.tx_auth_info().fee.gas;
        if gas > params.max_batch_gas {
            return Err(Error::GasOverflow);
        }

        // Enforce minimum gas price constraints.
        Self::enforce_min_gas_price(ctx, call)?;

        // Charge gas for transaction size.
        Self::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_byte
                .checked_mul(ctx.tx_size().into())
                .ok_or(Error::GasOverflow)?,
        )?;

        // Charge gas for signature verification.
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
        let total = (|| {
            let signature_cost = num_signature.checked_mul(params.gas_costs.auth_signature)?;
            let multisig_signer_cost =
                num_multisig_signer.checked_mul(params.gas_costs.auth_multisig_signer)?;
            let sum = signature_cost.checked_add(multisig_signer_cost)?;
            Some(sum)
        })()
        .ok_or(Error::GasOverflow)?;
        Self::use_tx_gas(ctx, total)?;

        // Charge gas for callformat.
        match call.format {
            CallFormat::Plain => {} // No additional gas required.
            CallFormat::EncryptedX25519DeoxysII => {
                Self::use_tx_gas(ctx, params.gas_costs.callformat_x25519_deoxysii)?
            }
        }

        // Set weight based on configured gas limit.
        Self::add_weight(ctx, GAS_WEIGHT_NAME.into(), gas)?;

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

    fn after_handle_call<C: TxContext>(ctx: &mut C) -> Result<(), Error> {
        // Emit gas used event.
        let used_gas = Self::used_tx_gas(ctx);
        ctx.emit_unconditional_event(Event::GasUsed { amount: used_gas });

        Ok(())
    }
}

impl<Cfg: Config> module::IncomingMessageHandler for Module<Cfg> {}

impl<Cfg: Config> module::MigrationHandler for Module<Cfg> {
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

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn get_block_weight_limits<C: Context>(ctx: &mut C) -> BTreeMap<TransactionWeight, u64> {
        let batch_gas_limit = Self::params(ctx.runtime_state()).max_batch_gas;

        let mut res = BTreeMap::new();
        res.insert(GAS_WEIGHT_NAME.into(), batch_gas_limit);

        res
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
