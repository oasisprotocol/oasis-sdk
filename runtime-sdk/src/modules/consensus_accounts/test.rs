use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use anyhow::anyhow;
use io_context::Context as IoContext;

use oasis_core_runtime::{
    common::versioned::Versioned,
    consensus::{
        beacon,
        roothash::{Message, StakingMessage},
        staking,
        state::{beacon::MutableState as BeaconMutableState, ConsensusState},
        Event, HEIGHT_LATEST,
    },
    storage::mkvs,
    types::EventKind,
};

use crate::{
    context::BatchContext,
    event::IntoTags,
    history,
    module::{BlockHandler, MethodHandler, MigrationHandler},
    modules::{
        accounts::{Genesis as AccountsGenesis, Module as Accounts, API},
        consensus::{Error as ConsensusError, Module as Consensus},
    },
    testing::{
        keys,
        mock::{self, EmptyRuntime},
    },
    types::{
        address::SignatureAddressSpec,
        token::{BaseUnits, Denomination},
        transaction,
    },
};

use super::{
    types::{Delegate, Deposit, Undelegate, Withdraw},
    Module, *,
};

fn init_accounts_ex<C: BatchContext>(ctx: &mut C, address: Address) {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut meta = Default::default();
    let genesis = Default::default();

    Accounts::init_or_migrate(
        ctx,
        &mut meta,
        AccountsGenesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(address, {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(denom.clone(), 1_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(denom.clone(), 1_000);
                total_supplies
            },
            ..Default::default()
        },
    );
    Module::<Accounts, Consensus>::init_or_migrate(ctx, &mut meta, genesis);
}

fn init_accounts<C: BatchContext>(ctx: &mut C) {
    init_accounts_ex(ctx, keys::alice::address());
}

#[test]
fn test_init() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);
}

#[test]
fn test_api_deposit_invalid_denomination() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

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
    init_accounts(&mut ctx);

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
    init_accounts(&mut ctx);

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

        let mut state = tx_ctx.commit();
        assert_eq!(1, state.messages.len(), "one message should be emitted");
        let (msg, hook) = state.messages.pop().unwrap();

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
    let balance = Accounts::get_balance(test::keys::bob::address(), denom.clone()).unwrap();
    assert_eq!(balance, 1_000u128, "deposited balance should be minted");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 2_000u128,
        "deposited balance should be minted"
    );

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
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
    init_accounts(&mut ctx);

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
    init_accounts(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Withdraw".to_owned(),
            body: cbor::to_value(Withdraw {
                // It's probably more common to withdraw into your own account, but we're using a
                // separate `to` account to make sure everything is hooked up to the right places.
                to: Some(keys::bob::address()),
                amount: BaseUnits::new(5_000, Denomination::from_str("TEST").unwrap()),
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
        assert!(matches!(result, Error::InsufficientBalance));
    });
}

