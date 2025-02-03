//! Accounts module.
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
};

use num_traits::Zero;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::{
    context::Context,
    core::common::quantity::Quantity,
    handler, migration,
    module::{self, FeeProxyHandler, Module as _, Parameters as _},
    modules,
    modules::core::{Error as CoreError, API as _},
    runtime::Runtime,
    sdk_derive,
    sender::SenderMeta,
    state::CurrentState,
    storage,
    storage::Prefix,
    types::{
        address::{Address, SignatureAddressSpec},
        token,
        transaction::{AuthInfo, Transaction},
    },
};

pub mod fee;
#[cfg(test)]
pub(crate) mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "accounts";

/// Maximum delta that the transaction nonce can be in the future from the current nonce to still
/// be accepted during transaction checks.
const MAX_CHECK_NONCE_FUTURE_DELTA: u64 = 0; // Increase once supported in Oasis Core.

/// Errors emitted by the accounts module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("insufficient balance")]
    #[sdk_error(code = 2)]
    InsufficientBalance,

    #[error("forbidden by policy")]
    #[sdk_error(code = 3)]
    Forbidden,

    #[error("not found")]
    #[sdk_error(code = 4)]
    NotFound,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
}

/// Events emitted by the accounts module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Transfer {
        from: Address,
        to: Address,
        amount: token::BaseUnits,
    },

    #[sdk_event(code = 2)]
    Burn {
        owner: Address,
        amount: token::BaseUnits,
    },

    #[sdk_event(code = 3)]
    Mint {
        owner: Address,
        amount: token::BaseUnits,
    },
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_transfer: u64,
}

/// Parameters for the accounts module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub transfers_disabled: bool,
    pub gas_costs: GasCosts,

    #[cbor(optional)]
    pub debug_disable_nonce_check: bool,

    #[cbor(optional)]
    pub denomination_infos: BTreeMap<token::Denomination, types::DenominationInfo>,
}

/// Errors emitted during rewards parameter validation.
#[derive(Error, Debug)]
pub enum ParameterValidationError {
    #[error("debug option used: {0}")]
    DebugOptionUsed(String),
}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    #[cfg(not(feature = "unsafe-allow-debug"))]
    fn validate_basic(&self) -> Result<(), Self::Error> {
        if self.debug_disable_nonce_check {
            return Err(ParameterValidationError::DebugOptionUsed(
                "debug_disable_nonce_check".to_string(),
            ));
        }

        Ok(())
    }
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
    pub accounts: BTreeMap<Address, types::Account>,
    pub balances: BTreeMap<Address, BTreeMap<token::Denomination, u128>>,
    pub total_supplies: BTreeMap<token::Denomination, u128>,
}

