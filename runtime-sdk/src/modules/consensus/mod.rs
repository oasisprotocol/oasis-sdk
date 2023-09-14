//! Consensus module.
//!
//! Low level consensus module for communicating with the consensus layer.
use std::{convert::TryInto, num::NonZeroUsize, str::FromStr, sync::Mutex};

use once_cell::sync::Lazy;
use thiserror::Error;

use oasis_core_runtime::{
    common::versioned::Versioned,
    consensus::{
        beacon::EpochTime,
        roothash::{Message, StakingMessage},
        staking,
        staking::{Account as ConsensusAccount, Delegation as ConsensusDelegation},
        state::{
            beacon::ImmutableState as BeaconImmutableState,
            staking::ImmutableState as StakingImmutableState, StateError,
        },
        HEIGHT_LATEST,
    },
};

use crate::{
    context::{Context, TxContext},
    history, migration, module,
    module::{Module as _, Parameters as _},
    modules, sdk_derive,
    types::{
        address::{Address, SignatureAddressSpec},
        message::MessageEventHookInvocation,
        token,
        transaction::AddressSpec,
    },
};

#[cfg(test)]
mod test;

/// Unique module name.
const MODULE_NAME: &str = "consensus";

/// Parameters for the consensus module.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub consensus_denomination: token::Denomination,
    pub consensus_scaling_factor: u64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            consensus_denomination: token::Denomination::from_str("TEST").unwrap(),
            consensus_scaling_factor: 1,
        }
    }
}

/// Errors emitted during rewards parameter validation.
#[derive(Error, Debug)]
pub enum ParameterValidationError {
    #[error("consensus scaling factor set to zero")]
    ZeroScalingFactor,

    #[error("consensus scaling factor is not a power of 10")]
    ScalingFactorNotPowerOf10,
}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    fn validate_basic(&self) -> Result<(), Self::Error> {
        if self.consensus_scaling_factor == 0 {
            return Err(ParameterValidationError::ZeroScalingFactor);
        }

        let log = self.consensus_scaling_factor.ilog10();
        if 10u64.pow(log) != self.consensus_scaling_factor {
            return Err(ParameterValidationError::ScalingFactorNotPowerOf10);
        }

        Ok(())
    }
}
/// Events emitted by the consensus module (none so far).
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("invalid denomination")]
    #[sdk_error(code = 2)]
    InvalidDenomination,

    #[error("internal state: {0}")]
    #[sdk_error(code = 3)]
    InternalStateError(#[from] StateError),

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),

    #[error("consensus incompatible signer")]
    #[sdk_error(code = 4)]
    ConsensusIncompatibleSigner,

    #[error("amount not representable")]
    #[sdk_error(code = 5)]
    AmountNotRepresentable,

    #[error("history: {0}")]
    #[sdk_error(transparent)]
    History(#[from] history::Error),
}

/// Interface that can be called from other modules.
pub trait API {
    /// Transfer an amount from the runtime account.
    fn transfer<C: TxContext>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error>;

    /// Withdraw an amount into the runtime account.
    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error>;

    /// Escrow an amount of the runtime account funds.
    fn escrow<C: TxContext>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error>;

    /// Reclaim an amount of runtime staked shares.
    fn reclaim_escrow<C: TxContext>(
        ctx: &mut C,
        from: Address,
        amount: u128,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error>;

    /// Returns consensus token denomination.
    fn consensus_denomination<C: Context>(ctx: &mut C) -> Result<token::Denomination, Error>;

    /// Ensures transaction signer is consensus compatible.
    fn ensure_compatible_tx_signer<C: TxContext>(ctx: &C) -> Result<(), Error>;

    /// Query consensus account info.
    fn account<C: Context>(ctx: &C, addr: Address) -> Result<ConsensusAccount, Error>;

    /// Query consensus delegation info.
    fn delegation<C: Context>(
        ctx: &C,
        delegator_addr: Address,
        escrow_addr: Address,
    ) -> Result<ConsensusDelegation, Error>;

    /// Convert runtime amount to consensus amount, scaling as needed.
    fn amount_from_consensus<C: Context>(ctx: &mut C, amount: u128) -> Result<u128, Error>;

    /// Convert consensus amount to runtime amount, scaling as needed.
    fn amount_to_consensus<C: Context>(ctx: &mut C, amount: u128) -> Result<u128, Error>;

    /// Determine consensus height corresponding to the given epoch transition. This query may be
    /// expensive in case the epoch is far back.
    fn height_for_epoch<C: Context>(ctx: &C, epoch: EpochTime) -> Result<u64, Error>;
}

pub struct Module;

impl Module {
    fn ensure_consensus_denomination<C: Context>(
        ctx: &mut C,
        denomination: &token::Denomination,
    ) -> Result<(), Error> {
        if denomination != &Self::consensus_denomination(ctx)? {
            return Err(Error::InvalidDenomination);
        }

        Ok(())
    }
}

#[sdk_derive(Module)]
impl Module {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    pub fn init(genesis: Genesis) {
        // Validate genesis parameters.
        genesis
            .parameters
            .validate_basic()
            .expect("invalid genesis parameters");