#[test]
fn test_api_withdraw_incompatible_signer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

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
    init_accounts_ex(&mut ctx, signer_address);

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
                amount: BaseUnits::new(1_000, denom.clone()),
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

        let mut state = tx_ctx.commit();
        assert_eq!(1, state.messages.len(), "one message should be emitted");
        let (msg, hook) = state.messages.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::bob::address().into(),
                    amount: 1_000u128.into(),
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
    let balance = Accounts::get_balance(signer_address, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be locked");

    let balance = Accounts::get_balance(*ADDRESS_PENDING_WITHDRAWAL, denom.clone()).unwrap();
    assert_eq!(balance, 1_000u128, "withdrawn balance should be locked");

    // Simulate the message being processed and make sure withdrawal is successfully completed.
    let me = Default::default();
    Module::<Accounts, Consensus>::message_result_transfer(
        &mut ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(*ADDRESS_PENDING_WITHDRAWAL, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be burned");
    let balance = Accounts::get_balance(signer_address, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be burned");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 0u128,
        "withdrawn balance should be burned"
    );

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
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
    assert_eq!(event.amount.amount(), 1_000);
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
    init_accounts(&mut ctx);

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
        Module::<Accounts, Consensus>::tx_withdraw(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("withdraw tx should succeed");

        let mut state = tx_ctx.commit();
        assert_eq!(1, state.messages.len(), "one message should be emitted");
        let (msg, hook) = state.messages.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::Transfer(staking::Transfer {
                    to: keys::bob::address().into(),
                    amount: 1_000u128.into(),
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
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be locked");

    let balance = Accounts::get_balance(*ADDRESS_PENDING_WITHDRAWAL, denom.clone()).unwrap();
    assert_eq!(balance, 1_000u128, "withdrawn balance should be locked");

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
    let balance = Accounts::get_balance(*ADDRESS_PENDING_WITHDRAWAL, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "withdrawn balance should be refunded");
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 1_000u128, "withdrawn balance should be refunded");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 1_000u128,
        "withdrawn balance should be refunded"
    );

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
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
    assert_eq!(event.amount.amount(), 1_000);
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
    init_accounts(&mut ctx);

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
    let bals = Accounts::get_balances(keys::alice::address()).unwrap();
    assert_eq!(bals.balances[&denom], 1_001, "alice balance deposited in")
}

fn perform_delegation<C: BatchContext>(ctx: &mut C, success: bool) -> u64 {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let nonce = 123;
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Delegate".to_owned(),
            body: cbor::to_value(Delegate {
                to: keys::bob::address(),
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
        Module::<Accounts, Consensus>::tx_delegate(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("delegate tx should succeed");

        let mut state = tx_ctx.commit();
        assert_eq!(1, state.messages.len(), "one message should be emitted");
        let (msg, hook) = state.messages.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::AddEscrow(staking::Escrow {
                    account: keys::bob::address().into(),
                    amount: 1_000u128.into(),
                })
            )),
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_DELEGATE_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        hook
    });

    // Make sure that delegated balance is in the module's pending delegations account.
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "delegated balance should be locked");

    let balance = Accounts::get_balance(*ADDRESS_PENDING_DELEGATION, denom.clone()).unwrap();
    assert_eq!(balance, 1_000u128, "delegated balance should be locked");

    // Simulate the message being processed.
    let me = if success {
        MessageEvent {
            module: "staking".to_string(),
            code: 0,
            index: 0,
            result: Some(cbor::to_value(AddEscrowResult {
                owner: Default::default(),
                escrow: keys::bob::address().into(),
                amount: 1_000u128.into(),
                new_shares: 1_000u128.into(),
            })),
        }
    } else {
        MessageEvent {
            module: "staking".to_string(),
            code: 1,
            index: 0,
            result: None,
        }
    };
    Module::<Accounts, Consensus>::message_result_delegate(
        ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    nonce
}

#[test]
fn test_api_delegate() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    let nonce = perform_delegation(&mut ctx, true);

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(*ADDRESS_PENDING_DELEGATION, denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "delegated balance should be burned");
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 0u128, "delegated balance should be burned");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 0u128,
        "delegated balance should be burned"
    );

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(tags.len(), 2, "delegate and burn events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x02"); // accounts.Burn (code = 2) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x03"); // consensus_accounts.Delegate (code = 3) event

    // Decode delegate event.
    #[derive(Debug, Default, cbor::Decode)]
    struct DelegateEvent {
        from: Address,
        nonce: u64,
        to: Address,
        amount: token::BaseUnits,
        #[cbor(optional)]
        error: Option<types::ConsensusError>,
    }
    let mut events: Vec<DelegateEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::bob::address());
    assert_eq!(event.amount.amount(), 1_000);
    assert_eq!(event.amount.denomination(), &denom);
    assert_eq!(event.error, None);

    // Test delegation queries.
    let mut ctx = mock.create_ctx();
    let di = Module::<Accounts, Consensus>::query_delegation(
        &mut ctx,
        types::DelegationQuery {
            from: keys::alice::address(),
            to: keys::bob::address(),
        },
    )
    .expect("delegation query should succeed");
    assert_eq!(di.shares, 1_000);

    let dis = Module::<Accounts, Consensus>::query_delegations(
        &mut ctx,
        types::DelegationsQuery {
            from: keys::alice::address(),
        },
    )
    .expect("delegations query should succeed");
    assert_eq!(dis.len(), 1);
    assert_eq!(dis[0].shares, 1_000);
}

