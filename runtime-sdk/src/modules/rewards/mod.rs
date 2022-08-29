//! Rewards module.
use std::convert::{TryFrom, TryInto};

use num_traits::Zero;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::{
    context::Context,
    core::consensus::beacon,
    module::{self, Module as _, Parameters as _},
    modules, sdk_derive, storage,
    types::address::{Address, SignatureAddressSpec},
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "rewards";

/// Errors emitted by the rewards module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,
}

/// Parameters for the rewards module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub schedule: types::RewardSchedule,

    pub participation_threshold_numerator: u64,
    pub participation_threshold_denominator: u64,
}

/// Errors emitted during rewards parameter validation.
#[derive(Error, Debug)]
pub enum ParameterValidationError {
    #[error("invalid participation threshold (numerator > denominator)")]
    InvalidParticipationThreshold,

    #[error("invalid schedule")]
    InvalidSchedule(#[from] types::RewardScheduleError),
}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    fn validate_basic(&self) -> Result<(), Self::Error> {
        self.schedule.validate_basic()?;

        if self.participation_threshold_numerator > self.participation_threshold_denominator {
            return Err(ParameterValidationError::InvalidParticipationThreshold);
        }
        if self.participation_threshold_denominator.is_zero() {
            return Err(ParameterValidationError::InvalidParticipationThreshold);
        }

        Ok(())
    }
}

/// Genesis state for the rewards module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// State schema constants.
pub mod state {
    /// Last epoch that we processed.
    pub const LAST_EPOCH: &[u8] = &[0x01];
    /// Map of epochs to rewards pending distribution.
    pub const REWARDS: &[u8] = &[0x02];
}

pub struct Module<Accounts: modules::accounts::API> {
    _accounts: std::marker::PhantomData<Accounts>,
}

/// Module's address that has the reward pool.
pub static ADDRESS_REWARD_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "reward-pool"));

impl<Accounts: modules::accounts::API> module::Module for Module<Accounts> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = ();
    type Parameters = Parameters;
}

#[sdk_derive(MethodHandler)]
impl<Accounts: modules::accounts::API> Module<Accounts> {}

impl<Accounts: modules::accounts::API> Module<Accounts> {
    /// Initialize state from genesis.
    fn init<C: Context>(ctx: &mut C, genesis: Genesis) {
        genesis
            .parameters
            .validate_basic()
            .expect("invalid genesis parameters");

        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate<C: Context>(_ctx: &mut C, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

impl<Accounts: modules::accounts::API> module::MigrationHandler for Module<Accounts> {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
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

impl<Accounts: modules::accounts::API> module::TransactionHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::IncomingMessageHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::BlockHandler for Module<Accounts> {
    fn end_block<C: Context>(ctx: &mut C) {
        let epoch = ctx.epoch();

        // Load previous epoch.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut tstore = storage::TypedStore::new(&mut store);
        let previous_epoch: beacon::EpochTime = tstore.get(&state::LAST_EPOCH).unwrap_or_default();
        tstore.insert(&state::LAST_EPOCH, &epoch);

        // Load rewards accumulator for the current epoch.
        let epochs = storage::TypedStore::new(storage::PrefixStore::new(store, &state::REWARDS));
        let mut rewards: types::EpochRewards =
            epochs.get(epoch.to_storage_key()).unwrap_or_default();

        // Reward each good entity.
        for entity_id in &ctx.runtime_round_results().good_compute_entities {
            let address = Address::from_sigspec(&SignatureAddressSpec::Ed25519(entity_id.into()));
            rewards.pending.entry(address).or_default().increment();
        }

        // Punish each bad entity by forbidding rewards for this epoch.
        for entity_id in &ctx.runtime_round_results().bad_compute_entities {
            let address = Address::from_sigspec(&SignatureAddressSpec::Ed25519(entity_id.into()));
            rewards.pending.entry(address).or_default().forbid();
        }

        // Disburse any rewards for previous epochs when the epoch changes.
        if epoch != previous_epoch {
            let params = Self::params(ctx.runtime_state());
            let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
            let mut epochs =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::REWARDS));
            let epoch_rewards: Vec<(DecodableEpochTime, types::EpochRewards)> =
                epochs.iter().collect();

            // Remove all epochs that we will process.
            for (epoch, _) in &epoch_rewards {
                epochs.remove(epoch.0.to_storage_key());
            }

            // Process accumulated rewards for previous epochs.
            'epochs: for (epoch, rewards) in epoch_rewards {
                let epoch = epoch.0;

                // Fetch reward schedule for the given epoch.
                let reward = params.schedule.for_epoch(epoch);
                if reward.amount().is_zero() {
                    continue;
                }

                // Disburse rewards.
                for address in rewards.for_disbursement(
                    params.participation_threshold_numerator,
                    params.participation_threshold_denominator,
                ) {
                    match Accounts::transfer(ctx, *ADDRESS_REWARD_POOL, address, &reward) {
                        Ok(_) => {}
                        Err(modules::accounts::Error::InsufficientBalance) => {
                            // Since rewards are the same for the whole epoch, if there is not
                            // enough in the pool, just continue with the next epoch which may
                            // specify a lower amount or a different denomination.
                            continue 'epochs;
                        }
                        Err(err) => panic!("failed to disburse rewards: {:?}", err),
                    }
                }
            }
        }

        // Update rewards for current epoch.
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut epochs =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::REWARDS));
        epochs.insert(epoch.to_storage_key(), rewards);
    }
}

impl<Accounts: modules::accounts::API> module::InvariantHandler for Module<Accounts> {}

/// A trait that exists solely to convert `beacon::EpochTime` to bytes for use as a storage key.
trait ToStorageKey {
    fn to_storage_key(&self) -> [u8; 8];
}

impl ToStorageKey for beacon::EpochTime {
    fn to_storage_key(&self) -> [u8; 8] {
        self.to_be_bytes()
    }
}

/// A struct that exists solely to decode `beacon::EpochTime` previously encoded via `ToStorageKey`.
struct DecodableEpochTime(beacon::EpochTime);

impl TryFrom<&[u8]> for DecodableEpochTime {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(DecodableEpochTime(beacon::EpochTime::from_be_bytes(
            value.try_into()?,
        )))
    }
}