/// Interface that can be called from other modules.
pub trait API {
    /// Transfer an amount from one account to the other.
    fn transfer(from: Address, to: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    /// Transfer an amount from one account to the other without emitting an event.
    fn transfer_silent(from: Address, to: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    /// Mint new tokens, increasing the total supply.
    fn mint(to: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    /// Burn existing tokens, decreasing the total supply.
    fn burn(from: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    /// Sets an account's nonce.
    fn set_nonce(address: Address, nonce: u64);

    /// Fetch an account's current nonce.
    fn get_nonce(address: Address) -> Result<u64, Error>;

    /// Increments an account's nonce.
    fn inc_nonce(address: Address);

    /// Sets an account's balance of the given denomination.
    ///
    /// # Warning
    ///
    /// This method is dangerous as it can result in invariant violations.
    fn set_balance(address: Address, amount: &token::BaseUnits);

    /// Fetch an account's balance of the given denomination.
    fn get_balance(address: Address, denomination: token::Denomination) -> Result<u128, Error>;

    /// Ensures that the given account has at least the specified balance.
    fn ensure_balance(address: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        let balance = Self::get_balance(address, amount.denomination().clone())?;
        if balance < amount.amount() {
            Err(Error::InsufficientBalance)
        } else {
            Ok(())
        }
    }

    /// Get allowance for an address and denomination.
    ///
    /// The allowance is the amount an account owner allows another account to
    /// spend from the owner's account for a given denomination.
    ///
    /// Note that the API user is responsible for taking allowances into
    /// account, the transfer functions in this API do not.
    fn get_allowance(
        owner: Address,
        beneficiary: Address,
        denomination: token::Denomination,
    ) -> Result<u128, Error>;

    /// Set a user's allowance for spending tokens for the given denomination
    /// from the owner's account.
    fn set_allowance(owner: Address, beneficiary: Address, amount: &token::BaseUnits);

    /// Fetch an account's current balances.
    fn get_balances(address: Address) -> Result<types::AccountBalances, Error>;

    /// Fetch addresses.
    fn get_addresses(denomination: token::Denomination) -> Result<Vec<Address>, Error>;

    /// Fetch total supplies.
    fn get_total_supplies() -> Result<BTreeMap<token::Denomination, u128>, Error>;

    /// Fetch the total supply for the given denomination.
    fn get_total_supply(denomination: token::Denomination) -> Result<u128, Error>;

    /// Sets the total supply for the given denomination.
    ///
    /// # Warning
    ///
    /// This method is dangerous as it can result in invariant violations.
    fn set_total_supply(amount: &token::BaseUnits);

    /// Fetch information about a denomination.
    fn get_denomination_info(
        denomination: &token::Denomination,
    ) -> Result<types::DenominationInfo, Error>;

    /// Moves the amount into the per-transaction fee accumulator.
    fn charge_tx_fee(from: Address, amount: &token::BaseUnits) -> Result<(), modules::core::Error>;

    /// Indicates that the unused portion of the transaction fee should be refunded after the
    /// transaction completes (even in case it fails).
    fn set_refund_unused_tx_fee(refund: bool);

    /// Take the flag indicating that the unused portion of the transaction fee should be refunded
    /// after the transaction completes is set.
    ///
    /// After calling this method the flag is reset to `false`.
    fn take_refund_unused_tx_fee() -> bool;

    /// Check transaction signer account nonces.
    /// Return payer address.
    fn check_signer_nonces<C: Context>(
        ctx: &C,
        tx_auth_info: &AuthInfo,
    ) -> Result<Address, modules::core::Error>;

    /// Update transaction signer account nonces.
    fn update_signer_nonces<C: Context>(
        ctx: &C,
        tx_auth_info: &AuthInfo,
    ) -> Result<(), modules::core::Error>;
}

/// State schema constants.
pub mod state {
    /// Map of account addresses to account metadata.
    pub const ACCOUNTS: &[u8] = &[0x01];
    /// Map of account addresses to map of denominations to balances.
    pub const BALANCES: &[u8] = &[0x02];
    /// Map of total supplies (per denomination).
    pub const TOTAL_SUPPLY: &[u8] = &[0x03];
    /// Map of allowances (per denomination).
    pub const ALLOWANCES: &[u8] = &[0x04];
}

pub struct Module;

/// Module's address that has the common pool.
///
/// oasis1qz78phkdan64g040cvqvqpwkplfqf6tj6uwcsh30
pub static ADDRESS_COMMON_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "common-pool"));
/// Module's address that has the fee accumulator.
///
/// oasis1qp3r8hgsnphajmfzfuaa8fhjag7e0yt35cjxq0u4
pub static ADDRESS_FEE_ACCUMULATOR: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "fee-accumulator"));

/// This is needed to properly iterate over the BALANCES map.
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
struct AddressWithDenomination(Address, token::Denomination);

#[derive(Error, Debug)]
enum AWDError {
    #[error("malformed address")]
    MalformedAddress,

    #[error("malformed denomination")]
    MalformedDenomination,
}

impl std::convert::TryFrom<&[u8]> for AddressWithDenomination {
    type Error = AWDError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let address =
            Address::try_from(&bytes[..Address::SIZE]).map_err(|_| AWDError::MalformedAddress)?;
        let denomination = token::Denomination::try_from(&bytes[Address::SIZE..])
            .map_err(|_| AWDError::MalformedDenomination)?;
        Ok(AddressWithDenomination(address, denomination))
    }
}