#[test]
fn test_api_delegate_insufficient_balance() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Delegate".to_owned(),
            body: cbor::to_value(Delegate {
                to: keys::bob::address(),
                amount: BaseUnits::new(5_000, denom.clone()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                123,
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
        let result = Module::<Accounts, Consensus>::tx_delegate(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(result, Error::InsufficientBalance));
    });
}

#[test]
fn test_api_delegate_fail() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    perform_delegation(&mut ctx, false);

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(*ADDRESS_PENDING_DELEGATION, denom.clone()).unwrap();
    assert_eq!(
        balance, 0u128,
        "pending delegation balance should be returned on failure"
    );
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(
        balance, 1_000u128,
        "delegated balance should be returned on failure"
    );
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 1_000u128,
        "delegated balance should be returned on failure"
    );

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(tags.len(), 2, "delegate and burn events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x03"); // consensus_accounts.Delegate (code = 3) event
}

fn perform_undelegation<C: BatchContext>(
    ctx: &mut C,
    success: Option<bool>,
) -> (u64, Option<cbor::Value>) {
    let rt_address = Address::from_runtime_id(ctx.runtime_id());
    let nonce = 123;
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Undelegate".to_owned(),
            body: cbor::to_value(Undelegate {
                from: keys::bob::address(),
                shares: 400,
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
        Module::<Accounts, Consensus>::tx_undelegate(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .expect("undelegate tx should succeed");

        let mut state = tx_ctx.commit();
        assert_eq!(1, state.messages.len(), "one message should be emitted");
        let (msg, hook) = state.messages.pop().unwrap();

        assert_eq!(
            Message::Staking(Versioned::new(
                0,
                StakingMessage::ReclaimEscrow(staking::ReclaimEscrow {
                    account: keys::bob::address().into(),
                    shares: 400u128.into(),
                })
            )),
            msg,
            "emitted message should match"
        );

        assert_eq!(
            CONSENSUS_UNDELEGATE_HANDLER.to_string(),
            hook.hook_name,
            "emitted hook should match"
        );

        hook
    });

    // Make sure the delegation was updated to remove shares.
    let di = Module::<Accounts, Consensus>::query_delegation(
        ctx,
        types::DelegationQuery {
            from: keys::alice::address(),
            to: keys::bob::address(),
        },
    )
    .expect("delegation query should succeed");
    assert_eq!(di.shares, 600);

    // Simulate the message being processed.
    let me = match success {
        None => {
            // Return early if we shouldn't process the message.
            return (nonce, Some(hook.payload));
        }
        Some(true) => MessageEvent {
            module: "staking".to_string(),
            code: 0,
            index: 0,
            result: Some(cbor::to_value(ReclaimEscrowResult {
                owner: rt_address.into(),
                escrow: keys::bob::address().into(),
                amount: 400u128.into(),
                remaining_shares: 600u128.into(),
                debonding_shares: 400u128.into(),
                debond_end_time: 14,
            })),
        },
        Some(false) => MessageEvent {
            module: "staking".to_string(),
            code: 1,
            index: 0,
            result: None,
        },
    };
    Module::<Accounts, Consensus>::message_result_undelegate(
        ctx,
        me,
        cbor::from_value(hook.payload).unwrap(),
    );

    (nonce, None)
}

struct MockHistory {
    events: Vec<Event>,
}

impl history::HistoryHost for MockHistory {
    fn consensus_state_at(&self, height: u64) -> Result<ConsensusState, history::Error> {
        match height {
            HEIGHT_LATEST => {
                let io_ctx = Arc::new(IoContext::background());
                let mut mkvs = mkvs::Tree::builder()
                    .with_root_type(mkvs::RootType::State)
                    .build(Box::new(mkvs::sync::NoopReadSyncer));

                BeaconMutableState::set_epoch_state(
                    &mut mkvs,
                    IoContext::create_child(&io_ctx),
                    beacon::EpochTimeState {
                        epoch: 14,
                        height: 50,
                    },
                )
                .unwrap();

                BeaconMutableState::set_future_epoch_state(
                    &mut mkvs,
                    IoContext::create_child(&io_ctx),
                    beacon::EpochTimeState {
                        epoch: 15,
                        height: 70,
                    },
                )
                .unwrap();

                Ok(ConsensusState::new(60, mkvs))
            }
            _ => Err(history::Error::FailedToFetchBlock),
        }
    }

    fn consensus_events_at(
        &self,
        height: u64,
        _kind: EventKind,
    ) -> Result<Vec<Event>, history::Error> {
        match height {
            50 => {
                // We expect the event fetch to be for height 50 as that is the height we
                // reported above as the height for epoch transition for epoch 14.
                Ok(self.events.clone())
            }
            _ => Err(history::Error::FailedToFetchEvents),
        }
    }
}

#[derive(Debug, Default, cbor::Decode)]
struct UndelegateStartEvent {
    from: Address,
    nonce: u64,
    to: Address,
    shares: u128,
    debond_end_time: EpochTime,
    #[cbor(optional)]
    error: Option<types::ConsensusError>,
}

#[derive(Debug, Default, cbor::Decode)]
struct UndelegateDoneEvent {
    from: Address,
    to: Address,
    shares: u128,
    amount: token::BaseUnits,
}

#[test]
fn test_api_undelegate() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    perform_delegation(&mut ctx, true);

    ctx.commit();
    let mut ctx = mock.create_ctx();

    let (nonce, _) = perform_undelegation(&mut ctx, Some(true));
    let rt_address = Address::from_runtime_id(ctx.runtime_id());

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(tags.len(), 1, "undelegate start event should be emitted");
    assert_eq!(tags[0].key, b"consensus_accounts\x00\x00\x00\x04"); // consensus_accounts.UndelegateStart (code = 4) event

    // Decode undelegate start event.
    let mut events: Vec<UndelegateStartEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::bob::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::alice::address());
    assert_eq!(event.shares, 400);
    assert_eq!(event.debond_end_time, 14);
    assert_eq!(event.error, None);

    // Simulate some epoch transitions.
    for epoch in 1..=13 {
        mock.epoch = epoch;

        let mut ctx = mock.create_ctx();
        <EmptyRuntime as Runtime>::Core::begin_block(&mut ctx);
        Module::<Accounts, Consensus>::end_block(&mut ctx);

        // Make sure nothing changes.
        let state = ctx.commit();
        let tags = state.events.into_tags();
        assert_eq!(tags.len(), 0, "no events should be emitted");
    }

    // Do the epoch transition where debonding should happen.
    mock.epoch = 14;
    mock.history = Box::new(MockHistory {
        events: vec![Event::Staking(staking::Event {
            escrow: Some(staking::EscrowEvent::Reclaim {
                owner: rt_address.into(),
                escrow: keys::bob::address().into(),
                amount: 410u128.into(), // Received some rewards.
                shares: 400u128.into(),
            }),
            ..Default::default()
        })],
    });

    let mut ctx = mock.create_ctx();
    <EmptyRuntime as Runtime>::Core::begin_block(&mut ctx);
    Module::<Accounts, Consensus>::end_block(&mut ctx);

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(
        tags.len(),
        2,
        "undelegate done and mint events should be emitted"
    );
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x05"); // consensus_accounts.UndelegateDone (code = 5) event

    // Decode undelegate done event.
    let mut events: Vec<UndelegateDoneEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::bob::address());
    assert_eq!(event.to, keys::alice::address());
    assert_eq!(event.shares, 400);
    assert_eq!(event.amount.amount(), 410);
    assert_eq!(event.amount.denomination(), &denom);

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 410u128, "undelegated balance should be minted");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 410u128,
        "undelegated balance should be minted"
    );
}

