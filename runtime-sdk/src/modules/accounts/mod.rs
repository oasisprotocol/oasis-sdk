//! Accounts module.
use std::{collections::BTreeMap, iter::FromIterator};

use lazy_static::lazy_static;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    context::{DispatchContext, TxContext},
    crypto::signature::PublicKey,
    error::{self, Error as _},
    module,
    module::{CallableMethodInfo, Module as _, QueryMethodInfo},
    modules, storage,
    types::{
        address::Address,
        token,
        transaction::{CallResult, Transaction},
    },
};

#[cfg(test)]
mod test;
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
}

/// Events emitted by the accounts module.
#[derive(Debug, Serialize, Deserialize, oasis_runtime_sdk_macros::Event)]
#[serde(untagged)]
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

/// Parameters for the accounts module.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "transfers_disabled")]
    pub transfers_disabled: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            transfers_disabled: false,
        }
    }
}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,

    #[serde(rename = "accounts")]
    pub accounts: BTreeMap<Address, types::Account>,

    #[serde(rename = "balances")]
    pub balances: BTreeMap<Address, BTreeMap<token::Denomination, token::Quantity>>,

    #[serde(rename = "total_supplies")]
    pub total_supplies: BTreeMap<token::Denomination, token::Quantity>,
}

impl Default for Genesis {
    fn default() -> Self {
        Self {
            parameters: Default::default(),
            accounts: BTreeMap::new(),
            balances: BTreeMap::new(),
            total_supplies: BTreeMap::new(),
        }
    }
}

// TODO: Add a custom macro for easier module derivation.

/*
module!{
    #[module(name = MODULE_NAME)]
    impl Module {
        type Error = Error;
        type Event = Event;

        #[module::callable_method(name = "Transfer")]
        fn tx_transfer(ctx: &mut Context, body: u64) -> Result<(), Error> {
            //
            Ok(())
        }

        #[module::api]
        fn transfer(ctx: &mut Context, body: u64) -> Result<(), Error> {
            //
            Ok(())
        }

        #[module::api]
        fn mint(ctx: &mut Context, msg: messages::Mint) -> Result<(), Error> {
            //
            Ok(())
        }
    }
}
*/

/// Interface that can be called from other modules.
pub trait API {
    /// Transfer an amount from one account to the other.
    fn transfer(
        ctx: &mut TxContext<'_, '_>,
        from: Address,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error>;

    /// Mint new tokens, increasing the total supply.
    fn mint(
        ctx: &mut TxContext<'_, '_>,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error>;

    /// Burn existing tokens, decreasing the total supply.
    fn burn(
        ctx: &mut TxContext<'_, '_>,
        from: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error>;

    /// Fetch an account's current nonce.
    fn get_nonce<S: storage::Store>(state: S, address: Address) -> Result<u64, Error>;

    /// Fetch an account's current balances.
    fn get_balances<S: storage::Store>(
        state: S,
        address: Address,
    ) -> Result<types::AccountBalances, Error>;
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

lazy_static! {
    /// Module's address that has the common pool.
    pub static ref ADDRESS_COMMON_POOL: Address = Address::from_module(MODULE_NAME, "common-pool");
    /// Module's address that has the fee accumulator.
    pub static ref ADDRESS_FEE_ACCUMULATOR: Address = Address::from_module(MODULE_NAME, "fee-accumulator");
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
        let mut value: token::Quantity = account.get(amount.denomination()).unwrap_or_default();
        value += amount.amount();

        account.insert(amount.denomination(), &value);
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
        let mut value: token::Quantity = account.get(amount.denomination()).unwrap_or_default();

        value = value
            .checked_sub(&amount.amount())
            .ok_or(Error::InsufficientBalance)?;
        account.insert(amount.denomination(), &value);
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
        let mut total_supply: token::Quantity = total_supplies
            .get(amount.denomination())
            .unwrap_or_default();
        total_supply += amount.amount();
        total_supplies.insert(amount.denomination(), &total_supply);
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
        let mut total_supply: token::Quantity = total_supplies
            .get(amount.denomination())
            .unwrap_or_default();
        total_supply = total_supply
            .checked_sub(&amount.amount())
            .ok_or(Error::InsufficientBalance)?;
        total_supplies.insert(amount.denomination(), &total_supply);
        Ok(())
    }
}

impl API for Module {
    fn transfer(
        ctx: &mut TxContext<'_, '_>,
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

    fn mint(
        ctx: &mut TxContext<'_, '_>,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        // Add to destination account.
        Self::add_amount(ctx.runtime_state(), to, amount)?;

        // Increase total supply.
        Self::inc_total_supply(ctx.runtime_state(), amount)?;

        Ok(())
    }

    fn burn(
        ctx: &mut TxContext<'_, '_>,
        from: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        // Remove from target account.
        Self::sub_amount(ctx.runtime_state(), from, amount)?;

        // Decrease total supply.
        Self::dec_total_supply(ctx.runtime_state(), amount)
            .expect("target account had enough balance so total supply should not underflow");

        Ok(())
    }

    fn get_nonce<S: storage::Store>(state: S, address: Address) -> Result<u64, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let account: types::Account = accounts.get(&address).unwrap_or_default();
        Ok(account.nonce)
    }

    fn get_balances<S: storage::Store>(
        state: S,
        address: Address,
    ) -> Result<types::AccountBalances, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &address));

        Ok(types::AccountBalances {
            balances: BTreeMap::from_iter(account.iter()),
        })
    }
}

