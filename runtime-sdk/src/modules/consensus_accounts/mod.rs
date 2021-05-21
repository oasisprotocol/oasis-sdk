//! Consensus accounts module.
//!
//! This module allows consensus transfers in and out of the runtime account,
//! while keeping track of amount deposited per account.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::{common::cbor, consensus::staking::Account as ConsensusAccount};

use crate::{
    context::{Context, TxContext},
    error::{self, Error as _},
    module,
    module::Module as _,
    modules,
    modules::core::{Module as Core, API as _},
    types::{
        address::Address,
        message::{MessageEvent, MessageEventHookInvocation, MessageResult},
        token,
        transaction::CallResult,
    },
    CheckTxWeight,
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

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
}

/// Gas costs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasCosts {
    #[serde(rename = "tx_deposit")]
    pub tx_deposit: u64,
    #[serde(rename = "tx_withdraw")]
    pub tx_withdraw: u64,
}

/// Parameters for the consensus module.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "gas_costs")]
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Events emitted by the consensus module (none so far).
#[derive(Debug, Serialize, Deserialize, oasis_runtime_sdk_macros::Event)]
#[serde(untagged)]
pub enum Event {}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,
}

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
            Core::add_weight(ctx, CheckTxWeight::ConsensusMessages, 1)?;
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
            Core::add_weight(ctx, CheckTxWeight::ConsensusMessages, 1)?;
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
    fn tx_deposit<C: TxContext>(ctx: &mut C, body: types::Deposit) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        Core::use_tx_gas(ctx, params.gas_costs.tx_deposit)?;

        let signer = &ctx.tx_auth_info().signer_info[0];
        Consensus::ensure_compatible_tx_signer(ctx)?;

        let address = signer.address_spec.address();
        Self::deposit(ctx, address, body.amount)
    }

    /// Withdraw from the runtime.
    fn tx_withdraw<C: TxContext>(ctx: &mut C, body: types::Withdraw) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        Core::use_tx_gas(ctx, params.gas_costs.tx_withdraw)?;

        // Signer.
        let signer = &ctx.tx_auth_info().signer_info[0];
        Consensus::ensure_compatible_tx_signer(ctx)?;

        let address = signer.address_spec.address();
        Self::withdraw(ctx, address, body.amount)
    }

    fn query_balance<C: Context>(
        ctx: &mut C,
        args: types::BalanceQuery,
    ) -> Result<types::AccountBalance, Error> {
        let denomination = Consensus::consensus_denomination(ctx)?;
        let balances = Accounts::get_balances(ctx.runtime_state(), args.address)
            .map_err(|_| Error::InvalidArgument)?;
        let balance = balances
            .balances
            .get(&denomination)
            .ok_or(Error::InvalidArgument)?;
        Ok(types::AccountBalance {
            balance: balance.clone(),
        })
    }

    fn query_consensus_account<C: Context>(
        ctx: &mut C,
        args: types::ConsensusAccountQuery,
    ) -> Result<ConsensusAccount, Error> {
        Consensus::account(ctx, args.address).map_err(|_| Error::InvalidArgument)
    }

    fn message_result_transfer<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusTransferContext,
    ) {
        if !me.is_success() {
            // Transfer out failed.
            return;
        }

        // Update runtime state.
        Accounts::burn(ctx, context.address, &context.amount).expect("should have enough balance");
    }

    fn message_result_withdraw<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusWithdrawContext,
    ) {
        if !me.is_success() {
            // Transfer out failed.
            return;
        }

        // Update runtime state.
        Accounts::mint(ctx, context.address, &context.amount).unwrap();
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::Module
    for Module<Accounts, Consensus>
{
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

/// Module methods.
impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::MethodHandler
    for Module<Accounts, Consensus>
{
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "consensus.Deposit" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_deposit(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "consensus.Withdraw" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_withdraw(ctx, args)?))
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
            "consensus.Balance" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_balance(ctx, args)?))
            })()),
            "consensus.Account" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_consensus_account(ctx, args)?))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }

    fn dispatch_message_result<C: Context>(
        ctx: &mut C,
        handler_name: &str,
        result: MessageResult,
    ) -> module::DispatchResult<MessageResult, ()> {
        match handler_name {
            CONSENSUS_TRANSFER_HANDLER => {
                Self::message_result_transfer(
                    ctx,
                    result.event,
                    cbor::from_value(result.context).expect("invalid message handler context"),
                );
                module::DispatchResult::Handled(())
            }
            CONSENSUS_WITHDRAW_HANDLER => {
                Self::message_result_withdraw(
                    ctx,
                    result.event,
                    cbor::from_value(result.context).expect("invalid message handler context"),
                );
                module::DispatchResult::Handled(())
            }
            _ => module::DispatchResult::Unhandled(result),
        }
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::MigrationHandler
    for Module<Accounts, Consensus>
{
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            // Set genesis parameters.
            Self::set_params(ctx.runtime_state(), &genesis.parameters);
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