#[test]
fn test_api_undelegate_insufficient_balance() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "consensus.Undelegate".to_owned(),
            body: cbor::to_value(Undelegate {
                from: keys::bob::address(),
                shares: 400,
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                123,
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
        let result = Module::<Accounts, Consensus>::tx_undelegate(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(result, Error::InsufficientBalance));
    });
}

#[test]
fn test_api_undelegate_fail() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    perform_delegation(&mut ctx, true);

    ctx.commit();
    let mut ctx = mock.create_ctx();

    let (nonce, _) = perform_undelegation(&mut ctx, Some(false));

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(tags.len(), 1, "undelegate start event should be emitted");
    assert_eq!(tags[0].key, b"consensus_accounts\x00\x00\x00\x04"); // consensus_accounts.UndelegateStart (code = 4) event

    // Decode undelegate start event.
    let mut events: Vec<UndelegateStartEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::bob::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::alice::address());
    assert_eq!(event.shares, 400);
    assert_eq!(event.debond_end_time, 0xffffffffffffffff);
    assert!(event.error.is_some());
}

#[test]
fn test_api_undelegate_suspension() {
    let denom: Denomination = Denomination::from_str("TEST").unwrap();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    init_accounts(&mut ctx);

    perform_delegation(&mut ctx, true);

    ctx.commit();
    let mut ctx = mock.create_ctx();

    // Simulate the following scenario:
    //
    //   * Undelegate submitted in round R.
    //   * Debonding starts in the consensus layer.
    //   * Runtime suspends in round R+1, undelegate results not processed.
    //   * Debonding ends.
    //   * Runtime resumes, undelegate results processed.
    //

    let (nonce, hook_payload) = perform_undelegation(&mut ctx, None); // Do not process undelegation results.
    let rt_address = Address::from_runtime_id(ctx.runtime_id());

    // Make sure no events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert!(tags.is_empty(), "no events should be emitted");

    // Simulate the runtime resuming and processing both undelegate results and the debonding period
    // ending at the same time. Also start in a future epoch.

    mock.epoch = 15;
    mock.history = Box::new(MockHistory {
        events: vec![Event::Staking(staking::Event {
            escrow: Some(staking::EscrowEvent::Reclaim {
                owner: rt_address.into(),
                escrow: keys::bob::address().into(),
                amount: 410u128.into(), // Received some rewards.
                shares: 400u128.into(),
            }),
            ..Default::default()
        })],
    });

    let mut ctx = mock.create_ctx();

    // Process undelegation message result.
    let me = MessageEvent {
        module: "staking".to_string(),
        code: 0,
        index: 0,
        result: Some(cbor::to_value(ReclaimEscrowResult {
            owner: rt_address.into(),
            escrow: keys::bob::address().into(),
            amount: 400u128.into(),
            remaining_shares: 600u128.into(),
            debonding_shares: 400u128.into(),
            debond_end_time: 14,
        })),
    };
    Module::<Accounts, Consensus>::message_result_undelegate(
        &mut ctx,
        me,
        cbor::from_value(hook_payload.unwrap()).unwrap(),
    );

    // Process block.
    <EmptyRuntime as Runtime>::Core::begin_block(&mut ctx);
    Module::<Accounts, Consensus>::end_block(&mut ctx);

    // Make sure events were emitted.
    let state = ctx.commit();
    let tags = state.events.into_tags();
    assert_eq!(
        tags.len(),
        3,
        "undelegate start, done and mint events should be emitted"
    );
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_accounts\x00\x00\x00\x04"); // consensus_accounts.UndelegateStart (code = 4) event
    assert_eq!(tags[2].key, b"consensus_accounts\x00\x00\x00\x05"); // consensus_accounts.UndelegateDone (code = 5) event

    // Decode undelegate start event.
    let mut events: Vec<UndelegateStartEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::bob::address());
    assert_eq!(event.nonce, nonce);
    assert_eq!(event.to, keys::alice::address());
    assert_eq!(event.shares, 400);
    assert_eq!(event.debond_end_time, 14);
    assert_eq!(event.error, None);

    // Decode undelegate done event.
    let mut events: Vec<UndelegateDoneEvent> = cbor::from_slice(&tags[2].value).unwrap();
    assert_eq!(events.len(), 1);
    let event = events.pop().unwrap();
    assert_eq!(event.from, keys::bob::address());
    assert_eq!(event.to, keys::alice::address());
    assert_eq!(event.shares, 400);
    assert_eq!(event.amount.amount(), 410);
    assert_eq!(event.amount.denomination(), &denom);

    // Ensure runtime balance is updated.
    let balance = Accounts::get_balance(keys::alice::address(), denom.clone()).unwrap();
    assert_eq!(balance, 410u128, "undelegated balance should be minted");
    let total_supplies = Accounts::get_total_supplies().unwrap();
    assert_eq!(total_supplies.len(), 1);
    assert_eq!(
        total_supplies[&denom], 410u128,
        "undelegated balance should be minted"
    );
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
