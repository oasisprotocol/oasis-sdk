//! Accounts module.
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
};

use num_traits::Zero;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::{
    context::{Context, TxContext},
    core::common::quantity::Quantity,
    handler, module,
    module::{Module as _, Parameters as _},
    modules,
    modules::core::{Error as CoreError, API as _},
    runtime::Runtime,
    sdk_derive, storage,
    storage::Prefix,
    types::{
        address::{Address, SignatureAddressSpec},
        token,
        transaction::{AuthInfo, Transaction},
    },
};

#[cfg(test)]
pub(crate) mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "accounts";

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

#[allow(clippy::trivially_copy_pass_by_ref)]
#[inline]
const fn is_false(v: &bool) -> bool {
    !(*v)
}

/// Parameters for the accounts module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub transfers_disabled: bool,
    pub gas_costs: GasCosts,

    #[cbor(optional)]
    #[cbor(default)]
    #[cbor(skip_serializing_if = "is_false")]
    pub debug_disable_nonce_check: bool,

    #[cbor(optional, default, skip_serializing_if = "BTreeMap::is_empty")]
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
    fn transfer<C: Context>(
        ctx: &mut C,
        from: Address,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error>;

    /// Mint new tokens, increasing the total supply.
    fn mint<C: Context>(ctx: &mut C, to: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    /// Burn existing tokens, decreasing the total supply.
    fn burn<C: Context>(ctx: &mut C, from: Address, amount: &token::BaseUnits)
        -> Result<(), Error>;

    /// Sets an account's nonce.
    fn set_nonce<S: storage::Store>(state: S, address: Address, nonce: u64);

    /// Fetch an account's current nonce.
    fn get_nonce<S: storage::Store>(state: S, address: Address) -> Result<u64, Error>;

    /// Sets an account's balance of the given denomination.
    ///
    /// # Warning
    ///
    /// This method is dangerous as it can result in invariant violations.
    fn set_balance<S: storage::Store>(state: S, address: Address, amount: &token::BaseUnits);

    /// Fetch an account's balance of the given denomination.
    fn get_balance<S: storage::Store>(
        state: S,
        address: Address,
        denomination: token::Denomination,
    ) -> Result<u128, Error>;

    /// Ensures that the given account has at least the specified balance.
    fn ensure_balance<S: storage::Store>(
        state: S,
        address: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let balance = Self::get_balance(state, address, amount.denomination().clone())?;
        if balance < amount.amount() {
            Err(Error::InsufficientBalance)
        } else {
            Ok(())
        }
    }

    /// Fetch an account's current balances.
    fn get_balances<S: storage::Store>(
        state: S,
        address: Address,
    ) -> Result<types::AccountBalances, Error>;

    /// Fetch addresses.
    fn get_addresses<S: storage::Store>(
        state: S,
        denomination: token::Denomination,
    ) -> Result<Vec<Address>, Error>;

    /// Fetch total supplies.
    fn get_total_supplies<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<token::Denomination, u128>, Error>;

    /// Sets the total supply for the given denomination.
    ///
    /// # Warning
    ///
    /// This method is dangerous as it can result in invariant violations.
    fn set_total_supply<S: storage::Store>(state: S, amount: &token::BaseUnits);

    /// Fetch information about a denomination.
    fn get_denomination_info<S: storage::Store>(
        state: S,
        denomination: &token::Denomination,
    ) -> Result<types::DenominationInfo, Error>;

    /// Move amount from address into fee accumulator.
    fn move_into_fee_accumulator<C: Context>(
        ctx: &mut C,
        from: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), modules::core::Error>;

    /// Move amount from fee accumulator into address.
    fn move_from_fee_accumulator<C: Context>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), modules::core::Error>;

    /// Check transaction signer account nonces.
    /// Return payee address.
    fn check_signer_nonces<C: Context>(
        ctx: &mut C,
        tx_auth_info: &AuthInfo,
    ) -> Result<Option<Address>, modules::core::Error>;

    /// Update transaction signer account nonces.
    fn update_signer_nonces<C: Context>(
        ctx: &mut C,
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
}

pub struct Module;

/// Module's address that has the common pool.
pub static ADDRESS_COMMON_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "common-pool"));
/// Module's address that has the fee accumulator.
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
    fn add_amount<S: storage::Store>(
        state: S,
        addr: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
        let mut value: u128 = account.get(amount.denomination()).unwrap_or_default();

        value = value
            .checked_add(amount.amount())
            .ok_or(Error::InvalidArgument)?;
        account.insert(amount.denomination(), value);
        Ok(())
    }

    /// Subtract given amount of tokens from the specified account's balance.
    fn sub_amount<S: storage::Store>(
        state: S,
        addr: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
        let mut value: u128 = account.get(amount.denomination()).unwrap_or_default();

        value = value
            .checked_sub(amount.amount())
            .ok_or(Error::InsufficientBalance)?;
        account.insert(amount.denomination(), value);
        Ok(())
    }

    /// Increment the total supply for the given amount.
    fn inc_total_supply<S: storage::Store>(
        state: S,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
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
    }

    /// Decrement the total supply for the given amount.
    fn dec_total_supply<S: storage::Store>(
        state: S,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
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
    }

    /// Get all balances.
    fn get_all_balances<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<Address, BTreeMap<token::Denomination, u128>>, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::TypedStore::new(storage::PrefixStore::new(store, &state::BALANCES));

        // Unfortunately, we can't just return balances.iter().collect() here,
        // because the stored format doesn't match -- we need this workaround
        // instead.

        let balmap: BTreeMap<AddressWithDenomination, u128> = balances.iter().collect();

        let mut b: BTreeMap<Address, BTreeMap<token::Denomination, u128>> = BTreeMap::new();

        for (addrden, amt) in &balmap {
            let addr = &addrden.0;
            let den = &addrden.1;

            // Fetch existing account's balances or insert blank ones.
            let addr_bals = b.entry(*addr).or_insert_with(BTreeMap::new);

            // Add to given denomination's balance or insert it if new.
            addr_bals
                .entry(den.clone())
                .and_modify(|a| *a += amt)
                .or_insert_with(|| *amt);
        }

        Ok(b)
    }
}

