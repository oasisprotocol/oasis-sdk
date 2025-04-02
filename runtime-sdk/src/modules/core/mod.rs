//! Core definitions module.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::{TryFrom, TryInto},
    fmt::Display,
};

use anyhow::anyhow;
use oasis_runtime_sdk_macros::{handler, sdk_derive};
use thiserror::Error;

use crate::{
    callformat,
    context::Context,
    core::consensus::beacon::EpochTime,
    dispatcher,
    error::Error as SDKError,
    keymanager, migration,
    module::{
        self, CallResult, InvariantHandler as _, MethodHandler as _, Module as _,
        ModuleInfoHandler as _,
    },
    sender::SenderMeta,
    state::{CurrentState, Mode, Options, TransactionWithMeta},
    storage::{self},
    types::{
        token::{self, Denomination},
        transaction::{
            self, AddressSpec, AuthProof, Call, CallFormat, CallerAddress, SignerInfo, Transaction,
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

    #[error("forbidden by node policy")]
    #[sdk_error(code = 22)]
    Forbidden,

    #[error("transaction is too large")]
    #[sdk_error(code = 23)]
    OversizedTransaction,

    #[error("transaction is expired or not yet valid")]
    #[sdk_error(code = 24)]
    ExpiredTransaction,

    #[error("read-only transaction attempted modifications")]
    #[sdk_error(code = 25)]
    ReadOnlyTransaction,

    #[error("future nonce")]
    #[sdk_error(code = 26)]
    FutureNonce,

    #[error("call depth exceeded (depth: {0} max: {1})")]
    #[sdk_error(code = 27)]
    CallDepthExceeded(u16, u16),

    #[error("{0}")]
    #[sdk_error(transparent)]
    TxSimulationFailed(#[from] TxSimulationFailure),
}

impl Error {
    /// Generate a proper OutOfGas error, depending on whether the module is configured to emit gas
    /// use information or not.
    pub fn out_of_gas<Cfg: Config>(limit: u64, wanted: u64) -> Self {
        if Cfg::EMIT_GAS_USED_EVENTS {
            Self::OutOfGas(limit, wanted)
        } else {
            // Mask gas used information.
            Self::OutOfGas(0, 0)
        }
    }
}

/// Simulation failure error.
#[derive(Error, Debug)]
pub struct TxSimulationFailure {
    message: String,
    module_name: String,
    code: u32,
}

impl TxSimulationFailure {
    /// Returns true if the failure is "core::Error::OutOfGas".
    pub fn is_error_core_out_of_gas(&self) -> bool {
        self.module_name == MODULE_NAME && self.code == 12
    }
}

impl Display for TxSimulationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl SDKError for TxSimulationFailure {
    fn module_name(&self) -> &str {
        &self.module_name
    }

    fn code(&self) -> u32 {
        self.code
    }
}

impl TryFrom<CallResult> for TxSimulationFailure {
    type Error = anyhow::Error;

    fn try_from(value: CallResult) -> Result<Self, Self::Error> {
        match value {
            CallResult::Failed {
                module,
                code,
                message,
            } => Ok(TxSimulationFailure {
                code,
                module_name: module,
                message,
            }),
            _ => Err(anyhow!("CallResult not Failed")),
        }
    }
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
    pub storage_byte: u64,

    pub auth_signature: u64,
    pub auth_multisig_signer: u64,

    pub callformat_x25519_deoxysii: u64,
}

/// Dynamic min gas price parameters.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DynamicMinGasPrice {
    /// Enables the dynamic min gas price feature which dynamically adjusts the minimum gas price
    /// based on block fullness, inspired by EIP-1559.
    ///
    /// Only takes effect if `min_gas_price`(s) are set.
    pub enabled: bool,

    /// Target block gas usage indicates the desired block gas usage as a percentage of the total
    /// block gas limit.
    ///
    /// The min gas price will adjust up or down depending on whether the actual gas usage is above
    /// or below this target.
    pub target_block_gas_usage_percentage: u8,
    /// Represents a constant value used to limit the rate at which the min price can change
    /// between blocks.
    ///
    /// For example, if `min_price_max_change_denominator` is set to 8, the maximum change in
    /// min price is 12.5% between blocks.
    pub min_price_max_change_denominator: u8,
}

/// Errors emitted during core parameter validation.
#[derive(Error, Debug)]
pub enum ParameterValidationError {
    #[error("invalid dynamic target block gas usage percentage (10-100)")]
    InvalidTargetBlockGasUsagePercentage,
    #[error("invalid dynamic min price max change denominator (1-50)")]
    InvalidMinPriceMaxChangeDenominator,
}
/// Parameters for the core module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub max_batch_gas: u64,
    pub max_tx_size: u32,
    pub max_tx_signers: u32,
    pub max_multisig_signers: u32,
    pub gas_costs: GasCosts,
    pub min_gas_price: BTreeMap<token::Denomination, u128>,
    pub dynamic_min_gas_price: DynamicMinGasPrice,
}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    fn validate_basic(&self) -> Result<(), Self::Error> {
        // Validate dynamic min gas price parameters.
        let dmgp = &self.dynamic_min_gas_price;
        if dmgp.enabled {
            if dmgp.target_block_gas_usage_percentage < 10
                || dmgp.target_block_gas_usage_percentage > 100
            {
                return Err(ParameterValidationError::InvalidTargetBlockGasUsagePercentage);
            }
            if dmgp.min_price_max_change_denominator < 1
                || dmgp.min_price_max_change_denominator > 50
            {
                return Err(ParameterValidationError::InvalidMinPriceMaxChangeDenominator);
            }
        }
        Ok(())
    }
}

