//! Accounts module.
use std::{collections::BTreeMap, convert::TryInto};

use num_traits::Zero;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    context::{Context, TxContext},
    crypto::signature::PublicKey,
    error::{self, Error as _},
    module,
    module::Module as _,
    modules,
    modules::core::{Error as CoreError, Module as Core, API as _},
    storage,
    types::{
        address::Address,
        token,
        transaction::{CallResult, Transaction},
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

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
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

/// Gas costs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasCosts {
    #[serde(rename = "tx_transfer")]
    pub tx_transfer: u64,
}

/// Parameters for the accounts module.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "transfers_disabled")]
    pub transfers_disabled: bool,
    #[serde(rename = "gas_costs")]
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Genesis state for the accounts module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

    /// Fetch an account's current nonce.
    fn get_nonce<S: storage::Store>(state: S, address: Address) -> Result<u64, Error>;

    /// Fetch an account's current balances.
    fn get_balances<S: storage::Store>(
        state: S,
        address: Address,
    ) -> Result<types::AccountBalances, Error>;

    /// Fetch total supplies.
    fn get_total_supplies<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<token::Denomination, token::Quantity>, Error>;
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

    /// Get all balances.
    fn get_all_balances<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<Address, BTreeMap<token::Denomination, token::Quantity>>, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let balances = storage::TypedStore::new(storage::PrefixStore::new(store, &state::BALANCES));

        // Unfortunately, we can't just return balances.iter().collect() here,
        // because the stored format doesn't match -- we need this workaround
        // instead.

        let balmap: BTreeMap<AddressWithDenomination, token::Quantity> = balances.iter().collect();

        let mut b: BTreeMap<Address, BTreeMap<token::Denomination, token::Quantity>> =
            BTreeMap::new();

        for (addrden, amt) in &balmap {
            let addr = &addrden.0;
            let den = &addrden.1;

            // Fetch existing account's balances or insert blank ones.
            let addr_bals = b.entry(*addr).or_insert_with(BTreeMap::new);

            // Add to given denomination's balance or insert it if new.
            addr_bals
                .entry(den.clone())
                .and_modify(|a| *a += amt)
                .or_insert_with(|| amt.clone());
        }

        Ok(b)
    }
}

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
            balances: account.iter().collect(),
        })
    }

    fn get_total_supplies<S: storage::Store>(
        state: S,
    ) -> Result<BTreeMap<token::Denomination, token::Quantity>, Error> {
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let ts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::TOTAL_SUPPLY));

        Ok(ts.iter().collect())
    }
}

impl Module {
    fn tx_transfer<C: TxContext>(ctx: &mut C, body: types::Transfer) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());

        // Reject transfers when they are disabled.
        if params.transfers_disabled {
            return Err(Error::Forbidden);
        }

        Core::use_tx_gas(ctx, params.gas_costs.tx_transfer)?;

        Self::transfer(ctx, ctx.tx_caller_address(), body.to, &body.amount)?;

        Ok(())
    }

    fn query_nonce<C: Context>(ctx: &mut C, args: types::NonceQuery) -> Result<u64, Error> {
        Self::get_nonce(ctx.runtime_state(), args.address)
    }

    fn query_balances<C: Context>(
        ctx: &mut C,
        args: types::BalancesQuery,
    ) -> Result<types::AccountBalances, Error> {
        Self::get_balances(ctx.runtime_state(), args.address)
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl module::MethodHandler for Module {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "accounts.Transfer" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_transfer(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            _ => module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, Result<cbor::Value, error::RuntimeError>> {
        match method {
            "accounts.Nonce" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_nonce(ctx, args)?))
            })()),
            "accounts.Balances" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_balances(ctx, args)?))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl Module {
    /// Initialize state from genesis.
    fn init<C: Context>(ctx: &mut C, genesis: &Genesis) {
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
    fn authenticate_tx<C: Context>(
        ctx: &mut C,
        tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        // Fetch information about each signer.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::ACCOUNTS));
        let mut payee = None;
        for si in tx.auth_info.signer_info.iter() {
            let address = si.address_spec.address();
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
                .or_default()
                .add(&tx.auth_info.fee.amount);

            // TODO: Emit event that fee has been paid.

            let gas_price = &tx.auth_info.fee.gas_price();
            // Bump transaction priority.
            Core::add_priority(ctx, gas_price.try_into().unwrap_or(u64::MAX))?;
        }
        Ok(())
    }
}

impl module::BlockHandler for Module {
    fn end_block<C: Context>(ctx: &mut C) {
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
        let mut computed_ts: BTreeMap<token::Denomination, token::Quantity> = BTreeMap::new();

        for bals in balances.values() {
            for (den, amt) in bals {
                computed_ts
                    .entry(den.clone())
                    .and_modify(|a| *a += amt)
                    .or_insert_with(|| amt.clone());
            }
        }

        // Now check if the computed and given total supplies match.
        for (den, ts) in &total_supplies {
            // Return error if total supplies have a denomination that we
            // didn't encounter when computing total supplies based on account
            // balances.
            #[allow(clippy::or_fun_call)]
            let computed = computed_ts
                .remove(&den)
                .ok_or(CoreError::InvariantViolation(
                    "unexpected denomination".to_string(),
                ))?;

            if &computed != ts {
                // Computed and actual total supplies don't match.
                return Err(CoreError::InvariantViolation(
                    "computed and actual total supplies don't match".to_string(),
                ));
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