impl Module {
    /// Add given amount of tokens to the specified account's balance.
    fn add_amount(addr: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        if amount.amount() == 0 {
            return Ok(());
        }

        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances = storage::PrefixStore::new(store, &state::BALANCES);
            let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
            let mut value: u128 = account.get(amount.denomination()).unwrap_or_default();

            value = value
                .checked_add(amount.amount())
                .ok_or(Error::InvalidArgument)?;
            account.insert(amount.denomination(), value);
            Ok(())
        })
    }

    /// Subtract given amount of tokens from the specified account's balance.
    fn sub_amount(addr: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        if amount.amount() == 0 {
            return Ok(());
        }

        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances = storage::PrefixStore::new(store, &state::BALANCES);
            let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
            let mut value: u128 = account.get(amount.denomination()).unwrap_or_default();

            value = value
                .checked_sub(amount.amount())
                .ok_or(Error::InsufficientBalance)?;
            account.insert(amount.denomination(), value);
            Ok(())
        })
    }

    /// Increment the total supply for the given amount.
    fn inc_total_supply(amount: &token::BaseUnits) -> Result<(), Error> {
        if amount.amount() == 0 {
            return Ok(());
        }

        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut total_supplies =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
            let mut total_supply: u128 = total_supplies
                .get(amount.denomination())
                .unwrap_or_default();

            total_supply = total_supply
                .checked_add(amount.amount())
                .ok_or(Error::InvalidArgument)?;
            total_supplies.insert(amount.denomination(), total_supply);
            Ok(())
        })
    }

    /// Decrement the total supply for the given amount.
    fn dec_total_supply(amount: &token::BaseUnits) -> Result<(), Error> {
        if amount.amount() == 0 {
            return Ok(());
        }

        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut total_supplies =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
            let mut total_supply: u128 = total_supplies
                .get(amount.denomination())
                .unwrap_or_default();

            total_supply = total_supply
                .checked_sub(amount.amount())
                .ok_or(Error::InsufficientBalance)?;
            total_supplies.insert(amount.denomination(), total_supply);
            Ok(())
        })
    }

    /// Get all balances.
    fn get_all_balances() -> Result<BTreeMap<Address, BTreeMap<token::Denomination, u128>>, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::BALANCES));

            // Unfortunately, we can't just return balances.iter().collect() here,
            // because the stored format doesn't match -- we need this workaround
            // instead.

            let balmap: BTreeMap<AddressWithDenomination, u128> = balances.iter().collect();

            let mut b: BTreeMap<Address, BTreeMap<token::Denomination, u128>> = BTreeMap::new();

            for (addrden, amt) in &balmap {
                let addr = &addrden.0;
                let den = &addrden.1;

                // Fetch existing account's balances or insert blank ones.
                let addr_bals = b.entry(*addr).or_default();

                // Add to given denomination's balance or insert it if new.
                addr_bals
                    .entry(den.clone())
                    .and_modify(|a| *a += amt)
                    .or_insert_with(|| *amt);
            }

            Ok(b)
        })
    }
}

/// Context key for the per-transaction unused fee refund decision.
const CONTEXT_KEY_TX_FEE_REFUND_UNUSED: &str = "accounts.TxRefundUnusedFee";
/// Context key for the per block fee manager.
const CONTEXT_KEY_FEE_MANAGER: &str = "accounts.FeeManager";

impl API for Module {
    fn transfer(from: Address, to: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        if CurrentState::with_env(|env| env.is_check_only()) || amount.amount() == 0 {
            return Ok(());
        }

        Self::transfer_silent(from, to, amount)?;

        // Emit a transfer event.
        CurrentState::with(|state| {
            state.emit_event(Event::Transfer {
                from,
                to,
                amount: amount.clone(),
            })
        });

        Ok(())
    }

    fn transfer_silent(from: Address, to: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        // Subtract from source account.
        Self::sub_amount(from, amount)?;
        // Add to destination account.
        Self::add_amount(to, amount)?;

        Ok(())
    }

    fn mint(to: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        if CurrentState::with_env(|env| env.is_check_only()) || amount.amount() == 0 {
            return Ok(());
        }

        // Add to destination account.
        Self::add_amount(to, amount)?;

        // Increase total supply.
        Self::inc_total_supply(amount)?;

        // Emit a mint event.
        CurrentState::with(|state| {
            state.emit_event(Event::Mint {
                owner: to,
                amount: amount.clone(),
            });
        });

        Ok(())
    }

    fn burn(from: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        if CurrentState::with_env(|env| env.is_check_only()) || amount.amount() == 0 {
            return Ok(());
        }

        // Remove from target account.
        Self::sub_amount(from, amount)?;

        // Decrease total supply.
        Self::dec_total_supply(amount)
            .expect("target account had enough balance so total supply should not underflow");

        // Emit a burn event.
        CurrentState::with(|state| {
            state.emit_event(Event::Burn {
                owner: from,
                amount: amount.clone(),
            });
        });

        Ok(())
    }

