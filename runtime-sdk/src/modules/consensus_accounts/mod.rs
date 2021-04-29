//! Consensus accounts module.
//!
//! This module allows consensus transfers in and out of the runtime account,
//! while keeping track of amount deposited per account.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::{common::cbor, consensus::staking::Account as ConsensusAccount};

use crate::{
    context::{Context, DispatchContext, TxContext},
    error::{self, Error as _},
    module,
    module::{CallableMethodInfo, Module as _, QueryMethodInfo},
    modules,
    types::{
        address::Address,
        message::{MessageEvent, MessageEventHookInvocation},
        token,
        transaction::CallResult,
    },
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "consensus_accounts";

#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("invalid denomination")]
    #[sdk_error(code = 2)]
    InvalidDenomination,

    #[error("withdraw: insufficient runtime balance")]
    #[sdk_error(code = 3)]
    InsufficientWithdrawBalance,

    #[error("consensus: {0}")]
    #[sdk_error(transparent)]
    Consensus(#[from] modules::consensus::Error),
}

/// Events emitted by the consensus module (none so far).
#[derive(Debug, Serialize, Deserialize, oasis_runtime_sdk_macros::Event)]
#[serde(untagged)]
pub enum Event {}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {}

/// Interface that can be called from other modules.
pub trait API {
    /// Deposit an amount into the runtime account.
    fn deposit<C: Context>(
        ctx: &mut C,
        from: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    /// Withdraw an amount out from the runtime account.
    fn withdraw<C: Context>(
        ctx: &mut C,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    // TODO:
    //  - Add/reclaim deposited escrow.
    //      - need a way to get escrow events in runtime: https://github.com/oasisprotocol/oasis-core/issues/3862
}

pub struct Module<Accounts: modules::accounts::API, Consensus: modules::consensus::API> {
    _accounts: std::marker::PhantomData<Accounts>,
    _consensus: std::marker::PhantomData<Consensus>,
}

const CONSENSUS_TRANSFER_HANDLER: &str = "consensus.TransferFromRuntime";
const CONSENSUS_WITHDRAW_HANDLER: &str = "consensus.WithdrawIntoRuntime";

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> API
    for Module<Accounts, Consensus>
{
    fn deposit<C: Context>(
        ctx: &mut C,
        from: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        // XXX: could check consensus state if allowance for the runtime account
        // exists, but consensus state could be outdated since last block, so
        // just try to withdraw.

        // Do withdraw from the consensus account and update the account state if
        // successful.
        Consensus::withdraw(
            ctx,
            from,
            &amount,
            MessageEventHookInvocation::new(
                CONSENSUS_WITHDRAW_HANDLER.to_string(),
                types::ConsensusWithdrawContext {
                    address: from,
                    amount: amount.clone(),
                },
            ),
        )?;

        Ok(())
    }

    fn withdraw<C: Context>(
        ctx: &mut C,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        // Check internal store if account has enough balance to withdraw.
        let balances =
            Accounts::get_balances(ctx.runtime_state(), to).map_err(|_| Error::InvalidArgument)?;
        let balance = balances
            .balances
            .get(amount.denomination())
            .ok_or(Error::InvalidArgument)?;
        if balance < amount.amount() {
            return Err(Error::InsufficientWithdrawBalance);
        }

        // Transfer out of runtime account and update the account state if successful.
        Consensus::transfer(
            ctx,
            to,
            &amount,
            MessageEventHookInvocation::new(
                CONSENSUS_TRANSFER_HANDLER.to_string(),
                types::ConsensusTransferContext {
                    address: to,
                    amount: amount.clone(),
                },
            ),
        )?;

        Ok(())
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    Module<Accounts, Consensus>
{
    /// Deposit in the runtime.
    fn tx_deposit(ctx: &mut TxContext<'_, '_>, body: types::Deposit) -> Result<(), Error> {
        let signer = &ctx
            .tx_auth_info()
            .expect("should be called with a transaction ctx")
            .signer_info[0];
        Consensus::ensure_compatible_tx_signer(ctx)?;

        let address = Address::from_pk(&signer.public_key);
        Self::deposit(ctx, address, body.amount)
    }

    /// Withdraw from the runtime.
    fn tx_withdraw(ctx: &mut TxContext<'_, '_>, body: types::Withdraw) -> Result<(), Error> {
        // Signer.
        let signer = &ctx
            .tx_auth_info()
            .expect("should be called with a transaction context")
            .signer_info[0];
        Consensus::ensure_compatible_tx_signer(ctx)?;

        let address = Address::from_pk(&signer.public_key);
        Self::withdraw(ctx, address, body.amount)
    }

    fn query_balance(
        ctx: &mut DispatchContext<'_>,
        args: types::BalanceQuery,
    ) -> Result<types::AccountBalance, Error> {
        let denomination = Consensus::consensus_denomination(ctx)?;
        let balances = Accounts::get_balances(ctx.runtime_state(), args.addr)
            .map_err(|_| Error::InvalidArgument)?;
        let balance = balances
            .balances
            .get(&denomination)
            .ok_or(Error::InvalidArgument)?;
        Ok(types::AccountBalance {
            balance: balance.clone(),
        })
    }

    fn query_consensus_account(
        ctx: &mut DispatchContext<'_>,
        args: types::ConsensusAccountQuery,
    ) -> Result<ConsensusAccount, Error> {
        Consensus::account(ctx, args.addr).map_err(|_| Error::InvalidArgument)
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    Module<Accounts, Consensus>
{
    fn _callable_deposit_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext<'_, '_>,
        body: cbor::Value,
    ) -> CallResult {
        let result = || -> Result<cbor::Value, Error> {
            let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
            Ok(cbor::to_value(&Self::tx_deposit(ctx, args)?))
        }();
        match result {
            Ok(value) => CallResult::Ok(value),
            Err(err) => err.to_call_result(),
        }
    }

    fn _callable_withdraw_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext<'_, '_>,
        body: cbor::Value,
    ) -> CallResult {
        let result = || -> Result<cbor::Value, Error> {
            let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
            Ok(cbor::to_value(&Self::tx_withdraw(ctx, args)?))
        }();
        match result {
            Ok(value) => CallResult::Ok(value),
            Err(err) => err.to_call_result(),
        }
    }

    fn _query_balance_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext<'_>,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_balance(ctx, args)?))
    }

    fn _query_consensus_account_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext<'_>,
        args: cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError> {
        let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::query_consensus_account(ctx, args)?))
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::Module
    for Module<Accounts, Consensus>
{
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = Event;
    type Parameters = ();
}

/// Module methods.
impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    module::MethodRegistrationHandler for Module<Accounts, Consensus>
{
    fn register_methods(methods: &mut module::MethodRegistry) {
        // Callable methods.
        methods.register_callable(module::CallableMethodInfo {
            name: "consensus.Deposit",
            handler: Self::_callable_deposit_handler,
        });
        methods.register_callable(module::CallableMethodInfo {
            name: "consensus.Withdraw",
            handler: Self::_callable_withdraw_handler,
        });

        // Queries.
        methods.register_query(module::QueryMethodInfo {
            name: "consensus.Balance",
            handler: Self::_query_balance_handler,
        });

        methods.register_query(module::QueryMethodInfo {
            name: "consensus.Account",
            handler: Self::_query_consensus_account_handler,
        })
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    Module<Accounts, Consensus>
{
    fn _consensus_transfer_handler(
        _mi: &module::MessageHandlerInfo,
        ctx: &mut DispatchContext<'_>,
        me: MessageEvent,
        h_ctx: cbor::Value,
    ) {
        let h_ctx: types::ConsensusTransferContext =
            cbor::from_value(h_ctx).expect("invalid message handler context");

        // Transfer out succeed.
        if me.is_success() {
            // Update runtime state.
            Accounts::burn(ctx, h_ctx.address, &h_ctx.amount).expect("should have enough balance");
        }
    }

    fn _consensus_withdraw_handler(
        _mi: &module::MessageHandlerInfo,
        ctx: &mut DispatchContext<'_>,
        me: MessageEvent,
        h_ctx: cbor::Value,
    ) {
        let h_ctx: types::ConsensusWithdrawContext =
            cbor::from_value(h_ctx).expect("invalid message handler context");

        // Deposit in succeed.
        if me.is_success() {
            // Update runtime state.
            Accounts::mint(ctx, h_ctx.address, &h_ctx.amount).unwrap();
        }
    }
}

/// Module message handlers.
impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    module::MessageHookRegistrationHandler for Module<Accounts, Consensus>
{
    // Register message handlers.
    fn register_handlers(handlers: &mut module::MessageHandlerRegistry) {
        handlers.register_handler(module::MessageHandlerInfo {
            name: CONSENSUS_TRANSFER_HANDLER,
            handler: Self::_consensus_transfer_handler,
        });

        handlers.register_handler(module::MessageHandlerInfo {
            name: CONSENSUS_WITHDRAW_HANDLER,
            handler: Self::_consensus_withdraw_handler,
        });
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::MigrationHandler
    for Module<Accounts, Consensus>
{
    type Genesis = Genesis;

    fn init_or_migrate(
        _ctx: &mut DispatchContext<'_>,
        meta: &mut modules::core::types::Metadata,
        _genesis: &Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::AuthHandler
    for Module<Accounts, Consensus>
{
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::BlockHandler
    for Module<Accounts, Consensus>
{
}
