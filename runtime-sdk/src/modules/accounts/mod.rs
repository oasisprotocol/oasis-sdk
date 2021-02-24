//! Accounts module.
use std::{collections::BTreeMap, iter::FromIterator};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    context::{DispatchContext, TxContext},
    error::{self, Error as _},
    event, module,
    module::{CallableMethodInfo, Module as _, QueryMethodInfo},
    modules, storage,
    types::{
        address::Address,
        token,
        transaction::{CallResult, Transaction},
    },
};

pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "accounts";

// TODO: Add a custom derive macro for easier error derivation (module/error codes).
/// Errors emitted by the accounts module.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("forbidden by policy")]
    Forbidden,
}

impl error::Error for Error {
    fn module(&self) -> &str {
        MODULE_NAME
    }

    fn code(&self) -> u32 {
        match self {
            Error::InvalidArgument => 1,
            Error::InsufficientBalance => 2,
            Error::Forbidden => 3,
        }
    }
}

impl From<Error> for error::RuntimeError {
    fn from(err: Error) -> error::RuntimeError {
        error::RuntimeError::new(err.module(), err.code(), &err.msg())
    }
}

// TODO: Add a custom derive macro for easier event derivation (tags).
/// Events emitted by the accounts module.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    Transfer {
        from: Address,
        to: Address,
        amount: token::BaseUnits,
    },

    Burn {
        owner: Address,
        amount: token::BaseUnits,
    },

    Mint {
        owner: Address,
        amount: token::BaseUnits,
    },
}

impl event::Event for Event {
    fn module(&self) -> &str {
        MODULE_NAME
    }

    fn code(&self) -> u32 {
        match self {
            Event::Transfer { .. } => 1,
            Event::Burn { .. } => 2,
            Event::Mint { .. } => 3,
        }
    }

    fn value(&self) -> cbor::Value {
        cbor::to_value(self)
    }
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
    fn transfer(
        ctx: &mut TxContext,
        from: Address,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error>;

    fn mint(ctx: &mut TxContext, to: Address, amount: &token::BaseUnits) -> Result<(), Error>;

    fn burn(ctx: &mut TxContext, from: Address, amount: &token::BaseUnits) -> Result<(), Error>;
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

impl Module {
    /// Add given amount of tokens to the specified account's balance.
    fn add_amount(
        ctx: &mut TxContext,
        addr: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
        let mut value: token::Quantity = account.get(amount.denomination()).unwrap_or_default();
        value += amount.amount();

        account.insert(amount.denomination(), &value);
        Ok(())
    }

    /// Subtract given amount of tokens from the specified account's balance.
    fn sub_amount(
        ctx: &mut TxContext,
        addr: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(balances, &addr));
        let mut value: token::Quantity = account.get(amount.denomination()).unwrap_or_default();

        value = value
            .checked_sub(&amount.amount())
            .ok_or(Error::InsufficientBalance)?;
        account.insert(amount.denomination(), &value);
        Ok(())
    }
}

impl API for Module {
    fn transfer(
        ctx: &mut TxContext,
        from: Address,
        to: Address,
        amount: &token::BaseUnits,
    ) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        // Subtract from source account.
        Module::sub_amount(ctx, from, amount)?;
        // Add to destination account.
        Module::add_amount(ctx, to, amount)?;

        // Emit a transfer event.
        ctx.emit_event(Event::Transfer {
            from,
            to,
            amount: amount.clone(),
        });

        Ok(())
    }

    fn mint(ctx: &mut TxContext, to: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        // Add to destination account.
        Module::add_amount(ctx, to, amount)?;

        // Increase total supply.
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut total_supplies =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
        let mut total_supply: token::Quantity = total_supplies
            .get(amount.denomination())
            .unwrap_or_default();
        total_supply += amount.amount();
        total_supplies.insert(amount.denomination(), &total_supply);

        Ok(())
    }

    fn burn(ctx: &mut TxContext, from: Address, amount: &token::BaseUnits) -> Result<(), Error> {
        // Remove from target account.
        Module::sub_amount(ctx, from, amount)?;

        // Decrease total supply.
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut total_supplies =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));
        let mut total_supply: token::Quantity = total_supplies
            .get(amount.denomination())
            .unwrap_or_default();
        total_supply = total_supply
            .checked_sub(&amount.amount())
            .expect("target account had enough balance so total supply should not underflow");
        total_supplies.insert(amount.denomination(), &total_supply);

        Ok(())
    }
}

impl Module {
    fn tx_transfer(ctx: &mut TxContext, body: types::Transfer) -> Result<(), Error> {
        // Reject transfers when they are disabled.
        if Self::params(ctx.runtime_state()).transfers_disabled {
            return Err(Error::Forbidden);
        }

        Self::transfer(ctx, ctx.tx_caller_address(), body.to, &body.amount)?;

        Ok(())
    }

    fn query_nonce(ctx: &mut DispatchContext, args: types::NonceQuery) -> Result<u64, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let account: types::Account = accounts.get(&args.address).unwrap_or_default();
        Ok(account.nonce)
    }

    fn query_balances(
        ctx: &mut DispatchContext,
        args: types::BalancesQuery,
    ) -> Result<types::AccountBalances, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let balances = storage::PrefixStore::new(store, &state::BALANCES);
        let account = storage::TypedStore::new(storage::PrefixStore::new(balances, &args.address));

        Ok(types::AccountBalances {
            balances: BTreeMap::from_iter(account.iter()),
        })
    }
}

impl Module {
    fn _callable_transfer_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext,
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
        ctx: &mut DispatchContext,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_nonce(ctx, args)?))
    }

    fn _query_balances_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext,
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
    fn init(ctx: &mut DispatchContext, genesis: &Genesis) {
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
                .get(denomination)
                .expect("unexpected total supply");
            if computed != total_supply {
                panic!(
                    "unexpected total supply (expected: {} got: {})",
                    total_supply, computed
                );
            }

            total_supplies.insert(denomination, total_supply);
        }

        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), &genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate(_ctx: &mut DispatchContext, _from: u32) -> bool {
        // No migrations currently supported.
        false
    }
}

impl module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate(
        ctx: &mut DispatchContext,
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

impl module::AuthHandler for Module {
    fn authenticate_tx(
        ctx: &mut DispatchContext,
        tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        // Fetch information about each signer.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        for si in tx.auth_info.signer_info.iter() {
            let address = Address::from_pk(&si.public_key);
            let mut account: types::Account = accounts.get(&address).unwrap_or_default();
            if account.nonce != si.nonce {
                return Err(modules::core::Error::InvalidNonce);
            }

            // Update nonce.
            // TODO: Could support an option to defer this.
            account.nonce += 1;
            accounts.insert(&address, &account);
        }
        Ok(())
    }
}

impl module::BlockHandler for Module {
}