    fn set_nonce(address: Address, nonce: u64) {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut accounts =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
            let mut account: types::Account = accounts.get(address).unwrap_or_default();
            account.nonce = nonce;
            accounts.insert(address, account);
        })
    }

    fn get_nonce(address: Address) -> Result<u64, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let accounts =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
            let account: types::Account = accounts.get(address).unwrap_or_default();
            Ok(account.nonce)
        })
    }

    fn inc_nonce(address: Address) {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut accounts =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
            let mut account: types::Account = accounts.get(address).unwrap_or_default();
            account.nonce = account.nonce.saturating_add(1);
            accounts.insert(address, account);
        })
    }

    fn set_balance(address: Address, amount: &token::BaseUnits) {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances = storage::PrefixStore::new(store, &state::BALANCES);
            let mut account =
                storage::TypedStore::new(storage::PrefixStore::new(balances, &address));
            account.insert(amount.denomination(), amount.amount());
        });
    }

    fn get_balance(address: Address, denomination: token::Denomination) -> Result<u128, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances = storage::PrefixStore::new(store, &state::BALANCES);
            let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));

            Ok(account.get(denomination).unwrap_or_default())
        })
    }

    fn get_balances(address: Address) -> Result<types::AccountBalances, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances = storage::PrefixStore::new(store, &state::BALANCES);
            let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));

            Ok(types::AccountBalances {
                balances: account.iter().collect(),
            })
        })
    }

    fn get_allowance(
        owner: Address,
        beneficiary: Address,
        denomination: token::Denomination,
    ) -> Result<u128, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let allowances = storage::PrefixStore::new(store, &state::ALLOWANCES);
            let for_owner = storage::PrefixStore::new(allowances, &owner);
            let for_beneficiary =
                storage::TypedStore::new(storage::PrefixStore::new(for_owner, &beneficiary));

            Ok(for_beneficiary.get(denomination).unwrap_or_default())
        })
    }

    fn set_allowance(owner: Address, beneficiary: Address, amount: &token::BaseUnits) {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let allowances = storage::PrefixStore::new(store, &state::ALLOWANCES);
            let for_owner = storage::PrefixStore::new(allowances, &owner);
            let mut for_beneficiary =
                storage::TypedStore::new(storage::PrefixStore::new(for_owner, &beneficiary));

            for_beneficiary.insert(amount.denomination(), amount.amount());
        })
    }

    fn get_addresses(denomination: token::Denomination) -> Result<Vec<Address>, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let balances: BTreeMap<AddressWithDenomination, Quantity> =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::BALANCES))
                    .iter()
                    .collect();

            Ok(balances
                .into_keys()
                .filter(|bal| bal.1 == denomination)
                .map(|bal| bal.0)
                .collect())
        })
    }

    fn get_total_supplies() -> Result<BTreeMap<token::Denomination, u128>, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let ts =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));

            Ok(ts.iter().collect())
        })
    }

    fn get_total_supply(denomination: token::Denomination) -> Result<u128, Error> {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let ts =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
            Ok(ts.get(denomination).unwrap_or_default())
        })
    }

    fn set_total_supply(amount: &token::BaseUnits) {
        CurrentState::with_store(|store| {
            let store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut total_supplies =
                storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
            total_supplies.insert(amount.denomination(), amount.amount());
        });
    }

    fn get_denomination_info(
        denomination: &token::Denomination,
    ) -> Result<types::DenominationInfo, Error> {
        Self::params()
            .denomination_infos
            .get(denomination)
            .cloned()
            .ok_or(Error::NotFound)
    }

    fn charge_tx_fee(from: Address, amount: &token::BaseUnits) -> Result<(), modules::core::Error> {
        if CurrentState::with_env(|env| env.is_simulation()) {
            return Ok(());
        }

        Self::sub_amount(from, amount).map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

        CurrentState::with(|state| {
            state
                .block_value::<fee::FeeManager>(CONTEXT_KEY_FEE_MANAGER)
                .or_default()
                .record_fee(from, amount);
        });

        Ok(())
    }

    fn set_refund_unused_tx_fee(refund: bool) {
        CurrentState::with(|state| {
            if state.env().is_simulation() {
                return;
            }

            state
                .block_value(CONTEXT_KEY_TX_FEE_REFUND_UNUSED)
                .set(refund);
        });
    }

    fn take_refund_unused_tx_fee() -> bool {
        CurrentState::with(|state| {
            if state.env().is_simulation() {
                return false;
            }

            state
                .block_value(CONTEXT_KEY_TX_FEE_REFUND_UNUSED)
                .take()
                .unwrap_or(false)
        })
    }

    fn check_signer_nonces<C: Context>(
        _ctx: &C,
        auth_info: &AuthInfo,
    ) -> Result<Address, modules::core::Error> {
        let is_pre_schedule = CurrentState::with_env(|env| env.is_pre_schedule());
        let is_check_only = CurrentState::with_env(|env| env.is_check_only());

        // TODO: Optimize the check/update pair so that the accounts are
        // fetched only once.
        let params = Self::params();
        let sender = CurrentState::with_store(|store| {
            // Fetch information about each signer.
            let mut store = storage::PrefixStore::new(store, &MODULE_NAME);
            let accounts =
                storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
            let mut sender = None;
            for si in auth_info.signer_info.iter() {
                let address = si.address_spec.address();
                let account: types::Account = accounts.get(address).unwrap_or_default();

                // First signer pays for the fees and is considered the sender.
                if sender.is_none() {
                    sender = Some(SenderMeta {
                        address,
                        tx_nonce: si.nonce,
                        state_nonce: account.nonce,
                    });
                }

                // When nonce checking is disabled, skip the rest of the checks.
                if params.debug_disable_nonce_check {
                    continue;
                }

                // Check signer nonce against the corresponding account nonce.
                match si.nonce.cmp(&account.nonce) {
                    Ordering::Less => {
                        // In the past and will never become valid, reject.
                        return Err(modules::core::Error::InvalidNonce);
                    }
                    Ordering::Equal => {} // Ok.
                    Ordering::Greater => {
                        // If too much in the future, reject.
                        if si.nonce - account.nonce > MAX_CHECK_NONCE_FUTURE_DELTA {
                            return Err(modules::core::Error::InvalidNonce);
                        }

                        // If in the future and this is before scheduling, reject with separate error
                        // that will make the scheduler skip the transaction.
                        if is_pre_schedule {
                            return Err(modules::core::Error::FutureNonce);
                        }

                        // If in the future and this is during execution, reject.
                        if !is_check_only {
                            return Err(modules::core::Error::InvalidNonce);
                        }

                        // If in the future and this is during checks, accept.
                    }
                }
            }

            Ok(sender)
        })?;

        // Configure the sender.
        let sender = sender.expect("at least one signer is always present");
        let sender_address = sender.address;
        if is_check_only {
            <C::Runtime as Runtime>::Core::set_sender_meta(sender);
        }

        Ok(sender_address)
    }

    fn update_signer_nonces<C: Context>(
        _ctx: &C,
        auth_info: &AuthInfo,
    ) -> Result<(), modules::core::Error> {
        CurrentState::with_store(|store| {
            // Fetch information about each signer.
            let mut store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut accounts =
                storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
            for si in auth_info.signer_info.iter() {
                let address = si.address_spec.address();
                let mut account: types::Account = accounts.get(address).unwrap_or_default();

                // Update nonce.
                account.nonce = account
                    .nonce
                    .checked_add(1)
                    .ok_or(modules::core::Error::InvalidNonce)?; // Should never overflow.
                accounts.insert(address, account);
            }
            Ok(())
        })
    }
}

