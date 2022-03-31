//! Consensus accounts module.
//!
//! This module allows consensus transfers in and out of the runtime account,
//! while keeping track of amount deposited per account.

#[sdk_derive(MethodHandler)]
impl<Accounts: modules::accounts::API, Consensus: modules::consensus::API>
    Module<Accounts, Consensus>
{
    /// Some comment.
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

    /// Some multiline
    /// comment.
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
}
