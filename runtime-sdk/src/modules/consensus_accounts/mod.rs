//! Consensus accounts module.
//!
//! This module allows consensus transfers in and out of the runtime account,
//! while keeping track of amount deposited per account.
use std::{collections::BTreeSet, convert::TryInto};

use once_cell::sync::Lazy;
use thiserror::Error;

use oasis_core_runtime::consensus::staking::Account as ConsensusAccount;
use oasis_runtime_sdk_macros::{handler, sdk_derive};

use crate::{
    context::{Context, TxContext},
    error, module,
    module::Module as _,
    modules,
    modules::core::{Error as CoreError, API as _},
    runtime::Runtime,
    storage::Prefix,
    types::{
        address::Address,
        message::{MessageEvent, MessageEventHookInvocation},
        token,
        transaction::AuthInfo,
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

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_deposit: u64,
    pub tx_withdraw: u64,
}

/// Parameters for the consensus module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Events emitted by the consensus accounts module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Deposit {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    },

    #[sdk_event(code = 2)]
    Withdraw {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    },
}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// Interface that can be called from other modules.
pub trait API {
    /// Transfer from consensus staking account to runtime account.
    ///
    /// # Arguments
    ///
    /// * `nonce`: A caller-provided sequence number that will help identify the success/fail events.
    ///   When called from a deposit transaction, we use the signer nonce.
    fn deposit<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    /// Transfer from runtime account to consensus staking account.
    ///
    /// # Arguments
    ///
    /// * `nonce`: A caller-provided sequence number that will help identify the success/fail events.
    ///   When called from a withdraw transaction, we use the signer nonce.
    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
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

/// Module's address that has the tokens pending withdrawal.
pub static ADDRESS_PENDING_WITHDRAWAL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "pending-withdrawal"));

const CONSENSUS_TRANSFER_HANDLER: &str = "consensus.TransferFromRuntime";
const CONSENSUS_WITHDRAW_HANDLER: &str = "consensus.WithdrawIntoRuntime";

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> API
    for Module<Accounts, Consensus>
{
    fn deposit<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
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
                    from,
                    nonce,
                    address: to,
                    amount: amount.clone(),
                },
            ),
        )?;

        Ok(())
    }

    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        // Transfer out of runtime account and update the account state if successful.
        Consensus::transfer(
            ctx,
            to,
            &amount,
            MessageEventHookInvocation::new(
                CONSENSUS_TRANSFER_HANDLER.to_string(),
                types::ConsensusTransferContext {
                    to,
                    nonce,
                    address: from,
                    amount: amount.clone(),
                },
            ),
        )?;

        if ctx.is_check_only() {
            return Ok(());
        }

        // Transfer the given amount to the module's withdrawal account to make sure the tokens
        // remain available until actually withdrawn.
        Accounts::transfer(ctx, from, *ADDRESS_PENDING_WITHDRAWAL, &amount)
            .map_err(|_| Error::InsufficientWithdrawBalance)?;

        Ok(())
    }
}