#[sdk_derive(Module)]
impl Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    pub fn init(genesis: Genesis) {
        CurrentState::with_store(|store| {
            // Create accounts.
            let mut store = storage::PrefixStore::new(store, &MODULE_NAME);
            let mut accounts =
                storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
            for (address, account) in genesis.accounts {
                accounts.insert(address, account);
            }

            // Create balances.
            let mut balances = storage::PrefixStore::new(&mut store, &state::BALANCES);
            let mut computed_total_supply: BTreeMap<token::Denomination, u128> = BTreeMap::new();
            for (address, denominations) in genesis.balances.iter() {
                let mut account =
                    storage::TypedStore::new(storage::PrefixStore::new(&mut balances, &address));
                for (denomination, value) in denominations {
                    account.insert(denomination, value);

                    // Update computed total supply.
                    computed_total_supply
                        .entry(denomination.clone())
                        .and_modify(|v| *v += value)
                        .or_insert_with(|| *value);
                }
            }

            // Validate and set total supply.
            let mut total_supplies = storage::TypedStore::new(storage::PrefixStore::new(
                &mut store,
                &state::TOTAL_SUPPLY,
            ));
            for (denomination, total_supply) in genesis.total_supplies.iter() {
                let computed = computed_total_supply
                    .remove(denomination)
                    .expect("unexpected total supply");
                assert!(
                    &computed == total_supply,
                    "unexpected total supply (expected: {total_supply} got: {computed})",
                );

                total_supplies.insert(denomination, total_supply);
            }
            if let Some((denomination, total_supply)) = computed_total_supply.iter().next() {
                panic!("missing expected total supply: {total_supply} {denomination}",);
            }
        });