        // Set genesis parameters.
        Self::set_params(genesis.parameters);
    }
}

impl API for Module {
    fn transfer<C: TxContext>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        Self::ensure_consensus_denomination(ctx, amount.denomination())?;
        let amount = Self::amount_to_consensus(ctx, amount.amount())?;

        ctx.emit_message(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: to.into(),
                    amount: amount.into(),
                }),
            )),
            hook,
        )?;

        Ok(())
    }

    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        Self::ensure_consensus_denomination(ctx, amount.denomination())?;
        let amount = Self::amount_to_consensus(ctx, amount.amount())?;

        ctx.emit_message(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Withdraw(staking::Withdraw {
                    from: from.into(),
                    amount: amount.into(),
                }),
            )),
            hook,
        )?;

        Ok(())
    }

    fn escrow<C: TxContext>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        Self::ensure_consensus_denomination(ctx, amount.denomination())?;
        let amount = Self::amount_to_consensus(ctx, amount.amount())?;

        ctx.emit_message(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::AddEscrow(staking::Escrow {
                    account: to.into(),
                    amount: amount.into(),
                }),
            )),
            hook,
        )?;

        Ok(())
    }

    fn reclaim_escrow<C: TxContext>(
        ctx: &mut C,
        from: Address,
        shares: u128,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        ctx.emit_message(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::ReclaimEscrow(staking::ReclaimEscrow {
                    account: from.into(),
                    shares: shares.into(),
                }),
            )),
            hook,
        )?;

        Ok(())
    }

    fn consensus_denomination<C: Context>(_ctx: &mut C) -> Result<token::Denomination, Error> {
        Ok(Self::params().consensus_denomination)
    }

    fn ensure_compatible_tx_signer<C: TxContext>(ctx: &C) -> Result<(), Error> {
        match ctx.tx_auth_info().signer_info[0].address_spec {
            AddressSpec::Signature(SignatureAddressSpec::Ed25519(_)) => Ok(()),
            _ => Err(Error::ConsensusIncompatibleSigner),
        }
    }

    fn account<C: Context>(ctx: &C, addr: Address) -> Result<ConsensusAccount, Error> {
        let state = StakingImmutableState::new(ctx.consensus_state());
        state
            .account(ctx.io_ctx(), addr.into())
            .map_err(Error::InternalStateError)
    }

    fn delegation<C: Context>(
        ctx: &C,
        delegator_addr: Address,
        escrow_addr: Address,
    ) -> Result<ConsensusDelegation, Error> {
        let state = StakingImmutableState::new(ctx.consensus_state());
        state
            .delegation(ctx.io_ctx(), delegator_addr.into(), escrow_addr.into())
            .map_err(Error::InternalStateError)
    }

    fn amount_from_consensus<C: Context>(_ctx: &mut C, amount: u128) -> Result<u128, Error> {
        let scaling_factor = Self::params().consensus_scaling_factor;
        amount
            .checked_mul(scaling_factor.into())
            .ok_or(Error::AmountNotRepresentable)
    }

    fn amount_to_consensus<C: Context>(_ctx: &mut C, amount: u128) -> Result<u128, Error> {
        let scaling_factor = Self::params().consensus_scaling_factor;
        let scaled = amount
            .checked_div(scaling_factor.into())
            .ok_or(Error::AmountNotRepresentable)?;

        // Ensure there is no remainder as that is not representable in the consensus layer.
        let remainder = amount
            .checked_rem(scaling_factor.into())
            .ok_or(Error::AmountNotRepresentable)?;
        if remainder != 0 {
            return Err(Error::AmountNotRepresentable);
        }

        Ok(scaled)
    }

    fn height_for_epoch<C: Context>(ctx: &C, epoch: EpochTime) -> Result<u64, Error> {
        static HEIGHT_CACHE: Lazy<Mutex<lru::LruCache<EpochTime, u64>>> =
            Lazy::new(|| Mutex::new(lru::LruCache::new(NonZeroUsize::new(128).unwrap())));

        // Check the cache first to avoid more expensive traversals.
        let mut cache = HEIGHT_CACHE.lock().unwrap();
        if let Some(height) = cache.get(&epoch) {
            return Ok(*height);
        }

        // Resolve height for the given epoch.
        let mut height = HEIGHT_LATEST;
        loop {
            let state = ctx.history().consensus_state_at(height)?;

            let beacon = BeaconImmutableState::new(&state);

            let mut epoch_state = beacon.future_epoch_state(ctx.io_ctx())?;
            if epoch_state.height > state.height().try_into().unwrap() {
                // Use current epoch if future epoch is in the future.
                epoch_state = beacon.epoch_state(ctx.io_ctx()).unwrap();
            }
            height = epoch_state.height.try_into().unwrap();

            // Cache height for later queries.
            cache.put(epoch_state.epoch, height);

            if epoch_state.epoch == epoch {
                return Ok(height);
            }

            assert!(epoch_state.epoch > epoch);
            assert!(height > 1);

            // Go one height before epoch transition.
            height -= 1;
        }
    }
}

impl module::TransactionHandler for Module {}

impl module::BlockHandler for Module {}

impl module::InvariantHandler for Module {}