impl Module {
    fn tx_transfer(ctx: &mut TxContext<'_, '_>, body: types::Transfer) -> Result<(), Error> {
        // Reject transfers when they are disabled.
        if Self::params(ctx.runtime_state()).transfers_disabled {
            return Err(Error::Forbidden);
        }

        Self::transfer(ctx, ctx.tx_caller_address(), body.to, &body.amount)?;

        Ok(())
    }

    fn query_nonce(ctx: &mut DispatchContext<'_>, args: types::NonceQuery) -> Result<u64, Error> {
        Self::get_nonce(ctx.runtime_state(), args.address)
    }

    fn query_balances(
        ctx: &mut DispatchContext<'_>,
        args: types::BalancesQuery,
    ) -> Result<types::AccountBalances, Error> {
        Self::get_balances(ctx.runtime_state(), args.address)
    }
}

impl Module {
    fn _callable_transfer_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext<'_, '_>,
        body: cbor::Value,
    ) -> CallResult {
        let result = || -> Result<cbor::Value, Error> {
            let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
            Ok(cbor::to_value(&Self::tx_transfer(ctx, args)?))
        }();
        match result {
            Ok(value) => CallResult::Ok(value),
            Err(err) => err.to_call_result(),
        }
    }

    fn _query_nonce_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext<'_>,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_nonce(ctx, args)?))
    }

    fn _query_balances_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext<'_>,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_balances(ctx, args)?))
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl module::MethodRegistrationHandler for Module {
    fn register_methods(methods: &mut module::MethodRegistry) {
        // Callable methods.
        methods.register_callable(module::CallableMethodInfo {
            name: "accounts.Transfer",
            handler: Self::_callable_transfer_handler,
        });

        // Queries.
        methods.register_query(module::QueryMethodInfo {
            name: "accounts.Nonce",
            handler: Self::_query_nonce_handler,
        });
        methods.register_query(module::QueryMethodInfo {
            name: "accounts.Balances",
            handler: Self::_query_balances_handler,
        });
    }
}

impl Module {
    /// Initialize state from genesis.
    fn init(ctx: &mut DispatchContext<'_>, genesis: &Genesis) {
        // Create accounts.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        for (address, account) in genesis.accounts.iter() {
            accounts.insert(address, account);
        }

        // Create balances.
        let mut balances = storage::PrefixStore::new(&mut store, &state::BALANCES);
        let mut computed_total_supply: BTreeMap<token::Denomination, token::Quantity> =
            BTreeMap::new();
        for (address, denominations) in genesis.balances.iter() {
            let mut account =
                storage::TypedStore::new(storage::PrefixStore::new(&mut balances, &address));
            for (denomination, value) in denominations {
                account.insert(denomination, value);

                // Update computed total supply.
                computed_total_supply
                    .entry(denomination.clone())
                    .and_modify(|v| *v += value)
                    .or_insert_with(|| value.clone());
            }
        }