        // Validate genesis parameters.
        genesis
            .parameters
            .validate_basic()
            .expect("invalid genesis parameters");

        // Set genesis parameters.
        Self::set_params(genesis.parameters);
    }

    #[handler(prefetch = "accounts.Transfer")]
    fn prefetch_transfer(
        add_prefix: &mut dyn FnMut(Prefix),
        body: cbor::Value,
        auth_info: &AuthInfo,
    ) -> Result<(), crate::error::RuntimeError> {
        let args: types::Transfer = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
        let from = auth_info.signer_info[0].address_spec.address();

        // Prefetch accounts 'to'.
        add_prefix(Prefix::from(
            [MODULE_NAME.as_bytes(), state::ACCOUNTS, args.to.as_ref()].concat(),
        ));
        add_prefix(Prefix::from(
            [MODULE_NAME.as_bytes(), state::BALANCES, args.to.as_ref()].concat(),
        ));
        // Prefetch accounts 'from'.
        add_prefix(Prefix::from(
            [MODULE_NAME.as_bytes(), state::ACCOUNTS, from.as_ref()].concat(),
        ));
        add_prefix(Prefix::from(
            [MODULE_NAME.as_bytes(), state::BALANCES, from.as_ref()].concat(),
        ));

        Ok(())
    }

    #[handler(call = "accounts.Transfer")]
    fn tx_transfer<C: Context>(_ctx: &C, body: types::Transfer) -> Result<(), Error> {
        let params = Self::params();

        // Reject transfers when they are disabled.
        if params.transfers_disabled {
            return Err(Error::Forbidden);
        }

        <C::Runtime as Runtime>::Core::use_tx_gas(params.gas_costs.tx_transfer)?;

        let tx_caller_address = CurrentState::with_env(|env| env.tx_caller_address());
        Self::transfer(tx_caller_address, body.to, &body.amount)?;

        Ok(())
    }

    #[handler(query = "accounts.Nonce")]
    fn query_nonce<C: Context>(_ctx: &C, args: types::NonceQuery) -> Result<u64, Error> {
        Self::get_nonce(args.address)
    }

    #[handler(query = "accounts.Addresses", expensive)]
    fn query_addresses<C: Context>(
        _ctx: &C,
        args: types::AddressesQuery,
    ) -> Result<Vec<Address>, Error> {
        Self::get_addresses(args.denomination)
    }

    #[handler(query = "accounts.Balances")]
    fn query_balances<C: Context>(
        _ctx: &C,
        args: types::BalancesQuery,
    ) -> Result<types::AccountBalances, Error> {
        Self::get_balances(args.address)
    }

    #[handler(query = "accounts.DenominationInfo")]
    fn query_denomination_info<C: Context>(
        _ctx: &C,
        args: types::DenominationInfoQuery,
    ) -> Result<types::DenominationInfo, Error> {
        Self::get_denomination_info(&args.denomination)
    }
}