/// Interface that can be called from other modules.
pub trait API {
    /// Module configuration.
    type Config: Config;

    /// Attempt to use gas. If the gas specified would cause either total used to exceed
    /// its limit, fails with Error::OutOfGas or Error::BatchOutOfGas, and neither gas usage is
    /// increased.
    fn use_batch_gas(gas: u64) -> Result<(), Error>;

    /// Attempt to use gas. If the gas specified would cause either total used to exceed
    /// its limit, fails with Error::OutOfGas or Error::BatchOutOfGas, and neither gas usage is
    /// increased.
    fn use_tx_gas(gas: u64) -> Result<(), Error>;

    /// Returns the remaining batch-wide gas.
    fn remaining_batch_gas() -> u64;

    /// Returns the total batch-wide gas used.
    fn used_batch_gas() -> u64;

    /// Return the remaining tx-wide gas.
    fn remaining_tx_gas() -> u64;

    /// Return the used tx-wide gas.
    fn used_tx_gas() -> u64;

    /// Configured maximum amount of gas that can be used in a batch.
    fn max_batch_gas() -> u64;

    /// Configured minimum gas price.
    fn min_gas_price(denom: &token::Denomination) -> Option<u128>;

    /// Sets the transaction priority to the provided amount.
    fn set_priority(priority: u64);

    /// Takes and returns the stored transaction priority.
    fn take_priority() -> u64;

    /// Set transaction sender metadata.
    fn set_sender_meta(meta: SenderMeta);

    /// Takes and returns the stored transaction sender metadata.
    fn take_sender_meta() -> SenderMeta;

    /// Returns the configured max iterations in the binary search for the estimate
    /// gas.
    fn estimate_gas_search_max_iters<C: Context>(ctx: &C) -> u64;

    /// Check whether the epoch has changed since last processed block.
    fn has_epoch_changed() -> bool;
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// Local configuration that can be provided by the node operator.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct LocalConfig {
    /// Minimum gas price to accept.
    #[cbor(optional)]
    pub min_gas_price: BTreeMap<token::Denomination, u128>,

    /// When estimating gas in `core.EstimateGas`, simulate the tx (and report) only up to this much
    /// used gas. This limit is more likely to be relevant if `estimate_gas_by_simulating_contracts` is
    /// enabled in the local config. The special value of 0 means that the maximum amount of gas in a
    /// batch will be used.
    #[cbor(optional)]
    pub max_estimated_gas: u64,

    /// The maximum number of iterations of the binary search to be done when simulating contracts for
    /// gas estimation in `core.EstimateGas`.
    /// The special value of 0 means that binary search won't be performed, and the transaction will be
    /// simulated using maximum possible gas, which might return an overestimation in some special cases.
    /// This setting should likely be kept at 0, unless the runtime is using the EVM module.
    #[cbor(optional)]
    pub estimate_gas_search_max_iters: u64,
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
    /// Map of message idx to message handlers for messages emitted in previous round.
    pub const MESSAGE_HANDLERS: &[u8] = &[0x02];
    /// Last processed epoch for detecting epoch changes.
    pub const LAST_EPOCH: &[u8] = &[0x03];
    /// Dynamic min gas price.
    pub const DYNAMIC_MIN_GAS_PRICE: &[u8] = &[0x04];
}

/// Module configuration.
#[allow(clippy::declare_interior_mutable_const)]
pub trait Config: 'static {
    /// Default local minimum gas price configuration that is used in case no overrides are set in
    /// local per-node configuration.
    const DEFAULT_LOCAL_MIN_GAS_PRICE: once_cell::unsync::Lazy<
        BTreeMap<token::Denomination, u128>,
    > = once_cell::unsync::Lazy::new(BTreeMap::new);

    /// Default local estimate gas max search iterations configuration that is used in case no overrides
    /// are set in the local per-node configuration.
    const DEFAULT_LOCAL_ESTIMATE_GAS_SEARCH_MAX_ITERS: u64 = 0;

    /// Estimated gas amount to be added to failed transaction simulations for selected methods.
    const ESTIMATE_GAS_EXTRA_FAIL: once_cell::unsync::Lazy<BTreeMap<&'static str, u64>> =
        once_cell::unsync::Lazy::new(BTreeMap::new);

    /// Methods which are exempt from minimum gas price requirements.
    const MIN_GAS_PRICE_EXEMPT_METHODS: once_cell::unsync::Lazy<BTreeSet<&'static str>> =
        once_cell::unsync::Lazy::new(BTreeSet::new);

    /// Whether gas used events should be emitted for every transaction.
    ///
    /// Confidential runtimes may want to disable this as it is a possible side channel.
    const EMIT_GAS_USED_EVENTS: bool = true;

    /// Whether to allow submission of read-only transactions in an interactive way.
    ///
    /// Note that execution of such transactions is allowed to access confidential state.
    const ALLOW_INTERACTIVE_READ_ONLY_TRANSACTIONS: bool = false;

    /// The gas cost of the internal call to retrieve the current calldata public key.
    const GAS_COST_CALL_CALLDATA_PUBLIC_KEY: u64 = 20;
    /// The gas cost of the internal call to retrieve the current epoch.
    const GAS_COST_CALL_CURRENT_EPOCH: u64 = 10;
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