        // Validate and set total supply.
        let mut total_supplies =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::TOTAL_SUPPLY));
        for (denomination, total_supply) in genesis.total_supplies.iter() {
            let computed = computed_total_supply
                .remove(denomination)
                .expect("unexpected total supply");
            if &computed != total_supply {
                panic!(
                    "unexpected total supply (expected: {} got: {})",
                    total_supply, computed
                );
            }

            total_supplies.insert(denomination, total_supply);
        }
        for (denomination, total_supply) in computed_total_supply.iter() {
            panic!(
                "missing expected total supply: {} {}",
                total_supply, denomination
            );
        }

        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), &genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate(_ctx: &mut DispatchContext<'_>, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

impl module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate(
        ctx: &mut DispatchContext<'_>,
        meta: &mut modules::core::types::Metadata,
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

/// A fee accumulator that stores fees from all transactions in a block.
#[derive(Default)]
struct FeeAccumulator {
    total_fees: BTreeMap<token::Denomination, token::Quantity>,
}

impl FeeAccumulator {
    /// Add given fee to the accumulator.
    fn add(&mut self, fee: &token::BaseUnits) {
        let current = self
            .total_fees
            .entry(fee.denomination().clone())
            .or_default();
        *current += fee.amount();
    }
}

/// Context key for the fee accumulator.
const CONTEXT_KEY_FEE_ACCUMULATOR: &str = "accounts.FeeAccumulator";

impl module::AuthHandler for Module {
    fn authenticate_tx(
        ctx: &mut DispatchContext<'_>,
        tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        // Fetch information about each signer.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        let mut payee = None;
        for si in tx.auth_info.signer_info.iter() {
            let address = Address::from_pk(&si.public_key);
            let mut account: types::Account = accounts.get(&address).unwrap_or_default();
            if account.nonce != si.nonce {
                return Err(modules::core::Error::InvalidNonce);
            }

            // First signer pays for the fees.
            if payee.is_none() {
                payee = Some(address);
            }

            // Update nonce.
            // TODO: Could support an option to defer this.
            account.nonce += 1;
            accounts.insert(&address, &account);
        }

        // Charge the specified amount of fees.
        if !tx.auth_info.fee.amount.amount().is_zero() {
            let payee = payee.expect("at least one signer is always present");

            Self::sub_amount(ctx.runtime_state(), payee, &tx.auth_info.fee.amount)
                .map_err(|_| modules::core::Error::InsufficientFeeBalance)?;

            ctx.value::<FeeAccumulator>(CONTEXT_KEY_FEE_ACCUMULATOR)
                .add(&tx.auth_info.fee.amount);

            // TODO: Emit event that fee has been paid.
        }
        Ok(())
    }
}

impl module::BlockHandler for Module {
    fn end_block(ctx: &mut DispatchContext<'_>) {
        // Determine the fees that are available for disbursement from the last block.
        let mut previous_fees = Self::get_balances(ctx.runtime_state(), *ADDRESS_FEE_ACCUMULATOR)
            .expect("get_balances must succeed")
            .balances;

        // Disburse transaction fees to entities controlling all the good nodes in the committee.
        let addrs: Vec<Address> = ctx
            .runtime_round_results()
            .good_compute_entities
            .iter()
            .map(|pk| Address::from_pk(&PublicKey::Ed25519(pk.into())))
            .collect();

        if !addrs.is_empty() {
            let amounts: Vec<_> = previous_fees
                .iter()
                .filter_map(|(denom, fee)| {
                    let fee = fee
                        .checked_div(&token::Quantity::from(addrs.len() as u64))
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

                    Self::add_amount(ctx.runtime_state(), address, &amount)
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
        let acc = ctx.take_value::<FeeAccumulator>(CONTEXT_KEY_FEE_ACCUMULATOR);
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
