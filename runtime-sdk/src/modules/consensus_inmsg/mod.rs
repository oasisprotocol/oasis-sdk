//! Handling incoming messages from the consensus layer.

mod config;
mod events;
#[cfg(test)]
mod test;

use std::convert::TryInto;

use anyhow::anyhow;

use crate::{
    context::{Context, Mode},
    core::consensus::roothash::IncomingMessage,
    dispatcher::{self, DispatchResult},
    error::Error as _,
    module::{CallResult, InMsgHandler, InMsgResult},
    modules::{
        accounts::API as _,
        consensus::API as _,
        core::{Error, API as _},
    },
    runtime::Runtime,
    types::token,
};

pub use config::Config;
pub use events::Event;

/// Unique module name.
const MODULE_NAME: &str = "consensus_inmsg";

/// Incoming message handler.
pub struct InMsgTx<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

impl<Cfg: Config> InMsgHandler for InMsgTx<Cfg> {
    fn process_in_msg<'a, C: Context>(ctx: &mut C, in_msg: &'a IncomingMessage) -> InMsgResult<'a> {
        // Determine whether we should stop processing incoming messages based on remaining gas.
        let base_gas = <C::Runtime as Runtime>::Core::gas_costs(ctx).inmsg_base;
        let max_batch_gas = <C::Runtime as Runtime>::Core::max_batch_gas(ctx);
        let max_inmsg_gas = <C::Runtime as Runtime>::Core::max_inmsg_gas(ctx);
        let remaining_gas = <C::Runtime as Runtime>::Core::remaining_batch_gas(ctx);
        let min_remaining_gas = max_batch_gas
            .saturating_sub(max_inmsg_gas)
            .saturating_add(base_gas);

        if remaining_gas <= min_remaining_gas {
            return InMsgResult::Stop;
        }

        // By default, the address to mint the attached tokens into is the caller specified in the
        // incoming message (authenticated by the consensus layer).
        let mut mint_address = in_msg.caller.clone().into();
        let mut error = None;

        // Capture the result so we make sure to mint the tokens even in case of a bad transaction.
        let mut result = match &in_msg.data[..] {
            &[] => {
                // The incoming message does not contain a transaciton. In this case we only perform
                // the deposit and don't execute anything else.
                InMsgResult::Skip
            }
            raw_tx => {
                // The incoming message contains a transaction. In this case it must be a valid
                // transaction and we execute it. If the transaction is malformed, it is skipped.
                match dispatcher::Dispatcher::<C::Runtime>::decode_tx(ctx, raw_tx) {
                    Err(_) => {
                        error = Some(Error::MalformedTransaction(anyhow!("decoding failed")));
                        InMsgResult::Skip
                    }
                    Ok(tx) => {
                        // In case the transaction is valid, the mint address is the signer of the
                        // contained transaction.
                        mint_address = tx.auth_info.signer_info[0].address_spec.address();

                        // In case the transaction cannot actually fit in the allocated space, skip
                        // as we will never be able to include it.
                        if tx.auth_info.fee.gas > max_inmsg_gas {
                            error = Some(Error::OutOfGas(max_inmsg_gas, tx.auth_info.fee.gas));
                            InMsgResult::Skip
                        } else if tx.auth_info.fee.consensus_messages
                            > Cfg::MAX_CONSENSUS_MSG_SLOTS_PER_TX
                        {
                            error = Some(Error::OutOfMessageSlots);
                            InMsgResult::Skip
                        } else {
                            // If we don't have enough gas remaining to process the inner transaction,
                            // we need to stop processing incoming messages.
                            if tx.auth_info.fee.gas > remaining_gas {
                                return InMsgResult::Stop;
                            }
                            // Same if we don't have enough consensus message slots.
                            if tx.auth_info.fee.consensus_messages > ctx.remaining_messages() {
                                return InMsgResult::Stop;
                            }

                            // We still need to do transaction checks. However those checks may
                            // fail if the minted tokens are not available.
                            //
                            // Given that a failing check can only change the result from Execute
                            // to Skip (but not Stop), this is fine.
                            InMsgResult::Execute(raw_tx, tx)
                        }
                    }
                }
            }
        };

        // Charge base gas for processing an incoming message. If there is not enough gas, stop
        // processing further incoming messages.
        if let Err(_) = <C::Runtime as Runtime>::Core::use_batch_gas(ctx, base_gas) {
            return InMsgResult::Stop;
        }

        // Mint tokens into the given address.
        let amount_fee =
            Cfg::Consensus::amount_from_consensus(ctx, in_msg.fee.clone().try_into().unwrap())
                .unwrap();
        let amount_deposit =
            Cfg::Consensus::amount_from_consensus(ctx, in_msg.tokens.clone().try_into().unwrap())
                .unwrap();
        let denomination = Cfg::Consensus::consensus_denomination(ctx).unwrap();
        Cfg::Accounts::mint(
            ctx,
            mint_address,
            &token::BaseUnits::new(amount_fee + amount_deposit, denomination.clone()),
        )
        .unwrap();

        // Move fee into the accumulator.
        Cfg::Accounts::move_into_fee_accumulator(
            ctx,
            mint_address,
            &token::BaseUnits::new(amount_fee, denomination),
        )
        .unwrap();

        // Perform transaction checks before deciding to execute the transaction. In case the check
        // fails we do not bother executing the transaction and just skip it.
        //
        // This also takes care of potential duplicate transactions.
        if let InMsgResult::Execute(raw_tx, tx) = result {
            let check_result = ctx.with_child(Mode::CheckTx, |mut ctx| {
                dispatcher::Dispatcher::<C::Runtime>::dispatch_tx(
                    &mut ctx,
                    raw_tx.len().try_into().unwrap(),
                    tx.clone(),
                    0,
                )
            });
            result = match check_result {
                Err(err) => {
                    error = Some(Error::TxCheckFailed(err.into_serializable()));
                    InMsgResult::Skip
                }
                Ok(DispatchResult {
                    result: result @ CallResult::Failed { .. },
                    ..
                }) => {
                    error = Some(Error::TxCheckFailed(result.into()));
                    InMsgResult::Skip
                }
                _ => InMsgResult::Execute(raw_tx, tx),
            };
        }

        // Emit incoming message processed event.
        ctx.emit_event(Event::Processed {
            id: in_msg.id,
            tag: in_msg.tag,
            error: error.map(Error::into_serializable),
        });

        result
    }
}