/// A fee accumulator that stores fees from all transactions in a block.
#[derive(Default)]
struct FeeAccumulator {
    total_fees: BTreeMap<token::Denomination, u128>,
}

impl FeeAccumulator {
    /// Add given fee to the accumulator.
    fn add(&mut self, fee: &token::BaseUnits) {
        let current = self
            .total_fees
            .entry(fee.denomination().clone())
            .or_default();

        *current = current.checked_add(fee.amount()).unwrap(); // Should never overflow.
    }

    /// Subtract given fee from the accumulator.
    fn sub(&mut self, fee: &token::BaseUnits) -> Result<(), Error> {
        let current = self
            .total_fees
            .entry(fee.denomination().clone())
            .or_default();

        *current = current
            .checked_sub(fee.amount())
            .ok_or(Error::InsufficientBalance)?;
        Ok(())
    }
}

/// Context key for the fee accumulator.
const CONTEXT_KEY_FEE_ACCUMULATOR: &str = "accounts.FeeAccumulator";

impl API for Module {
    fn transfer<C: Context>(
        ctx: &mut C,
        from: Address,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        // Subtract from source account.
        Self::sub_amount(ctx.runtime_state(), from, amount)?;
        // Add to destination account.
        Self::add_amount(ctx.runtime_state(), to, amount)?;

        // Emit a transfer event.
        ctx.emit_event(Event::Transfer {
            from,
            to,
            amount: amount.clone(),
        });

        Ok(())
    }

    fn mint<C: Context>(ctx: &mut C, to: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        // Add to destination account.
        Self::add_amount(ctx.runtime_state(), to, amount)?;

        // Increase total supply.
        Self::inc_total_supply(ctx.runtime_state(), amount)?;

        // Emit a mint event.
        ctx.emit_event(Event::Mint {
            owner: to,
            amount: amount.clone(),
        });

        Ok(())
    }

