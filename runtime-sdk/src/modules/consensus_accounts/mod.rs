//! Consensus accounts module.
//!
//! This module allows consensus transfers in and out of the runtime account,
//! while keeping track of amount deposited per account.
use std::{collections::BTreeSet, convert::TryInto, num::NonZeroUsize};

use once_cell::sync::Lazy;
use thiserror::Error;

use oasis_core_runtime::{
    consensus::{
        self,
        beacon::{EpochTime, EPOCH_INVALID},
        staking::{self, Account as ConsensusAccount, AddEscrowResult, ReclaimEscrowResult},
    },
    types::EventKind,
};
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

pub mod state;
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

    #[error("insufficient balance")]
    #[sdk_error(code = 3)]
    InsufficientBalance,

    #[error("forbidden by policy")]
    #[sdk_error(code = 4)]
    Forbidden,

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
    pub tx_delegate: u64,
    pub tx_undelegate: u64,
}

/// Parameters for the consensus module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub gas_costs: GasCosts,

    /// Whether delegate functionality should be disabled.
    pub disable_delegate: bool,
    /// Whether undelegate functionality should be disabled.
    pub disable_undelegate: bool,
    /// Whether deposit functionality should be disabled.
    pub disable_deposit: bool,
    /// Whether withdraw functionality should be disabled.
    pub disable_withdraw: bool,
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

    #[sdk_event(code = 3)]
    Delegate {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    },

    #[sdk_event(code = 4)]
    UndelegateStart {
        from: Address,
        nonce: u64,
        to: Address,
        shares: u128,
        debond_end_time: EpochTime,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    },

    #[sdk_event(code = 5)]
    UndelegateDone {
        from: Address,
        to: Address,
        shares: u128,
        amount: token::BaseUnits,
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

    /// Delegate from runtime account to consensus staking account.
    ///
    /// # Arguments
    ///
    /// * `nonce`: A caller-provided sequence number that will help identify the success/fail events.
    ///   When called from a delegate transaction, we use the signer nonce.
    fn delegate<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    /// Start the undelegation process of the given number of shares from consensus staking account
    /// to runtime account.
    ///
    /// # Arguments
    ///
    /// * `nonce`: A caller-provided sequence number that will help identify the success/fail events.
    ///   When called from an undelegate transaction, we use the signer nonce.
    fn undelegate<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        shares: u128,
    ) -> Result<(), Error>;
}

pub struct Module<Accounts: modules::accounts::API, Consensus: modules::consensus::API> {
    _accounts: std::marker::PhantomData<Accounts>,
    _consensus: std::marker::PhantomData<Consensus>,
}

/// Module's address that has the tokens pending withdrawal.
pub static ADDRESS_PENDING_WITHDRAWAL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "pending-withdrawal"));

/// Module's address that has the tokens pending delegation.
pub static ADDRESS_PENDING_DELEGATION: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "pending-delegation"));