impl module::TransactionHandler for Module {
    fn authenticate_tx<C: Context>(
        ctx: &C,
        tx: &Transaction,
    ) -> Result<module::AuthDecision, modules::core::Error> {
        // Check nonces.
        let default_payer = Self::check_signer_nonces(ctx, &tx.auth_info)?;

        // Attempt to resolve a proxy fee payer if set.
        let payer =
            <C::Runtime as Runtime>::FeeProxy::resolve_payer(ctx, tx)?.unwrap_or(default_payer);

        // Charge the specified amount of fees.
        if !tx.auth_info.fee.amount.amount().is_zero() {
            if CurrentState::with_env(|env| env.is_check_only()) {
                // Do not update balances during transaction checks. In case of checks, only do it
                // after all the other checks have already passed as otherwise retrying the
                // transaction will not be possible.
                Self::ensure_balance(payer, &tx.auth_info.fee.amount)
                    .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

                // Make sure to record the payer during transaction checks.
                CurrentState::with(|state| {
                    state
                        .block_value::<fee::FeeManager>(CONTEXT_KEY_FEE_MANAGER)
                        .or_default()
                        .record_fee(payer, &tx.auth_info.fee.amount);
                });
            } else {
                // Actually perform the move.
                Self::charge_tx_fee(payer, &tx.auth_info.fee.amount)?;
            }

            let gas_price = tx.auth_info.fee.gas_price();
            // Set transaction priority.
            <C::Runtime as Runtime>::Core::set_priority(gas_price.try_into().unwrap_or(u64::MAX));
        }

        // Do not update nonces early during transaction checks. In case of checks, only do it after
        // all the other checks have already passed as otherwise retrying the transaction will not
        // be possible.
        if !CurrentState::with_env(|env| env.is_check_only()) {
            Self::update_signer_nonces(ctx, &tx.auth_info)?;
        }

        Ok(module::AuthDecision::Continue)
    }

    fn after_handle_call<C: Context>(
        _ctx: &C,
        result: module::CallResult,
    ) -> Result<module::CallResult, modules::core::Error> {
        // Check whether unused part of the fee should be refunded.
        let refund_fee = if Self::take_refund_unused_tx_fee() {
            let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();
            let gas_price = CurrentState::with_env(|env| env.tx_auth_info().fee.gas_price());

            gas_price.saturating_mul(remaining_gas.into())
        } else {
            0
        };

        CurrentState::with(|state| {
            let mgr = state
                .block_value::<fee::FeeManager>(CONTEXT_KEY_FEE_MANAGER)
                .or_default();

            // Update the per-tx fee accumulator. State must be updated in `after_dispatch_tx` as
            // otherwise any state updates may be reverted in case call result is a failure.
            mgr.record_refund(refund_fee);

            // Emit event for paid fee.
            let tx_fee = mgr.tx_fee().cloned().unwrap_or_default();
            if tx_fee.amount() > 0 {
                state.emit_unconditional_event(Event::Transfer {
                    from: tx_fee.payer(),
                    to: *ADDRESS_FEE_ACCUMULATOR,
                    amount: token::BaseUnits::new(tx_fee.amount(), tx_fee.denomination()),
                });
            }
        });

        Ok(result)
    }

    fn after_dispatch_tx<C: Context>(
        ctx: &C,
        tx_auth_info: &AuthInfo,
        result: &module::CallResult,
    ) {
        // Move transaction fees into the per-block fee accumulator.
        let fee_updates = CurrentState::with(|state| {
            let mgr = state
                .block_value::<fee::FeeManager>(CONTEXT_KEY_FEE_MANAGER)
                .or_default();
            mgr.commit_tx()
        });
        // Refund any fees. This needs to happen after tx dispatch to ensure state is updated.
        Self::add_amount(fee_updates.payer, &fee_updates.refund).unwrap();

        if !CurrentState::with_env(|env| env.is_check_only()) {
            // Do nothing further outside transaction checks.
            return;
        }
        if !matches!(result, module::CallResult::Ok(_)) {
            // Do nothing in case the call failed to allow retries.
            return;
        }

        // Update payer balance.
        Self::sub_amount(fee_updates.payer, &tx_auth_info.fee.amount).unwrap(); // Already checked.

        // Update nonces.
        Self::update_signer_nonces(ctx, tx_auth_info).unwrap();
    }
}