    fn burn<C: Context>(
        ctx: &mut C,
        from: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        // Remove from target account.
        Self::sub_amount(ctx.runtime_state(), from, amount)?;

        // Decrease total supply.
        Self::dec_total_supply(ctx.runtime_state(), amount)
            .expect("target account had enough balance so total supply should not underflow");

        // Emit a burn event.
        ctx.emit_event(Event::Burn {
            owner: from,
            amount: amount.clone(),
        });

        Ok(())
    }

    fn set_nonce<S: storage::Store>(state: S, address: Address, nonce: u64) {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let mut account: types::Account = accounts.get(&address).unwrap_or_default();
        account.nonce = nonce;
        accounts.insert(&address, account);
    }

    fn get_nonce<S: storage::Store>(state: S, address: Address) -> Result<u64, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let account: types::Account = accounts.get(&address).unwrap_or_default();
        Ok(account.nonce)
    }

    fn set_balance<S: storage::Store>(state: S, address: Address, amount: &token::BaseUnits) {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));
        account.insert(amount.denomination(), amount.amount());
    }

    fn get_balance<S: storage::Store>(
        state: S,
        address: Address,
        denomination: token::Denomination,
    ) -> Result<u128, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));

        Ok(account.get(&denomination).unwrap_or_default())
    }

    fn get_balances<S: storage::Store>(
        state: S,
        address: Address,
    ) -> Result<types::AccountBalances, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));

        Ok(types::AccountBalances {
            balances: account.iter().collect(),
        })
    }

    fn get_addresses<S: storage::Store>(
        state: S,
        denomination: token::Denomination,
    ) -> Result<Vec<Address>, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances: BTreeMap<AddressWithDenomination, Quantity> =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::BALANCES))
                .iter()
                .collect();

        Ok(balances
            .into_keys()
            .filter(|bal| bal.1 == denomination)
            .map(|bal| bal.0)
            .collect())
    }

    fn get_total_supplies<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<token::Denomination, u128>, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let ts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));

        Ok(ts.iter().collect())
    }

    fn set_total_supply<S: storage::Store>(state: S, amount: &token::BaseUnits) {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let mut total_supplies =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
        total_supplies.insert(amount.denomination(), amount.amount());
    }

    fn get_denomination_info<S: storage::Store>(
        state: S,
        denomination: &token::Denomination,
    ) -> Result<types::DenominationInfo, Error> {
        let params = Self::params(state);
        params
            .denomination_infos
            .get(denomination)
            .cloned()
            .ok_or(Error::NotFound)
    }

    fn move_into_fee_accumulator<C: Context>(
        ctx: &mut C,
        from: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), modules::core::Error> {
        if ctx.is_simulation() {
            return Ok(());
        }

        Self::sub_amount(ctx.runtime_state(), from, amount)
            .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

        ctx.value::<FeeAccumulator>(CONTEXT_KEY_FEE_ACCUMULATOR)
            .or_default()
            .add(amount);

        Ok(())
    }

    fn move_from_fee_accumulator<C: Context>(
        ctx: &mut C,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), modules::core::Error> {
        if ctx.is_simulation() {
            return Ok(());
        }

        ctx.value::<FeeAccumulator>(CONTEXT_KEY_FEE_ACCUMULATOR)
            .or_default()
            .sub(amount)
            .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

        Self::add_amount(ctx.runtime_state(), to, amount)
            .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

        Ok(())
    }

    fn check_signer_nonces<C: Context>(
        ctx: &mut C,
        auth_info: &AuthInfo,
    ) -> Result<Option<Address>, modules::core::Error> {
        // TODO: Optimize the check/update pair so that the accounts are
        // fetched only once.
        let params = Self::params(ctx.runtime_state());
        // Fetch information about each signer.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        let mut payee = None;
        for si in auth_info.signer_info.iter() {
            let address = si.address_spec.address();
            let account: types::Account = accounts.get(&address).unwrap_or_default();
            if account.nonce != si.nonce {
                // Reject unles nonce checking is disabled.
                if !params.debug_disable_nonce_check {
                    return Err(modules::core::Error::InvalidNonce);
                }
            }

            // First signer pays for the fees.
            if payee.is_none() {
                payee = Some(address);
            }
        }
        Ok(payee)
    }

    fn update_signer_nonces<C: Context>(
        ctx: &mut C,
        auth_info: &AuthInfo,
    ) -> Result<(), modules::core::Error> {
        // Fetch information about each signer.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        for si in auth_info.signer_info.iter() {
            let address = si.address_spec.address();
            let mut account: types::Account = accounts.get(&address).unwrap_or_default();

            // Update nonce.
            account.nonce = account
                .nonce
                .checked_add(1)
                .ok_or(modules::core::Error::InvalidNonce)?; // Should never overflow.
            accounts.insert(&address, account);
        }
        Ok(())
    }
}

