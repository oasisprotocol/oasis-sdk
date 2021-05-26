use std::{collections::BTreeMap, str::FromStr};

use oasis_core_runtime::consensus::{
    roothash::{Message, StakingMessage},
    staking,
};

use crate::{
    module::MigrationHandler,
    modules::{
        accounts::{Genesis as AccountsGenesis, Module as Accounts, API},
        consensus::Module as Consensus,
        core::types::Metadata,
    },
    testing::{keys, mock},
    types::{
        token::{BaseUnits, Denomination},
        transaction,
    },
};

use super::{
    types::{Deposit, Withdraw},
    Module, *,
};

#[test]
fn test_init() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &genesis);
}

#[test]
fn test_api_deposit_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                amount: BaseUnits::new(1_000.into(), Denomination::NATIVE),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
            },
        },
    };

    ctx.with_tx(tx, |mut tx_ctx, call| {
        assert!(Module::<Accounts, Consensus>::tx_deposit(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .is_err());
    });
}

#[test]
fn test_api_deposit() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                amount: BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap()),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
            },
        },
    };

    ctx.with_tx(tx, |mut tx_ctx, call| {
        Module::<Accounts, Consensus>::tx_deposit(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("deposit tx should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::Withdraw(staking::Withdraw {
                    from: keys::alice::address().into(),
                    amount: 1_000.into(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_WITHDRAW_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        )

        // TODO: support advancing the round. And ensure message is correctly processed.
    });
}

#[test]
fn test_api_withdraw_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                amount: BaseUnits::new(1_000.into(), Denomination::NATIVE),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
            },
        },
    };

    ctx.with_tx(tx, |mut tx_ctx, call| {
        assert!(Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .is_err());
    });
}

#[test]
fn test_api_withdraw_insufficient_balance() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                amount: BaseUnits::new(1_000.into(), Denomination::from_str("TEST").unwrap()),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
            },
        },
    };

    ctx.with_tx(tx, |mut tx_ctx, call| {
        assert!(Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .is_err());
    });
}

#[test]
fn test_api_withdraw() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        &AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &Default::default());

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                amount: BaseUnits::new(1_000_000.into(), denom.clone()),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
            },
        },
    };

    ctx.with_tx(tx, |mut tx_ctx, call| {
        Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("withdraw tx should succeed");

        let (_, msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.first().unwrap();

        assert_eq!(
            &Message::Staking {
                v: 0,
                msg: StakingMessage::Transfer(staking::Transfer {
                    to: keys::alice::address().into(),
                    amount: 1_000_000.into(),
                })
            },
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_TRANSFER_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        // TODO: support advancing the round. And ensure message is correctly processed.
    });
}

#[test]
fn test_consensus_transfer_handler() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        &AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &Default::default());

    // Simulate successful event.
    let me = Default::default();
    let h_ctx = types::ConsensusTransferContext {
        address: keys::alice::address(),
        amount: BaseUnits::new(999_999.into(), denom.clone()),
    };
    Module::<Accounts, Consensus>::message_result_transfer(&mut ctx, me, h_ctx);

    // Ensure runtime balance is updated.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address()).unwrap();
    assert_eq!(
        bals.balances[&denom],
        1.into(),
        "alice balance transferred out"
    )
}

#[test]
fn test_consensus_withdraw_handler() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        &AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, &Default::default());

    // Simulate successful event.
    let me = Default::default();
    let h_ctx = types::ConsensusWithdrawContext {
        address: keys::alice::address(),
        amount: BaseUnits::new(1.into(), denom.clone()),
    };
    Module::<Accounts, Consensus>::message_result_withdraw(&mut ctx, me, h_ctx);

    // Ensure runtime balance is updated.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address()).unwrap();
    assert_eq!(
        bals.balances[&denom],
        1_000_001.into(),
        "alice balance deposited in"
    )
}