const CONTEXT_KEY_GAS_USED: &str = "core.GasUsed";
const CONTEXT_KEY_PRIORITY: &str = "core.Priority";
const CONTEXT_KEY_SENDER_META: &str = "core.SenderMeta";
const CONTEXT_KEY_EPOCH_CHANGED: &str = "core.EpochChanged";

impl<Cfg: Config> API for Module<Cfg> {
    type Config = Cfg;

    fn use_batch_gas(gas: u64) -> Result<(), Error> {
        // Do not enforce batch limits for checks.
        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }
        let batch_gas_limit = Self::params().max_batch_gas;
        let batch_gas_used = Self::used_batch_gas();
        // NOTE: Going over the batch limit should trigger an abort as the scheduler should never
        //       allow scheduling past the batch limit but a malicious proposer might include too
        //       many transactions. Make sure to vote for failure in this case.
        let batch_new_gas_used = batch_gas_used
            .checked_add(gas)
            .ok_or(Error::Abort(dispatcher::Error::BatchOutOfGas))?;
        if batch_new_gas_used > batch_gas_limit {
            return Err(Error::Abort(dispatcher::Error::BatchOutOfGas));
        }

        CurrentState::with(|state| {
            state
                .block_value::<u64>(CONTEXT_KEY_GAS_USED)
                .set(batch_new_gas_used);
        });

        Ok(())
    }

    fn use_tx_gas(gas: u64) -> Result<(), Error> {
        let (gas_limit, gas_used) = CurrentState::with(|state| {
            (
                state.env().tx_auth_info().fee.gas,
                *state.local_value::<u64>(CONTEXT_KEY_GAS_USED).or_default(),
            )
        });
        let new_gas_used = {
            let sum = gas_used.checked_add(gas).ok_or(Error::GasOverflow)?;
            if sum > gas_limit {
                return Err(Error::out_of_gas::<Cfg>(gas_limit, sum));
            }
            sum
        };

        Self::use_batch_gas(gas)?;

        CurrentState::with(|state| {
            *state.local_value::<u64>(CONTEXT_KEY_GAS_USED).or_default() = new_gas_used;
        });

        Ok(())
    }

    fn remaining_batch_gas() -> u64 {
        let batch_gas_limit = Self::params().max_batch_gas;
        batch_gas_limit.saturating_sub(Self::used_batch_gas())
    }

    fn used_batch_gas() -> u64 {
        CurrentState::with(|state| {
            state
                .block_value::<u64>(CONTEXT_KEY_GAS_USED)
                .get()
                .cloned()
                .unwrap_or_default()
        })
    }

    fn remaining_tx_gas() -> u64 {
        let (gas_limit, gas_used) = CurrentState::with(|state| {
            (
                state.env().tx_auth_info().fee.gas,
                *state.local_value::<u64>(CONTEXT_KEY_GAS_USED).or_default(),
            )
        });
        let remaining_tx = gas_limit.saturating_sub(gas_used);
        // Also check remaining batch gas limit and return the minimum of the two.
        let remaining_batch = Self::remaining_batch_gas();
        std::cmp::min(remaining_tx, remaining_batch)
    }

    fn used_tx_gas() -> u64 {
        CurrentState::with(|state| *state.local_value::<u64>(CONTEXT_KEY_GAS_USED).or_default())
    }

    fn max_batch_gas() -> u64 {
        Self::params().max_batch_gas
    }

    fn min_gas_price(denom: &token::Denomination) -> Option<u128> {
        Self::min_gas_prices().get(denom).copied()
    }

    fn set_priority(priority: u64) {
        CurrentState::with(|state| {
            state.block_value::<u64>(CONTEXT_KEY_PRIORITY).set(priority);
        })
    }

    fn take_priority() -> u64 {
        CurrentState::with(|state| {
            state
                .block_value::<u64>(CONTEXT_KEY_PRIORITY)
                .take()
                .unwrap_or_default()
        })
    }

    fn set_sender_meta(meta: SenderMeta) {
        CurrentState::with(|state| {
            state
                .block_value::<SenderMeta>(CONTEXT_KEY_SENDER_META)
                .set(meta);
        });
    }

    fn take_sender_meta() -> SenderMeta {
        CurrentState::with(|state| {
            state
                .block_value::<SenderMeta>(CONTEXT_KEY_SENDER_META)
                .take()
                .unwrap_or_default()
        })
    }

    fn estimate_gas_search_max_iters<C: Context>(ctx: &C) -> u64 {
        ctx.local_config(MODULE_NAME)
            .as_ref()
            .map(|cfg: &LocalConfig| cfg.estimate_gas_search_max_iters)
            .unwrap_or(Cfg::DEFAULT_LOCAL_ESTIMATE_GAS_SEARCH_MAX_ITERS)
    }

    fn has_epoch_changed() -> bool {
        CurrentState::with(|state| {
            *state
                .block_value(CONTEXT_KEY_EPOCH_CHANGED)
                .get()
                .unwrap_or(&false)
        })
    }
}

