use std::str::FromStr;

use oasis_core_runtime::{
    common::{quantity::Quantity, versioned::Versioned},
    consensus::{
        roothash::{Message, StakingMessage},
        staking,
    },
};

use crate::{
    context::{BatchContext, Context},
    module::Module as _,
    modules::consensus::Module as Consensus,
    testing::{keys, mock},
    types::{
        message::MessageEventHookInvocation,
        token::{BaseUnits, Denomination},
    },
};

use super::{Genesis, Parameters, API as _};

#[test]
fn test_api_transfer_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::NATIVE);

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

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::alice::address().into(),
                    amount: amount.amount().into(),
                })
            )),
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
fn test_api_transfer_scaling_unrepresentable() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // Everything is multiplied by 1000.
            ..Default::default()
        },
    );

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        // Amount is not representable as it must be in multiples of 1000.
        let amount = BaseUnits::new(500, Denomination::from_str("TEST").unwrap());

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
fn test_api_transfer_scaling() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // Everything is multiplied by 1000.
            ..Default::default()
        },
    );

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::alice::address().into(),
                    // Amount should be properly scaled.
                    amount: 1u128.into(),
                })
            )),
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

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::Withdraw(staking::Withdraw {
                    from: keys::alice::address().into(),
                    amount: amount.amount().into(),
                })
            )),
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
fn test_api_withdraw_scaling() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // Everything is multiplied by 1000.
            ..Default::default()
        },
    );

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::Withdraw(staking::Withdraw {
                    from: keys::alice::address().into(),
                    amount: 1u128.into(),
                })
            )),
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

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::AddEscrow(staking::Escrow {
                    account: keys::alice::address().into(),
                    amount: amount.amount().into(),
                })
            )),
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
fn test_api_escrow_scaling() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // Everything is multiplied by 1000.
            ..Default::default()
        },
    );

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap());
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
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::AddEscrow(staking::Escrow {
                    account: keys::alice::address().into(),
                    amount: 1u128.into(),
                })
            )),
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

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // NOTE: Should be ignored for share amounts.
            ..Default::default()
        },
    );

    ctx.with_tx(0, 0, mock::transaction(), |mut tx_ctx, _call| {
        let hook_name = "test_event_handler";
        let amount = 1_000u128;
        Consensus::reclaim_escrow(
            &mut tx_ctx,
            keys::alice::address(),
            amount,
            MessageEventHookInvocation::new(hook_name.to_string(), 0),
        )
        .expect("reclaim escrow should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking(Versioned::new(
                0,
                StakingMessage::ReclaimEscrow(staking::ReclaimEscrow {
                    account: keys::alice::address().into(),
                    shares: amount.into(),
                })
            )),
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
        Quantity::from(0u128),
        acc.general.balance,
        "consensus balance should be zero"
    )
}

#[test]
fn test_api_scaling() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::set_params(
        ctx.runtime_state(),
        Parameters {
            consensus_scaling_factor: 1_000, // Everything is multiplied by 1000.
            ..Default::default()
        },
    );

    // Not representable.
    Consensus::amount_to_consensus(&mut ctx, 100).unwrap_err();
    Consensus::amount_to_consensus(&mut ctx, 1100).unwrap_err();
    Consensus::amount_to_consensus(&mut ctx, 2500).unwrap_err();
    Consensus::amount_to_consensus(&mut ctx, 2500).unwrap_err();
    Consensus::amount_to_consensus(&mut ctx, 1_000_250).unwrap_err();
    Consensus::amount_to_consensus(&mut ctx, 1_000_001).unwrap_err();
    // Scaling.
    assert_eq!(Consensus::amount_to_consensus(&mut ctx, 0).unwrap(), 0);
    assert_eq!(Consensus::amount_to_consensus(&mut ctx, 1000).unwrap(), 1);
    assert_eq!(Consensus::amount_to_consensus(&mut ctx, 2000).unwrap(), 2);
    assert_eq!(
        Consensus::amount_to_consensus(&mut ctx, 1_000_000).unwrap(),
        1000
    );
    assert_eq!(
        Consensus::amount_to_consensus(&mut ctx, 1_234_000).unwrap(),
        1234
    );
    assert_eq!(Consensus::amount_from_consensus(&mut ctx, 0).unwrap(), 0);
    assert_eq!(Consensus::amount_from_consensus(&mut ctx, 1).unwrap(), 1000);
    assert_eq!(
        Consensus::amount_from_consensus(&mut ctx, 10).unwrap(),
        10_000
    );
    assert_eq!(
        Consensus::amount_from_consensus(&mut ctx, 1000).unwrap(),
        1_000_000
    );
}

#[test]
fn test_query_parameters() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    let params = Parameters {
        consensus_denomination: Denomination::NATIVE,
        consensus_scaling_factor: 1_000,
    };
    Consensus::set_params(ctx.runtime_state(), params.clone());

    let queried_params = Consensus::query_parameters(&mut ctx, ()).unwrap();
    assert_eq!(queried_params, params);
}

#[test]
#[should_panic]
fn test_init_bad_scaling_factor_1() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::init(
        &mut ctx,
        Genesis {
            parameters: Parameters {
                consensus_denomination: Denomination::NATIVE,
                // Zero scaling factor is invalid.
                consensus_scaling_factor: 0,
            },
        },
    );
}

#[test]
#[should_panic]
fn test_init_bad_scaling_factor_2() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Consensus::init(
        &mut ctx,
        Genesis {
            parameters: Parameters {
                consensus_denomination: Denomination::NATIVE,
                // Scaling factor that is not a power of 10 is invalid.
                consensus_scaling_factor: 1230,
            },
        },
    );
}
