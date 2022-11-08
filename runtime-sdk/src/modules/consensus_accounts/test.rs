use std::{collections::BTreeMap, str::FromStr};

use anyhow::anyhow;

use oasis_core_runtime::{
    common::versioned::Versioned,
    consensus::{
        roothash::{Message, StakingMessage},
        staking,
    },
};

use crate::{
    context::BatchContext,
    event::IntoTags,
    module::{MethodHandler, MigrationHandler},
    modules::{
        accounts::{Genesis as AccountsGenesis, Module as Accounts, API},
        consensus::{Error as ConsensusError, Module as Consensus},
        core::types::Metadata,
    },
    testing::{keys, mock},
    types::{
        address::SignatureAddressSpec,
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

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);
}

#[test]
fn test_api_deposit_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Module::<Accounts, Consensus>::tx_deposit(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            result,
            Error::Consensus(ConsensusError::InvalidDenomination)
        ));
    });
}

#[test]
fn test_api_deposit_incompatible_signer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Module::<Accounts, Consensus>::tx_deposit(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            result,
            Error::Consensus(ConsensusError::ConsensusIncompatibleSigner)
        ));
    });
}

#[test]
fn test_api_deposit() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let nonce = 123;
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                // It's probably more common to deposit into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, denom.clone()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                nonce,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    let hook = ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        Module::<Accounts, Consensus>::tx_deposit(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("deposit tx should succeed");

        let (_, mut msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Withdraw(staking::Withdraw {
                    from: keys::alice::address().into(),
                    amount: 1_000u128.into(),
                })
            )),
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_WITHDRAW_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        hook
    });

    // Simulate the message being processed and make sure withdrawal is successfully completed.
    let me = Default::default();
    Module::<Accounts, Consensus>::message_result_withdraw(
        &mut ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(
        ctx.runtime_state(),
        test::keys::bob::address(),
        denom.clone(),
    )
    .unwrap();
    assert_eq!(balance, 1_000u128, "deposited balance should be minted");
    let total_supplies = Accounts::get_total_supplies(ctx.runtime_state()).unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 1_000u128,
        "deposited balance should be minted"
    );

    // Make sure events were emitted.
    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "deposit and mint events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x01"); // consensus_accounts.Deposit (code = 1) event

    // Decode deposit event.
    #[derive(Debug, Default, cbor::Decode)]
    struct DepositEvent {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    }

    let mut events: Vec<DepositEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::bob::address());
    assert_eq!(event.amount.amount(), 1_000);
    assert_eq!(event.amount.denomination(), &denom);
    assert_eq!(event.error, None);
}

#[test]
fn test_api_withdraw_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000);
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            result,
            Error::Consensus(ConsensusError::InvalidDenomination)
        ));
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

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(result, Error::InsufficientWithdrawBalance));
    });
}

#[test]
fn test_api_withdraw_incompatible_signer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };
    let genesis = Default::default();

    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, genesis);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                to: None,
                amount: BaseUnits::new(1_000, Denomination::from_str("TEST").unwrap()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            result,
            Error::Consensus(ConsensusError::ConsensusIncompatibleSigner)
        ));
    });
}

fn test_api_withdraw(signer_sigspec: SignatureAddressSpec) {
    let signer_address = Address::from_sigspec(&signer_sigspec);

    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(signer_address, {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000);
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, Default::default());

    let nonce = 123;
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000_000, denom.clone()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(signer_sigspec, nonce)],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    let hook = ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("withdraw tx should succeed");

        let (_, mut msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::bob::address().into(),
                    amount: 1_000_000u128.into(),
                })
            )),
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_TRANSFER_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        hook
    });

    // Make sure that withdrawn balance is in the module's pending withdrawal account.
    let balance =
        Accounts::get_balance(ctx.runtime_state(), signer_address, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be locked");

    let balance = Accounts::get_balance(
        ctx.runtime_state(),
        *ADDRESS_PENDING_WITHDRAWAL,
        denom.clone(),
    )
    .unwrap();
    assert_eq!(balance, 1_000_000u128, "withdrawn balance should be locked");

    // Simulate the message being processed and make sure withdrawal is successfully completed.
    let me = Default::default();
    Module::<Accounts, Consensus>::message_result_transfer(
        &mut ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(
        ctx.runtime_state(),
        *ADDRESS_PENDING_WITHDRAWAL,
        denom.clone(),
    )
    .unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be burned");
    let balance =
        Accounts::get_balance(ctx.runtime_state(), signer_address, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be burned");
    let total_supplies = Accounts::get_total_supplies(ctx.runtime_state()).unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 0u128,
        "withdrawn balance should be burned"
    );

    // Make sure events were emitted.
    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "withdraw and burn events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x02"); // accounts.Burn (code = 2) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x02"); // consensus_accounts.Withdraw (code = 2) event

    // Decode withdraw event.
    #[derive(Debug, Default, cbor::Decode)]
    struct WithdrawEvent {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    }
    let mut events: Vec<WithdrawEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, signer_address);
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::bob::address());
    assert_eq!(event.amount.amount(), 1_000_000);
    assert_eq!(event.amount.denomination(), &denom);
    assert_eq!(event.error, None);
}

#[test]
fn test_api_withdraw_ed25519() {
    test_api_withdraw(keys::alice::sigspec());
}