const CONSENSUS_TRANSFER_HANDLER: &str = "consensus.TransferFromRuntime";
const CONSENSUS_WITHDRAW_HANDLER: &str = "consensus.WithdrawIntoRuntime";
const CONSENSUS_DELEGATE_HANDLER: &str = "consensus.Delegate";
const CONSENSUS_UNDELEGATE_HANDLER: &str = "consensus.Undelegate";

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
            .map_err(|_| Error::InsufficientBalance)?;

        Ok(())
    }

    fn delegate<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        Consensus::escrow(
            ctx,
            to,
            &amount,
            MessageEventHookInvocation::new(
                CONSENSUS_DELEGATE_HANDLER.to_string(),
                types::ConsensusDelegateContext {
                    from,
                    nonce,
                    to,
                    amount: amount.clone(),
                },
            ),
        )?;

        if ctx.is_check_only() {
            return Ok(());
        }

        // Transfer the given amount to the module's delegation account to make sure the tokens
        // remain available until actually delegated.
        Accounts::transfer(ctx, from, *ADDRESS_PENDING_DELEGATION, &amount)
            .map_err(|_| Error::InsufficientBalance)?;

        Ok(())
    }

    fn undelegate<C: TxContext>(
        ctx: &mut C,
        from: Address,
        nonce: u64,
        to: Address,
        shares: u128,
    ) -> Result<(), Error> {
        // Subtract shares from delegation, making sure there are enough there.
        state::sub_delegation(to, from, shares)?;

        Consensus::reclaim_escrow(
            ctx,
            from,
            shares,
            MessageEventHookInvocation::new(
                CONSENSUS_UNDELEGATE_HANDLER.to_string(),
                types::ConsensusUndelegateContext {
                    from,
                    nonce,
                    to,
                    shares,
                },
            ),
        )?;

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
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_deposit)?;

        // Check whether deposit is allowed.
        if params.disable_deposit {
            return Err(Error::Forbidden);
        }

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
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_withdraw)?;

        // Check whether withdraw is allowed.
        if params.disable_withdraw {
            return Err(Error::Forbidden);
        }

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

    #[handler(call = "consensus.Delegate")]
    fn tx_delegate<C: TxContext>(ctx: &mut C, body: types::Delegate) -> Result<(), Error> {
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_delegate)?;

        // Check whether delegate is allowed.
        if params.disable_delegate {
            return Err(Error::Forbidden);
        }

        // Signer.
        let signer = &ctx.tx_auth_info().signer_info[0];
        let from = signer.address_spec.address();
        let nonce = signer.nonce;
        Self::delegate(ctx, from, nonce, body.to, body.amount)
    }

    #[handler(call = "consensus.Undelegate")]
    fn tx_undelegate<C: TxContext>(ctx: &mut C, body: types::Undelegate) -> Result<(), Error> {
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_undelegate)?;

        // Check whether undelegate is allowed.
        if params.disable_undelegate {
            return Err(Error::Forbidden);
        }

        // Signer.
        let signer = &ctx.tx_auth_info().signer_info[0];
        let to = signer.address_spec.address();
        let nonce = signer.nonce;
        Self::undelegate(ctx, body.from, nonce, to, body.shares)
    }

    #[handler(query = "consensus.Balance")]
    fn query_balance<C: Context>(
        ctx: &mut C,
        args: types::BalanceQuery,
    ) -> Result<types::AccountBalance, Error> {
        let denomination = Consensus::consensus_denomination(ctx)?;
        let balances = Accounts::get_balances(args.address).map_err(|_| Error::InvalidArgument)?;
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

    #[handler(query = "consensus.Delegation")]
    fn query_delegation<C: Context>(
        _ctx: &mut C,
        args: types::DelegationQuery,
    ) -> Result<types::DelegationInfo, Error> {
        state::get_delegation(args.from, args.to)
    }

    #[handler(query = "consensus.Delegations")]
    fn query_delegations<C: Context>(
        _ctx: &mut C,
        args: types::DelegationsQuery,
    ) -> Result<Vec<types::ExtendedDelegationInfo>, Error> {
        state::get_delegations(args.from)
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

    #[handler(message_result = CONSENSUS_DELEGATE_HANDLER)]
    fn message_result_delegate<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusDelegateContext,
    ) {
        if !me.is_success() {
            // Delegation failed, refund the balance.
            Accounts::transfer(
                ctx,
                *ADDRESS_PENDING_DELEGATION,
                context.from,
                &context.amount,
            )
            .expect("should have enough balance");

            // Emit delegation failed event.
            ctx.emit_event(Event::Delegate {
                from: context.from,
                nonce: context.nonce,
                to: context.to,
                amount: context.amount,
                error: Some(me.into()),
            });
            return;
        }

        // Burn the delegated tokens.
        Accounts::burn(ctx, *ADDRESS_PENDING_DELEGATION, &context.amount)
            .expect("should have enough balance");

        // Record delegation.
        let result = me
            .result
            .expect("event from consensus should have a result");
        let result: AddEscrowResult = cbor::from_value(result).unwrap();
        let shares = result.new_shares.try_into().unwrap();

        state::add_delegation(context.from, context.to, shares).unwrap();

        // Emit delegation successful event.
        ctx.emit_event(Event::Delegate {
            from: context.from,
            nonce: context.nonce,
            to: context.to,
            amount: context.amount,
            error: None,
        });
    }

    #[handler(message_result = CONSENSUS_UNDELEGATE_HANDLER)]
    fn message_result_undelegate<C: Context>(
        ctx: &mut C,
        me: MessageEvent,
        context: types::ConsensusUndelegateContext,
    ) {
        if !me.is_success() {
            // Undelegation failed, add shares back.
            state::add_delegation(context.to, context.from, context.shares).unwrap();

            // Emit undelegation failed event.
            ctx.emit_event(Event::UndelegateStart {
                from: context.from,
                nonce: context.nonce,
                to: context.to,
                shares: context.shares,
                debond_end_time: EPOCH_INVALID,
                error: Some(me.into()),
            });
            return;
        }

        // Queue undelegation processing at the debond end epoch. Further processing will happen in
        // the end block handler.
        let result = me
            .result
            .expect("event from consensus should have a result");
        let result: ReclaimEscrowResult = cbor::from_value(result).unwrap();
        let debonding_shares = result.debonding_shares.try_into().unwrap();

        state::add_undelegation(
            context.from,
            context.to,
            result.debond_end_time,
            debonding_shares,
        )
        .unwrap();

        // Emit undelegation started event.
        ctx.emit_event(Event::UndelegateStart {
            from: context.from,
            nonce: context.nonce,
            to: context.to,
            shares: context.shares,
            debond_end_time: result.debond_end_time,
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
        _ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
        genesis: Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            // Set genesis parameters.
            Self::set_params(genesis.parameters);
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

impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API> module::BlockHandler
    for Module<Accounts, Consensus>
{
    fn end_block<C: Context>(ctx: &mut C) {
        // Only do work in case the epoch has changed since the last processed block.
        if !<C::Runtime as Runtime>::Core::has_epoch_changed(ctx) {
            return;
        }

        let logger = ctx.get_logger("consensus_accounts");
        slog::debug!(logger, "epoch changed, processing queued undelegations";
            "epoch" => ctx.epoch(),
        );

        let mut reclaims: lru::LruCache<(EpochTime, Address), (u128, u128)> =
            lru::LruCache::new(NonZeroUsize::new(128).unwrap());

        let own_address = Address::from_runtime_id(ctx.runtime_id());
        let denomination = Consensus::consensus_denomination(ctx).unwrap();
        let qd = state::get_queued_undelegations(ctx.epoch()).unwrap();
        for ud in qd {
            let udi = state::take_undelegation(&ud).unwrap();

            slog::debug!(logger, "processing undelegation";
                "shares" => udi.shares,
            );

            // Determine total amount the runtime got during the reclaim operation.
            let (total_amount, total_shares) =
                if let Some(totals) = reclaims.get(&(ud.epoch, ud.from)) {
                    *totals
                } else {
                    // Fetch consensus height corresponding to the given epoch transition. This
                    // query may be expensive in case the epoch is far back, but the node is
                    // guaranteed to have it as it was the state after the last normal round
                    // (otherwise we would have already processed this epoch).
                    let height = Consensus::height_for_epoch(ctx, ud.epoch)
                        .expect("failed to determine height for epoch");

                    // Find the relevant reclaim escrow event.
                    //
                    // There will always be exactly one matching reclaim escrow event here, because
                    // debonding delegations get merged at the consensus layer when there are
                    // multiple reclaims for the same accounts on the same epoch.
                    let totals = ctx
                        .history()
                        .consensus_events_at(height, EventKind::Staking)
                        .expect("failed to fetch historic events")
                        .iter()
                        .find_map(|ev| match ev {
                            consensus::Event::Staking(staking::Event {
                                escrow:
                                    Some(staking::EscrowEvent::Reclaim {
                                        owner,
                                        escrow,
                                        amount,
                                        shares,
                                    }),
                                ..
                            }) if owner == &own_address.into() && escrow == &ud.from.into() => {
                                Some((amount.try_into().unwrap(), shares.try_into().unwrap()))
                            }
                            _ => None,
                        })
                        .expect("reclaim event should have been emitted");

                    reclaims.put((ud.epoch, ud.from), totals);
                    totals
                };

            // Compute proportion of received amount (shares * total_amount / total_shares).
            let amount = udi
                .shares
                .checked_mul(total_amount)
                .expect("shares * total_amount should not overflow")
                .checked_div(total_shares)
                .expect("total_shares should not be zero");
            let amount = Consensus::amount_from_consensus(ctx, amount).unwrap();
            let amount = token::BaseUnits::new(amount, denomination.clone());

            // Mint the given number of tokens.
            Accounts::mint(ctx, ud.to, &amount).unwrap();

            // Emit undelegation done event.
            ctx.emit_event(Event::UndelegateDone {
                from: ud.from,
                to: ud.to,
                shares: udi.shares,
                amount,
            });
        }
    }
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
        let ts = Accounts::get_total_supplies().or(Err(CoreError::InvariantViolation(
            "unable to get total supplies".to_string(),
        )))?;

        let rt_addr = Address::from_runtime_id(ctx.runtime_id());
        let rt_acct = Consensus::account(ctx, rt_addr).unwrap_or_default();
        let rt_ga_balance = rt_acct.general.balance;
        let rt_ga_balance: u128 = rt_ga_balance.try_into().unwrap_or(u128::MAX);

        let rt_ga_balance = Consensus::amount_from_consensus(ctx, rt_ga_balance).map_err(|_| {
            CoreError::InvariantViolation(
                "runtime's consensus balance is not representable".to_string(),
            )
        })?;

        if let Some(total_supply) = ts.get(&den) {
            if total_supply > &rt_ga_balance {
                return Err(CoreError::InvariantViolation(
                    "total supply is greater than runtime's general account balance".to_string(),
                ));
            }
        }

        // Check that the number of shares the runtime has escrowed in consensus is >= what is in
        // its internally tracked delegation state.

        let delegations = state::get_delegations_by_destination()
            .map_err(|_| CoreError::InvariantViolation("unable to get delegations".to_string()))?;

        for (to, shares) in delegations {
            let cons_shares = Consensus::delegation(ctx, rt_addr, to)
                .map_err(|err| {
                    CoreError::InvariantViolation(format!(
                        "unable to fetch consensus delegation {rt_addr} -> {to}: {err}"
                    ))
                })?
                .shares;

            if cons_shares < shares.into() {
                return Err(CoreError::InvariantViolation(format!(
                    "runtime does not have enough shares delegated to {to} (expected: {shares} got: {cons_shares}"
                )));
            }
        }

        Ok(())
    }
}