impl module::BlockHandler for Module {
    fn end_block<C: Context>(ctx: &C) {
        // Determine the fees that are available for disbursement from the last block.
        let mut previous_fees = Self::get_balances(*ADDRESS_FEE_ACCUMULATOR)
            .expect("get_balances must succeed")
            .balances;

        // Drain previous fees from the fee accumulator.
        for (denom, remainder) in &previous_fees {
            Self::sub_amount(
                *ADDRESS_FEE_ACCUMULATOR,
                &token::BaseUnits::new(*remainder, denom.clone()),
            )
            .expect("sub_amount must succeed");
        }

        // Disburse transaction fees to entities controlling all the good nodes in the committee.
        let addrs: Vec<Address> = ctx
            .runtime_round_results()
            .good_compute_entities
            .iter()
            .map(|pk| Address::from_sigspec(&SignatureAddressSpec::Ed25519(pk.into())))
            .collect();

        if !addrs.is_empty() {
            let amounts: Vec<_> = previous_fees
                .iter()
                .filter_map(|(denom, fee)| {
                    let fee = fee
                        .checked_div(addrs.len() as u128)
                        .expect("addrs is non-empty");

                    // Filter out zero-fee entries to avoid needless operations.
                    if fee.is_zero() {
                        None
                    } else {
                        Some(token::BaseUnits::new(fee, denom.clone()))
                    }
                })
                .collect();

            for address in addrs {
                for amount in &amounts {
                    let remaining = previous_fees
                        .get_mut(amount.denomination())
                        .expect("designated denomination should be there");
                    *remaining = remaining
                        .checked_sub(amount.amount())
                        .expect("there should be enough to disburse");

                    Self::add_amount(address, amount)
                        .expect("add_amount must succeed for fee disbursement");

                    // Emit transfer event for fee disbursement.
                    CurrentState::with(|state| {
                        state.emit_event(Event::Transfer {
                            from: *ADDRESS_FEE_ACCUMULATOR,
                            to: address,
                            amount: amount.clone(),
                        });
                    });
                }
            }
        }

        // Transfer remainder to a common pool account.
        for (denom, remainder) in previous_fees.into_iter() {
            if remainder.is_zero() {
                continue;
            }

            let amount = token::BaseUnits::new(remainder, denom);
            Self::add_amount(*ADDRESS_COMMON_POOL, &amount)
                .expect("add_amount must succeed for transfer to common pool");

            // Emit transfer event for fee disbursement.
            CurrentState::with(|state| {
                state.emit_event(Event::Transfer {
                    from: *ADDRESS_FEE_ACCUMULATOR,
                    to: *ADDRESS_COMMON_POOL,
                    amount,
                })
            });
        }

        // Fees for the active block should be transferred to the fee accumulator address.
        let block_fees = CurrentState::with(|state| {
            let mgr = state
                .block_value::<fee::FeeManager>(CONTEXT_KEY_FEE_MANAGER)
                .take()
                .unwrap_or_default();
            mgr.commit_block().into_iter()
        });

        for (denom, amount) in block_fees {
            Self::add_amount(
                *ADDRESS_FEE_ACCUMULATOR,
                &token::BaseUnits::new(amount, denom),
            )
            .expect("add_amount must succeed for transfer to fee accumulator")
        }
    }
}

impl module::InvariantHandler for Module {
    /// Check invariants.
    fn check_invariants<C: Context>(_ctx: &C) -> Result<(), CoreError> {
        // All account balances should sum up to the total supply for their
        // corresponding denominations.

        #[allow(clippy::or_fun_call)]
        let balances = Self::get_all_balances().or(Err(CoreError::InvariantViolation(
            "unable to get balances of all accounts".to_string(),
        )))?;
        #[allow(clippy::or_fun_call)]
        let total_supplies = Self::get_total_supplies().or(Err(CoreError::InvariantViolation(
            "unable to get total supplies".to_string(),
        )))?;

        // First, compute total supplies based on account balances.
        let mut computed_ts: BTreeMap<token::Denomination, u128> = BTreeMap::new();

        for bals in balances.values() {
            for (den, amt) in bals {
                computed_ts
                    .entry(den.clone())
                    .and_modify(|a| *a += amt)
                    .or_insert_with(|| *amt);
            }
        }

        // Now check if the computed and given total supplies match.
        for (den, ts) in &total_supplies {
            // Return error if total supplies have a denomination that we
            // didn't encounter when computing total supplies based on account
            // balances.
            #[allow(clippy::or_fun_call)]
            let computed = computed_ts
                .remove(den)
                .ok_or(CoreError::InvariantViolation(
                    "unexpected denomination".to_string(),
                ))?;

            if &computed != ts {
                // Computed and actual total supplies don't match.
                return Err(CoreError::InvariantViolation(format!(
                    "computed and actual total supplies don't match (computed={computed}, actual={ts})",
                )));
            }
        }

        // There should be no remaining denominations in the computed supplies,
        // because that would mean that accounts have denominations that don't
        // appear in the total supplies table, which would obviously be wrong.
        if computed_ts.is_empty() {
            Ok(())
        } else {
            Err(CoreError::InvariantViolation(
                "encountered denomination that isn't present in total supplies table".to_string(),
            ))
        }
    }
}
