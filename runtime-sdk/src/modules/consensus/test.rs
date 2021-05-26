use std::str::FromStr;

use num_traits::Zero;

use oasis_core_runtime::{
    common::quantity::Quantity,
    consensus::{
        roothash::{Message, StakingMessage},
        staking,
    },
};

use crate::{
    context::{BatchContext, Context},
    modules::consensus::Module as Consensus,
    testing::{keys, mock},
    types::{
        message::MessageEventHookInvocation,
        token::{BaseUnits, Denomination},
    },
};

use super::API as _;

#[test]
fn test_api_transfer_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000.into(), Denomination::NATIVE);

        assert!(Consensus::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            &amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .is_err());
    });
}

#[test]
fn test_api_transfer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap());
        Consensus::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            &amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .expect("transfer should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::Transfer(staking::Transfer {
                    to: keys::alice::address().into(),
                    amount: amount.amount().clone(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            hook_name.to_string(),
            hook.hook_name,
            "emitted hook should match"
        )
    });
}

#[test]
fn test_api_withdraw() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap());
        Consensus::withdraw(
            &mut tx_ctx,
            keys::alice::address(),
            &amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .expect("withdraw should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::Withdraw(staking::Withdraw {
                    from: keys::alice::address().into(),
                    amount: amount.amount().clone(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            hook_name.to_string(),
            hook.hook_name,
            "emitted hook should match"
        )
    });
}

#[test]
fn test_api_escrow() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap());
        Consensus::escrow(
            &mut tx_ctx,
            keys::alice::address(),
            &amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .expect("escrow should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::AddEscrow(staking::Escrow {
                    account: keys::alice::address().into(),
                    amount: amount.amount().clone(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            hook_name.to_string(),
            hook.hook_name,
            "emitted hook should match"
        )
    });
}

#[test]
fn test_api_reclaim_escrow() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap()); // TODO: shares.
        Consensus::reclaim_escrow(
            &mut tx_ctx,
            keys::alice::address(),
            &amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .expect("reclaim escrow should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::ReclaimEscrow(staking::ReclaimEscrow {
                    account: keys::alice::address().into(),
                    shares: amount.amount().clone(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            hook_name.to_string(),
            hook.hook_name,
            "emitted hook should match"
        )
    });
}
#[test]
fn test_api_account() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    // TODO: prepare mock consensus state.

    let acc = Consensus::account(&ctx, keys::alice::address()).expect("query should succeed");
    assert_eq!(
        Quantity::zero(),
        acc.general.balance,
        "consensus balance should be zero"
    )
}