#[test]
fn test_api_withdraw_secp256k1() {
    test_api_withdraw(keys::dave::sigspec());
}

#[test]
fn test_api_withdraw_handler_failure() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut meta = Metadata {
        ..Default::default()
    };

    Accounts::init_or_migrate(
        &mut ctx,
        &mut meta,
        AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000);
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, Default::default());

    let nonce = 123;
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: keys::bob::address().into(),
                amount: BaseUnits::new(1_000_000, denom.clone()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                nonce,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000,
                consensus_messages: 1,
            },
            ..Default::default()
        },
    };

    let hook = ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("withdraw tx should succeed");

        let (_, mut msgs) = tx_ctx.commit();
        assert_eq!(1, msgs.len(), "one message should be emitted");
        let (msg, hook) = msgs.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::bob::address().into(),
                    amount: 1_000_000u128.into(),
                })
            )),
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_TRANSFER_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        hook
    });

    // Make sure that withdrawn balance is in the module's pending withdrawal account.
    let balance =
        Accounts::get_balance(ctx.runtime_state(), keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be locked");

    let balance = Accounts::get_balance(
        ctx.runtime_state(),
        *ADDRESS_PENDING_WITHDRAWAL,
        denom.clone(),
    )
    .unwrap();
    assert_eq!(balance, 1_000_000u128, "withdrawn balance should be locked");

    // Simulate the message failing and make sure withdrawal amount is refunded.
    let me = MessageEvent {
        module: "staking".to_string(),
        code: 1, // Any non-zero code is treated as an error.
        index: 0,
        result: None,
    };
    Module::<Accounts, Consensus>::message_result_transfer(
        &mut ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    // Ensure amount is refunded.
    let balance = Accounts::get_balance(
        ctx.runtime_state(),
        *ADDRESS_PENDING_WITHDRAWAL,
        denom.clone(),
    )
    .unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be refunded");
    let balance =
        Accounts::get_balance(ctx.runtime_state(), keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(
        balance, 1_000_000u128,
        "withdrawn balance should be refunded"
    );
    let total_supplies = Accounts::get_total_supplies(ctx.runtime_state()).unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 1_000_000u128,
        "withdrawn balance should be refunded"
    );

    // Make sure events were emitted.
    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(
        tags.len(),
        2,
        "withdraw and transfer events should be emitted"
    );
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x02"); // consensus_accounts.Withdraw (code = 2) event

    // Decode withdraw event.
    #[derive(Debug, Default, cbor::Decode)]
    struct WithdrawEvent {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    }
    let mut events: Vec<WithdrawEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::bob::address());
    assert_eq!(event.amount.amount(), 1_000_000);
    assert_eq!(event.amount.denomination(), &denom);
    assert_eq!(
        event.error,
        Some(types::ConsensusError {
            module: "staking".to_string(),
            code: 1,
        })
    );
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
        AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000_000);
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(&mut ctx, &mut meta, Default::default());

    // Simulate successful event.
    let me = Default::default();
    let h_ctx = types::ConsensusWithdrawContext {
        from: keys::alice::address(),
        nonce: 0,
        address: keys::alice::address(),
        amount: BaseUnits::new(1, denom.clone()),
    };
    Module::<Accounts, Consensus>::message_result_withdraw(&mut ctx, me, h_ctx);

    // Ensure runtime balance is updated.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address()).unwrap();
    assert_eq!(
        bals.balances[&denom], 1_000_001,
        "alice balance deposited in"
    )
}

#[test]
fn test_prefetch() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    let auth_info = transaction::AuthInfo {
        signer_info: vec![transaction::SignerInfo::new_sigspec(
            keys::alice::sigspec(),
            0,
        )],
        fee: transaction::Fee {
            amount: Default::default(),
            gas: 1000,
            consensus_messages: 1,
        },
        ..Default::default()
    };

    // Test withdraw.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
            }),
            ..Default::default()
        },
        auth_info: auth_info.clone(),
    };
    // Withdraw should result in one prefix getting prefetched.
    ctx.with_tx(0, 0, tx, |mut _tx_ctx, call| {
        let mut prefixes = BTreeSet::new();
        let result = Module::<Accounts, Consensus>::prefetch(
            &mut prefixes,
            &call.method,
            call.body,
            &auth_info,
        )
        .ok_or(anyhow!("dispatch failure"))
        .expect("prefetch should succeed");

        assert!(matches!(result, Ok(())));
        assert_eq!(prefixes.len(), 1, "there should be 1 prefix to be fetched");
    });

    // Test deposit.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Deposit".to_owned(),
            body: cbor::to_value(Deposit {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
            }),
            ..Default::default()
        },
        auth_info: auth_info.clone(),
    };
    // Deposit should result in zero prefixes.
    ctx.with_tx(0, 0, tx, |mut _tx_ctx, call| {
        let mut prefixes = BTreeSet::new();
        let result = Module::<Accounts, Consensus>::prefetch(
            &mut prefixes,
            &call.method,
            call.body,
            &auth_info,
        )
        .ok_or(anyhow!("dispatch failure"))
        .expect("prefetch should succeed");

        assert!(matches!(result, Ok(())));
        assert_eq!(
            prefixes.len(),
            0,
            "there should be 0 prefixes to be fetched"
        );
    });
}