#[sdk_derive(Module)]
impl<Cfg: Config> Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    pub fn init(genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(genesis.parameters);
    }

    /// Run a transaction in simulation and return how much gas it uses. This looks up the method
    /// in the context's method registry. Transactions that fail still use gas, and this query will
    /// estimate that and return successfully, so do not use this query to see if a transaction will
    /// succeed.
    #[handler(query = "core.EstimateGas", allow_private_km)]
    pub fn query_estimate_gas<C: Context>(
        ctx: &C,
        mut args: types::EstimateGasQuery,
    ) -> Result<u64, Error> {
        let mut extra_gas = 0u64;
        // In case the runtime is confidential we are unable to authenticate the caller so we must
        // make sure to zeroize it to avoid leaking private information.
        if ctx.is_confidential() {
            args.caller = Some(
                args.caller
                    .unwrap_or_else(|| {
                        args.tx
                            .auth_info
                            .signer_info
                            .first()
                            .map(|si| si.address_spec.caller_address())
                            .unwrap_or(CallerAddress::Address(Default::default()))
                    })
                    .zeroized(),
            );
            args.propagate_failures = false; // Likely to fail as caller is zeroized.
        }
        // Assume maximum amount of gas in a batch, a reasonable maximum fee and maximum amount of consensus messages.
        args.tx.auth_info.fee.gas = {
            let local_max_estimated_gas = Self::get_local_max_estimated_gas(ctx);
            if local_max_estimated_gas == 0 {
                Self::params().max_batch_gas
            } else {
                local_max_estimated_gas
            }
        };
        args.tx.auth_info.fee.amount =
            token::BaseUnits::new(u64::MAX.into(), token::Denomination::NATIVE);
        args.tx.auth_info.fee.consensus_messages = ctx.max_messages();
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
        let propagate_failures = args.propagate_failures;
        let bs_max_iters = Self::estimate_gas_search_max_iters(ctx);

        // Update the address used within the transaction when caller address is passed.
        if let Some(caller) = args.caller.clone() {
            // Include additional gas for each signature verification since we will be overwriting
            // the signer infos below.
            extra_gas = extra_gas.saturating_add(
                Self::compute_signature_verification_cost(
                    &Self::params(),
                    &args.tx.auth_info.signer_info,
                )
                .unwrap_or_default(),
            );

            args.tx.auth_info.signer_info = vec![transaction::SignerInfo {
                address_spec: transaction::AddressSpec::Internal(caller),
                nonce: args
                    .tx
                    .auth_info
                    .signer_info
                    .first()
                    .map(|si| si.nonce)
                    .unwrap_or_default(),
            }];
        }

        // Determine if we need to add any extra gas for failing calls.
        #[allow(clippy::borrow_interior_mutable_const)]
        let extra_gas_fail = *Cfg::ESTIMATE_GAS_EXTRA_FAIL
            .get(args.tx.call.method.as_str())
            .unwrap_or(&0);

        // Simulates transaction with a specific gas limit.
        let simulate = |tx: &transaction::Transaction, gas: u64, report_failure: bool| {
            let mut tx = tx.clone();
            tx.auth_info.fee.gas = gas;
            let call = tx.call.clone(); // TODO: Avoid clone.

            CurrentState::with_transaction_opts(
                Options::new()
                    .with_mode(Mode::Simulate)
                    .with_tx(TransactionWithMeta {
                        data: tx,
                        size: tx_size,
                        index: 0,
                        hash: Default::default(),
                    }),
                || {
                    let (result, _) = dispatcher::Dispatcher::<C::Runtime>::dispatch_tx_call(
                        ctx,
                        call,
                        &Default::default(),
                    );
                    if !result.is_success() && report_failure {
                        // Report failure.
                        let err: TxSimulationFailure = result.try_into().unwrap(); // Guaranteed to be a Failed CallResult.
                        return Err(Error::TxSimulationFailed(err));
                    }
                    // Don't report success or failure. If the call fails, we still report
                    // how much gas it uses while it fails.
                    let gas_used = Self::used_batch_gas();
                    if result.is_success() {
                        Ok(gas_used)
                    } else {
                        Ok(gas_used.saturating_add(extra_gas_fail).clamp(0, gas))
                    }
                },
            )
        };

        // Do a binary search for exact gas limit.
        let (cap, mut lo, mut hi) = (
            args.tx.auth_info.fee.gas,
            10_u128,
            args.tx.auth_info.fee.gas as u128,
        ); // Use u128 to avoid overflows when computing the mid point.

        // Count iterations, and remember if fast path was tried.
        let (mut iters, mut fast_path_tried) = (0, false);
        // The following two variables are used to control the special case where a transaction fails
        // and we check if the error is due to out-of-gas by re-simulating the transaction with maximum
        // gas limit. This is needed due to EVM transactions failing with a "reverted" error when
        // not having enough gas for EIP-150 (and not with "out-of-gas").
        let (mut has_succeeded, mut tried_with_max_gas) = (false, false);
        while (lo + 1 < hi) && iters < bs_max_iters {
            iters += 1;

            let mid = (hi + lo) / 2;
            match simulate(&args.tx, mid as u64, true) {
                Ok(r) => {
                    // Estimate success. Try with lower gas.
                    hi = mid;

                    // The transaction succeeded at least once, meaning any future failure is due
                    // to insufficient gas limit.
                    has_succeeded = true;

                    // Optimization: In vast majority of cases the initially returned gas estimate
                    // might already be a good one. Check if this is the case to speed up the convergence.
                    if !fast_path_tried && (lo + 1 < hi) {
                        fast_path_tried = true;

                        // If simulate with the returned estimate succeeds, we can further shrink the
                        // high limit of the binary search.
                        match simulate(&args.tx, r, true) {
                            Ok(_) => hi = r as u128,
                            _ => continue,
                        }
                        // If simulate with one unit of gas smaller fails, we know the exact estimate.
                        match simulate(&args.tx, r - 1, true) {
                            Err(_) => {
                                // Stop the gas search.
                                break;
                            }
                            _ => continue,
                        }
                    }
                }
                Err(_) if has_succeeded => {
                    // Transaction previously succeeded. Transaction failed due to insufficient gas limit,
                    // regardless of the actual returned error.
                    // Try with higher gas.
                    lo = mid
                }
                Err(Error::TxSimulationFailed(failure)) if failure.is_error_core_out_of_gas() => {
                    // Estimate failed due to insufficient gas limit. Try with higher gas.
                    lo = mid
                }
                r @ Err(_) => {
                    let mut res = r;
                    if !tried_with_max_gas {
                        tried_with_max_gas = true;
                        // Transaction failed and simulation with max gas was not yet tried.
                        // Try simulating with maximum gas once:
                        //  - if fails, the transaction will always fail, stop the binary search.
                        //  - if succeeds, remember that transaction is failing due to insufficient gas
                        //    and continue the search.
                        res = simulate(&args.tx, cap, true)
                    }
                    match res {
                        Ok(_) => {
                            has_succeeded = true;
                            // Transaction can succeed. Try with higher gas.
                            lo = mid
                        }
                        err if propagate_failures => {
                            // Estimate failed (not with out-of-gas) and caller wants error propagation -> early exit and return the error.
                            return err;
                        }
                        _ => {
                            // Estimate failed (not with out-of-gas) but caller wants to know the gas usage.
                            // Exit loop and do one final estimate without error propagation.
                            // NOTE: don't continue the binary search for failing transactions as the convergence
                            // for these could take somewhat long and the estimate with default max gas is likely good.
                            break;
                        }
                    }
                }
            }
        }

        // hi == cap if binary search is disabled or this is a failing transaction.
        let result = if hi == Into::<u128>::into(cap) {
            // Simulate one last time with maximum gas limit.
            simulate(&args.tx, cap, propagate_failures)
        } else {
            Ok(hi as u64)
        };

        // Make sure the final result is clamped.
        result.map(|est| est.saturating_add(extra_gas).clamp(0, cap))
    }

    /// Check invariants of all modules in the runtime.
    #[handler(query = "core.CheckInvariants", expensive)]
    fn query_check_invariants<C: Context>(ctx: &C, _args: ()) -> Result<(), Error> {
        <C::Runtime as Runtime>::Modules::check_invariants(ctx)
    }

    fn calldata_public_key_common<C: Context>(
        ctx: &C,
    ) -> Result<types::CallDataPublicKeyQueryResponse, Error> {
        let key_manager = ctx
            .key_manager()
            .ok_or_else(|| Error::InvalidArgument(anyhow!("key manager not available")))?;
        let epoch = ctx.epoch();
        let public_key = key_manager
            .get_public_ephemeral_key(callformat::get_key_pair_id(epoch), epoch)
            .map_err(|err| match err {
                keymanager::KeyManagerError::InvalidEpoch(..) => {
                    Error::InvalidCallFormat(anyhow!("invalid epoch"))
                }
                _ => Error::Abort(err.into()),
            })?;

        Ok(types::CallDataPublicKeyQueryResponse { public_key, epoch })
    }

    /// Retrieve the public key for encrypting call data.
    #[handler(query = "core.CallDataPublicKey")]
    fn query_calldata_public_key<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<types::CallDataPublicKeyQueryResponse, Error> {
        Self::calldata_public_key_common(ctx)
    }

    /// Retrieve the public key for encrypting call data (internally exposed call).
    #[handler(call = "core.CallDataPublicKey", internal)]
    fn internal_calldata_public_key<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<types::CallDataPublicKeyQueryResponse, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_CALLDATA_PUBLIC_KEY)?;
        Self::calldata_public_key_common(ctx)
    }

    /// Retrieve the current epoch.
    #[handler(call = "core.CurrentEpoch", internal)]
    fn internal_current_epoch<C: Context>(ctx: &C, _args: ()) -> Result<u64, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_CURRENT_EPOCH)?;
        Ok(ctx.epoch())
    }

    /// Query the minimum gas price.
    #[handler(query = "core.MinGasPrice")]
    fn query_min_gas_price<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<BTreeMap<token::Denomination, u128>, Error> {
        let mut mgp = Self::min_gas_prices();

        // Generate a combined view with local overrides.
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
    fn query_runtime_info<C: Context>(ctx: &C, _args: ()) -> Result<RuntimeInfoResponse, Error> {
        Ok(RuntimeInfoResponse {
            runtime_version: <C::Runtime as Runtime>::VERSION,
            state_version: <C::Runtime as Runtime>::STATE_VERSION,
            modules: <C::Runtime as Runtime>::Modules::module_info(ctx),
        })
    }

    /// Execute a read-only transaction in an interactive mode.
    ///
    /// # Warning
    ///
    /// This query is allowed access to private key manager state.
    #[handler(query = "core.ExecuteReadOnlyTx", expensive, allow_private_km)]
    fn query_execute_read_only_tx<C: Context>(
        ctx: &C,
        args: types::ExecuteReadOnlyTxQuery,
    ) -> Result<types::ExecuteReadOnlyTxResponse, Error> {
        if !Cfg::ALLOW_INTERACTIVE_READ_ONLY_TRANSACTIONS {
            return Err(Error::Forbidden);
        }

        CurrentState::with_transaction_opts(Options::new().with_mode(Mode::Simulate), || {
            // TODO: Use separate batch gas limit for query execution.

            // Decode transaction and verify signature.
            let tx_size = args
                .tx
                .len()
                .try_into()
                .map_err(|_| Error::OversizedTransaction)?;
            let tx = dispatcher::Dispatcher::<C::Runtime>::decode_tx(ctx, &args.tx)?;

            // Only read-only transactions are allowed in interactive queries.
            if !tx.call.read_only {
                return Err(Error::InvalidArgument(anyhow::anyhow!(
                    "only read-only transactions are allowed"
                )));
            }
            // Only transactions with expiry are allowed in interactive queries.
            if tx.auth_info.not_before.is_none() || tx.auth_info.not_after.is_none() {
                return Err(Error::InvalidArgument(anyhow::anyhow!(
                    "only read-only transactions with expiry are allowed"
                )));
            }

            // Execute transaction.
            let (result, _) = dispatcher::Dispatcher::<C::Runtime>::execute_tx_opts(
                ctx,
                tx,
                &dispatcher::DispatchOptions {
                    tx_size,
                    method_authorizer: Some(&|method| {
                        // Ensure that the inner method is allowed to be called from an interactive
                        // context to avoid unexpected pitfalls.
                        <C::Runtime as Runtime>::Modules::is_allowed_interactive_call(method)
                            && <C::Runtime as Runtime>::is_allowed_interactive_call(method)
                    }),
                    ..Default::default()
                },
            )
            .map_err(|err| Error::InvalidArgument(err.into()))?;

            Ok(types::ExecuteReadOnlyTxResponse { result })
        })
    }
}