#[sdk_derive(MethodHandler)]
impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    Module<Accounts, Consensus>
{
    /// Deposit in the runtime.
    #[handler(call = "consensus.Deposit")]
    fn tx_deposit<C: TxContext>(ctx: &mut C, body: types::Deposit) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_deposit)?;

        let signer = &ctx.tx_auth_info().signer_info[0];
        Consensus::ensure_compatible_tx_signer(ctx)?;

        let address = signer.address_spec.address();
        let nonce = signer.nonce;
        Self::deposit(ctx, address, nonce, body.to.unwrap_or(address), body.amount)
    }

    /// Withdraw from the runtime.
    #[handler(prefetch = "consensus.Withdraw")]
    fn prefetch_withdraw(
        add_prefix: &mut dyn FnMut(Prefix),
        _body: cbor::Value,
        auth_info: &AuthInfo,
    ) -> Result<(), error::RuntimeError> {
        // Prefetch withdrawing account balance.
        let addr = auth_info.signer_info[0].address_spec.address();
        add_prefix(Prefix::from(
            [
                modules::accounts::Module::NAME.as_bytes(),
                modules::accounts::state::BALANCES,
                addr.as_ref(),
            ]
            .concat(),
        ));
        Ok(())
    }

    #[handler(call = "consensus.Withdraw")]
    fn tx_withdraw<C: TxContext>(ctx: &mut C, body: types::Withdraw) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_withdraw)?;

        // Signer.
        let signer = &ctx.tx_auth_info().signer_info[0];
        if body.to.is_none() {
            // If no `to` field is specified, i.e. withdrawing to the transaction sender's account,
            // only allow the consensus-compatible single-Ed25519-key signer type. Otherwise, the
            // tokens would get stuck in an account that you can't sign for on the consensus layer.
            Consensus::ensure_compatible_tx_signer(ctx)?;
        }

        let address = signer.address_spec.address();
        let nonce = signer.nonce;
        Self::withdraw(ctx, address, nonce, body.to.unwrap_or(address), body.amount)
    }

    #[handler(query = "consensus.Balance")]
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
            .copied()
            .unwrap_or_default();
        Ok(types::AccountBalance { balance })
    }

    #[handler(query = "consensus.Account")]
    fn query_consensus_account<C: Context>(
        ctx: &mut C,
        args: types::ConsensusAccountQuery,
    ) -> Result<ConsensusAccount, Error> {
        Consensus::account(ctx, args.address).map_err(|_| Error::InvalidArgument)
    }

    #[handler(message_result = CONSENSUS_TRANSFER_HANDLER)]
    fn message_result_transfer<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusTransferContext,
    ) {
        if !me.is_success() {
            // Transfer out failed, refund the balance.
            Accounts::transfer(
                ctx,
                *ADDRESS_PENDING_WITHDRAWAL,
                context.address,
                &context.amount,
            )
            .expect("should have enough balance");

            // Emit withdraw failed event.
            ctx.emit_event(Event::Withdraw {
                from: context.address,
                nonce: context.nonce,
                to: context.to,
                amount: context.amount.clone(),
                error: Some(me.into()),
            });
            return;
        }

        // Burn the withdrawn tokens.
        Accounts::burn(ctx, *ADDRESS_PENDING_WITHDRAWAL, &context.amount)
            .expect("should have enough balance");

        // Emit withdraw successful event.
        ctx.emit_event(Event::Withdraw {
            from: context.address,
            nonce: context.nonce,
            to: context.to,
            amount: context.amount.clone(),
            error: None,
        });
    }

    #[handler(message_result = CONSENSUS_WITHDRAW_HANDLER)]
    fn message_result_withdraw<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusWithdrawContext,
    ) {
        if !me.is_success() {
            // Transfer in failed, emit deposit failed event.
            ctx.emit_event(Event::Deposit {
                from: context.from,
                nonce: context.nonce,
                to: context.address,
                amount: context.amount.clone(),
                error: Some(me.into()),
            });
            return;
        }

        // Update runtime state.
        Accounts::mint(ctx, context.address, &context.amount).unwrap();

        // Emit deposit successful event.
        ctx.emit_event(Event::Deposit {
            from: context.from,
            nonce: context.nonce,
            to: context.address,
            amount: context.amount.clone(),
            error: None,
        });
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

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::MigrationHandler
    for Module<Accounts, Consensus>
{
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
        genesis: Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            // Set genesis parameters.
            Self::set_params(ctx.runtime_state(), genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    module::TransactionHandler for Module<Accounts, Consensus>
{
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    module::IncomingMessageHandler for Module<Accounts, Consensus>
{
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::BlockHandler
    for Module<Accounts, Consensus>
{
}

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::InvariantHandler
    for Module<Accounts, Consensus>
{
    /// Check invariants.
    fn check_invariants<C: Context>(ctx: &mut C) -> Result<(), CoreError> {
        // Total supply of the designated consensus layer token denomination
        // should be less than or equal to the balance of the runtime's general
        // account in the consensus layer.

        let den = Consensus::consensus_denomination(ctx).unwrap();
        #[allow(clippy::or_fun_call)]
        let ts = Accounts::get_total_supplies(ctx.runtime_state()).or(Err(
            CoreError::InvariantViolation("unable to get total supplies".to_string()),
        ))?;

        let rt_addr = Address::from_runtime_id(ctx.runtime_id());
        let rt_acct = Consensus::account(ctx, rt_addr).unwrap_or_default();
        let rt_ga_balance = rt_acct.general.balance;
        let rt_ga_balance: u128 = rt_ga_balance.try_into().unwrap_or(u128::MAX);

        let rt_ga_balance = Consensus::amount_from_consensus(ctx, rt_ga_balance).map_err(|_| {
            CoreError::InvariantViolation(
                "runtime's consensus balance is not representable".to_string(),
            )
        })?;

        match ts.get(&den) {
            Some(total_supply) => {
                if total_supply <= &rt_ga_balance {
                    Ok(())
                } else {
                    Err(CoreError::InvariantViolation(
                        "total supply is greater than runtime's general account balance"
                            .to_string(),
                    ))
                }
            }
            None => Ok(()), // Having no total supply also satisfies above invariant.
        }
    }
}