#[sdk_derive(MethodHandler)]
impl Module {
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
    fn tx_transfer<C: TxContext>(ctx: &mut C, body: types::Transfer) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());

        // Reject transfers when they are disabled.
        if params.transfers_disabled {
            return Err(Error::Forbidden);
        }

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_transfer)?;

        Self::transfer(ctx, ctx.tx_caller_address(), body.to, &body.amount)?;

        Ok(())
    }

    #[handler(query = "accounts.Nonce")]
    fn query_nonce<C: Context>(ctx: &mut C, args: types::NonceQuery) -> Result<u64, Error> {
        Self::get_nonce(ctx.runtime_state(), args.address)
    }

    #[handler(query = "accounts.Addresses", expensive)]
    fn query_addresses<C: Context>(
        ctx: &mut C,
        args: types::AddressesQuery,
    ) -> Result<Vec<Address>, Error> {
        Self::get_addresses(ctx.runtime_state(), args.denomination)
    }

    #[handler(query = "accounts.Balances")]
    fn query_balances<C: Context>(
        ctx: &mut C,
        args: types::BalancesQuery,
    ) -> Result<types::AccountBalances, Error> {
        Self::get_balances(ctx.runtime_state(), args.address)
    }

    #[handler(query = "accounts.DenominationInfo")]
    fn query_denomination_info<C: Context>(
        ctx: &mut C,
        args: types::DenominationInfoQuery,
    ) -> Result<types::DenominationInfo, Error> {
        Self::get_denomination_info(ctx.runtime_state(), &args.denomination)
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl Module {
    /// Initialize state from genesis.
    pub fn init<C: Context>(ctx: &mut C, genesis: Genesis) {
        // Create accounts.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
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
        let mut total_supplies =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::TOTAL_SUPPLY));
        for (denomination, total_supply) in genesis.total_supplies.iter() {
            let computed = computed_total_supply
                .remove(denomination)
                .expect("unexpected total supply");
            assert!(
                &computed == total_supply,
                "unexpected total supply (expected: {} got: {})",
                total_supply,
                computed
            );

            total_supplies.insert(denomination, total_supply);
        }
        for (denomination, total_supply) in computed_total_supply.iter() {
            panic!(
                "missing expected total supply: {} {}",
                total_supply, denomination
            );
        }

        // Validate genesis parameters.
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

impl module::MigrationHandler for Module {
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

impl module::TransactionHandler for Module {
    fn authenticate_tx<C: Context>(
        ctx: &mut C,
        tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        let payee = Self::check_signer_nonces(ctx, &tx.auth_info)?;

        // Charge the specified amount of fees.
        if !tx.auth_info.fee.amount.amount().is_zero() {
            let payee = payee.expect("at least one signer is always present");

            if ctx.is_check_only() {
                // Do not update balances during transaction checks. In case of checks, only do it
                // after all the other checks have already passed as otherwise retrying the
                // transaction will not be possible.
                Self::ensure_balance(ctx.runtime_state(), payee, &tx.auth_info.fee.amount)
                    .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;
            } else {
                // Actually perform the move.
                Self::move_into_fee_accumulator(ctx, payee, &tx.auth_info.fee.amount)?;
            }

            // TODO: Emit event that fee has been paid.

            let gas_price = tx.auth_info.fee.gas_price();
            // Bump transaction priority.
            <C::Runtime as Runtime>::Core::add_priority(
                ctx,
                gas_price.try_into().unwrap_or(u64::MAX),
            )?;
        }

        // Do not update nonces early during transaction checks. In case of checks, only do it after
        // all the other checks have already passed as otherwise retrying the transaction will not
        // be possible.
        if !ctx.is_check_only() {
            Self::update_signer_nonces(ctx, &tx.auth_info)?;
        }

        Ok(())
    }

    fn after_dispatch_tx<C: Context>(
        ctx: &mut C,
        tx_auth_info: &AuthInfo,
        result: &module::CallResult,
    ) {
        if !ctx.is_check_only() {
            // Do nothing outside transaction checks.
            return;
        }
        if !matches!(result, module::CallResult::Ok(_)) {
            // Do nothing in case the call failed to allow retries.
            return;
        }

        // Update payee balance.
        let payee = Self::check_signer_nonces(ctx, tx_auth_info).unwrap(); // Already checked.
        let payee = payee.unwrap(); // Already checked.
        let amount = &tx_auth_info.fee.amount;
        Self::sub_amount(ctx.runtime_state(), payee, amount).unwrap(); // Already checked.

        // Update nonces.
        Self::update_signer_nonces(ctx, tx_auth_info).unwrap();
    }
}

impl module::BlockHandler for Module {
    fn end_block<C: Context>(ctx: &mut C) {
        // Determine the fees that are available for disbursement from the last block.
        let mut previous_fees = Self::get_balances(ctx.runtime_state(), *ADDRESS_FEE_ACCUMULATOR)
            .expect("get_balances must succeed")
            .balances;

        // Drain previous fees from the fee accumulator.
        for (denom, remainder) in &previous_fees {
            Self::sub_amount(
                ctx.runtime_state(),
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

                    Self::add_amount(ctx.runtime_state(), address, amount)
                        .expect("add_amount must succeed for fee disbursement");
                }
            }
        }

        // Transfer remainder to a common pool account.
        for (denom, remainder) in previous_fees.into_iter() {
            Self::add_amount(
                ctx.runtime_state(),
                *ADDRESS_COMMON_POOL,
                &token::BaseUnits::new(remainder, denom),
            )
            .expect("add_amount must succeed for transfer to common pool")
        }

        // Fees for the active block should be transferred to the fee accumulator address.
        let acc = ctx
            .value::<FeeAccumulator>(CONTEXT_KEY_FEE_ACCUMULATOR)
            .take()
            .unwrap_or_default();
        for (denom, amount) in acc.total_fees.into_iter() {
            Self::add_amount(
                ctx.runtime_state(),
                *ADDRESS_FEE_ACCUMULATOR,
                &token::BaseUnits::new(amount, denom),
            )
            .expect("add_amount must succeed for transfer to fee accumulator")
        }
    }
}

impl module::InvariantHandler for Module {
    /// Check invariants.
    fn check_invariants<C: Context>(ctx: &mut C) -> Result<(), CoreError> {
        // All account balances should sum up to the total supply for their
        // corresponding denominations.

        #[allow(clippy::or_fun_call)]
        let balances = Self::get_all_balances(ctx.runtime_state()).or(Err(
            CoreError::InvariantViolation("unable to get balances of all accounts".to_string()),
        ))?;
        #[allow(clippy::or_fun_call)]
        let total_supplies = Self::get_total_supplies(ctx.runtime_state()).or(Err(
            CoreError::InvariantViolation("unable to get total supplies".to_string()),
        ))?;

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
                    "computed and actual total supplies don't match (computed={}, actual={})",
                    computed, ts
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