impl<Cfg: Config> Module<Cfg> {
    fn min_gas_prices() -> BTreeMap<Denomination, u128> {
        let params = Self::params();
        if params.dynamic_min_gas_price.enabled {
            CurrentState::with_store(|store| {
                let store =
                    storage::TypedStore::new(storage::PrefixStore::new(store, &MODULE_NAME));
                store
                    .get(state::DYNAMIC_MIN_GAS_PRICE)
                    // Use static min gas price when dynamic price was not yet computed.
                    .unwrap_or(params.min_gas_price)
            })
        } else {
            params.min_gas_price
        }
    }

    fn get_local_min_gas_price<C: Context>(ctx: &C, denom: &token::Denomination) -> u128 {
        #[allow(clippy::borrow_interior_mutable_const)]
        ctx.local_config(MODULE_NAME)
            .as_ref()
            .map(|cfg: &LocalConfig| cfg.min_gas_price.get(denom).copied())
            .unwrap_or_else(|| Cfg::DEFAULT_LOCAL_MIN_GAS_PRICE.get(denom).copied())
            .unwrap_or_default()
    }

    fn get_local_max_estimated_gas<C: Context>(ctx: &C) -> u64 {
        ctx.local_config(MODULE_NAME)
            .as_ref()
            .map(|cfg: &LocalConfig| cfg.max_estimated_gas)
            .unwrap_or_default()
    }

    fn enforce_min_gas_price<C: Context>(ctx: &C, call: &Call) -> Result<(), Error> {
        // If the method is exempt from min gas price requirements, checks always pass.
        #[allow(clippy::borrow_interior_mutable_const)]
        if Cfg::MIN_GAS_PRICE_EXEMPT_METHODS.contains(call.method.as_str()) {
            return Ok(());
        }

        let fee = CurrentState::with_env(|env| env.tx_auth_info().fee.clone());
        let denom = fee.amount.denomination();

        match Self::min_gas_price(denom) {
            // If the denomination is not among the global set, reject.
            None => return Err(Error::GasPriceTooLow),

            // Otherwise, allow overrides during local checks.
            Some(min_gas_price) => {
                if CurrentState::with_env(|env| env.is_check_only()) {
                    let local_mgp = Self::get_local_min_gas_price(ctx, denom);

                    // Reject during local checks.
                    if fee.gas_price() < local_mgp {
                        return Err(Error::GasPriceTooLow);
                    }
                }

                if fee.gas_price() < min_gas_price {
                    return Err(Error::GasPriceTooLow);
                }
            }
        }

        Ok(())
    }

    fn compute_signature_verification_cost(
        params: &Parameters,
        signer_info: &[SignerInfo],
    ) -> Option<u64> {
        let mut num_signature: u64 = 0;
        let mut num_multisig_signer: u64 = 0;
        for si in signer_info {
            match &si.address_spec {
                AddressSpec::Signature(_) => {
                    num_signature = num_signature.checked_add(1)?;
                }
                AddressSpec::Multisig(config) => {
                    num_multisig_signer =
                        num_multisig_signer.checked_add(config.signers.len() as u64)?;
                }
                AddressSpec::Internal(_) => {}
            }
        }

        let signature_cost = num_signature.checked_mul(params.gas_costs.auth_signature)?;
        let multisig_signer_cost =
            num_multisig_signer.checked_mul(params.gas_costs.auth_multisig_signer)?;
        let sum = signature_cost.checked_add(multisig_signer_cost)?;

        Some(sum)
    }
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {
    fn approve_raw_tx<C: Context>(_ctx: &C, tx: &[u8]) -> Result<(), Error> {
        let params = Self::params();
        if tx.len() > TryInto::<usize>::try_into(params.max_tx_size).unwrap() {
            return Err(Error::OversizedTransaction);
        }
        Ok(())
    }

    fn approve_unverified_tx<C: Context>(
        _ctx: &C,
        utx: &UnverifiedTransaction,
    ) -> Result<(), Error> {
        let params = Self::params();
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

    fn authenticate_tx<C: Context>(
        ctx: &C,
        tx: &Transaction,
    ) -> Result<module::AuthDecision, Error> {
        // Check whether the transaction is currently valid.
        let round = ctx.runtime_header().round;
        if let Some(not_before) = tx.auth_info.not_before {
            if round < not_before {
                // Too early.
                return Err(Error::ExpiredTransaction);
            }
        }
        if let Some(not_after) = tx.auth_info.not_after {
            if round > not_after {
                // Too late.
                return Err(Error::ExpiredTransaction);
            }
        }

        Ok(module::AuthDecision::Continue)
    }

    fn before_handle_call<C: Context>(ctx: &C, call: &Call) -> Result<(), Error> {
        // Ensure that specified gas limit is not greater than batch gas limit.
        let params = Self::params();
        let fee = CurrentState::with_env(|env| env.tx_auth_info().fee.clone());
        if fee.gas > params.max_batch_gas {
            return Err(Error::GasOverflow);
        }
        if fee.consensus_messages > ctx.max_messages() {
            return Err(Error::OutOfMessageSlots);
        }

        // Skip additional checks/gas payment for internally generated transactions.
        if CurrentState::with_env(|env| env.is_internal()) {
            return Ok(());
        }

        // Enforce minimum gas price constraints.
        Self::enforce_min_gas_price(ctx, call)?;

        // Charge gas for transaction size.
        let tx_size = CurrentState::with_env(|env| env.tx_size());
        Self::use_tx_gas(
            params
                .gas_costs
                .tx_byte
                .checked_mul(tx_size.into())
                .ok_or(Error::GasOverflow)?,
        )?;

        // Charge gas for signature verification.
        let total = CurrentState::with_env(|env| {
            Self::compute_signature_verification_cost(&params, &env.tx_auth_info().signer_info)
        })
        .ok_or(Error::GasOverflow)?;
        Self::use_tx_gas(total)?;

        // Charge gas for callformat.
        match call.format {
            CallFormat::Plain => {} // No additional gas required.
            CallFormat::EncryptedX25519DeoxysII => {
                Self::use_tx_gas(params.gas_costs.callformat_x25519_deoxysii)?
            }
        }

        Ok(())
    }

    fn after_handle_call<C: Context>(
        ctx: &C,
        result: module::CallResult,
    ) -> Result<module::CallResult, Error> {
        // Skip handling for internally generated calls.
        if CurrentState::with_env(|env| env.is_internal()) {
            return Ok(result);
        }

        let params = Self::params();

        // Compute storage update gas cost.
        let storage_gas = if params.gas_costs.storage_byte > 0 {
            let storage_update_bytes =
                CurrentState::with(|state| state.pending_store_update_byte_size());
            params
                .gas_costs
                .storage_byte
                .saturating_mul(storage_update_bytes as u64)
        } else {
            0
        };

        // Compute message gas cost.
        let message_gas = {
            let emitted_message_count =
                CurrentState::with(|state| state.emitted_messages_local_count());
            // Determine how much each message emission costs based on max_batch_gas and the number
            // of messages that can be emitted per batch.
            let message_gas_cost = params
                .max_batch_gas
                .checked_div(ctx.max_messages().into())
                .unwrap_or(u64::MAX); // If no messages are allowed, cost is infinite.
            message_gas_cost.saturating_mul(emitted_message_count as u64)
        };

        // Compute the gas amount that the transaction should pay in the end.
        let used_gas = Self::used_tx_gas();
        let max_gas = std::cmp::max(used_gas, std::cmp::max(storage_gas, message_gas));

        // Make sure the transaction actually pays for the maximum gas. Note that failure here is
        // fine since the extra resources (storage updates or emitted consensus messages) have not
        // actually been spent yet (this happens at the end of the round).
        let maybe_out_of_gas = Self::use_tx_gas(max_gas - used_gas); // Cannot overflow as max_gas >= used_gas.

        // Emit gas used event.
        if Cfg::EMIT_GAS_USED_EVENTS {
            let used_gas = Self::used_tx_gas();
            CurrentState::with(|state| {
                state.emit_unconditional_event(Event::GasUsed { amount: used_gas });
            });
        }

        // Evaluate the result of the above `use_tx_gas` here to make sure we emit the event.
        maybe_out_of_gas?;

        Ok(result)
    }
}

/// Computes the new minimum gas price based on the current gas usage and the target gas usage.
///
/// The new price is computed as follows (inspired by EIP-1559):
///  - If the actual gas used is greater than the target gas used, increase the minimum gas price.
///  - If the actual gas used is less than the target gas used, decrease the minimum gas price.
///
/// The price change is controlled by the `min_price_max_change_denominator` parameter.
fn min_gas_price_update(
    gas_used: u128,
    target_gas_used: u128,
    min_price_max_change_denominator: u128,
    current_price: u128,
) -> u128 {
    // If the target gas used is zero or the denominator is zero, don't change the price.
    if target_gas_used == 0 || min_price_max_change_denominator == 0 {
        return current_price;
    }

    // Calculate the difference (as a percentage) between the actual gas used in the block and the target gas used.
    let delta = (gas_used.max(target_gas_used) - gas_used.min(target_gas_used)).saturating_mul(100)
        / target_gas_used;

    // Calculate the change in gas price and divide by `min_price_max_change_denominator`.
    let price_change =
        (current_price.saturating_mul(delta) / 100) / min_price_max_change_denominator;

    // Adjust the current price based on whether the gas used was above or below the target.
    if gas_used > target_gas_used {
        current_price.saturating_add(price_change)
    } else {
        current_price.saturating_sub(price_change)
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn begin_block<C: Context>(ctx: &C) {
        CurrentState::with(|state| {
            let epoch = ctx.epoch();

            // Load previous epoch.
            let mut store = storage::PrefixStore::new(state.store(), &MODULE_NAME);
            let mut tstore = storage::TypedStore::new(&mut store);
            let previous_epoch: EpochTime = tstore.get(state::LAST_EPOCH).unwrap_or_default();
            if epoch != previous_epoch {
                tstore.insert(state::LAST_EPOCH, epoch);
            }

            // Set the epoch changed key as needed.
            state
                .block_value(CONTEXT_KEY_EPOCH_CHANGED)
                .set(epoch != previous_epoch);
        });
    }

    fn end_block<C: Context>(_ctx: &C) {
        let params = Self::params();
        if !params.dynamic_min_gas_price.enabled {
            return;
        }

        // Update dynamic min gas price for next block, inspired by EIP-1559.
        //
        // Adjust the min gas price for each block based on the gas used in the previous block and the desired target
        // gas usage set by `target_block_gas_usage_percentage`.
        let gas_used = Self::used_batch_gas() as u128;
        let max_batch_gas = Self::max_batch_gas() as u128;
        let target_gas_used = max_batch_gas.saturating_mul(
            params
                .dynamic_min_gas_price
                .target_block_gas_usage_percentage as u128,
        ) / 100;

        // Compute new prices.
        let mut mgp = Self::min_gas_prices();
        mgp.iter_mut().for_each(|(d, price)| {
            let mut new_min_price = min_gas_price_update(
                gas_used,
                target_gas_used,
                params
                    .dynamic_min_gas_price
                    .min_price_max_change_denominator as u128,
                *price,
            );

            // Ensure that the new price is at least the minimum gas price.
            if let Some(min_price) = params.min_gas_price.get(d) {
                if new_min_price < *min_price {
                    new_min_price = *min_price;
                }
            }
            *price = new_min_price;
        });

        // Update min prices.
        CurrentState::with_store(|store| {
            let mut store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut tstore = storage::TypedStore::new(&mut store);
            tstore.insert(state::DYNAMIC_MIN_GAS_PRICE, mgp);
        });
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
