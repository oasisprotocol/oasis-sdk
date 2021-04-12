//! Tests for the accounts module.
use std::collections::BTreeMap;

use oasis_core_runtime::common::cbor;

use crate::{
    context::{Context, DispatchContext},
    module::{AuthHandler, BlockHandler},
    modules::core,
    testing::{keys, mock},
    types::{
        token::{BaseUnits, Denomination},
        transaction,
    },
};

use super::{
    types::*, Error, Genesis, Module as Accounts, Parameters, ADDRESS_COMMON_POOL,
    ADDRESS_FEE_ACCUMULATOR, API as _,
};

#[test]
#[should_panic]
fn test_init_incorrect_total_supply_1() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(
        &mut ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            ..Default::default()
        },
    );
}

#[test]
#[should_panic]
fn test_init_incorrect_total_supply_2() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(
        &mut ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                // Bob.
                balances.insert(keys::bob::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
}

#[test]
fn test_init_1() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(
        &mut ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
}

#[test]
fn test_init_2() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(
        &mut ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                // Bob.
                balances.insert(keys::bob::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 2_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
}

#[test]
fn test_api_tx_transfer_disabled() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(
        &mut ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000.into());
                total_supplies
            },
            parameters: Parameters {
                transfers_disabled: true,
            },
            ..Default::default()
        },
    );

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
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

    // Try to transfer.
    ctx.with_tx(tx, |mut tx_ctx, call| {
        assert_eq!(
            Err(Error::Forbidden),
            Accounts::tx_transfer(&mut tx_ctx, cbor::from_value(call.body).unwrap()),
            "transfers are forbidden",
        );
    });
}

pub(crate) fn init_accounts(ctx: &mut DispatchContext<'_>) {
    Accounts::init(
        ctx,
        &Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Alice.
                balances.insert(keys::alice::address(), {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000.into());
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000.into());
                total_supplies
            },
            ..Default::default()
        },
    );
}

#[test]
fn test_api_transfer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    // Transfer tokens from one account to the other and check balances.
    ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
        Accounts::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            keys::bob::address(),
            &BaseUnits::new(1_000.into(), Denomination::NATIVE),
        )
        .expect("transfer should succeed");

        let result = Accounts::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            keys::bob::address(),
            &BaseUnits::new(1_000_000.into(), Denomination::NATIVE),
        );
        assert!(matches!(result, Err(Error::InsufficientBalance)));

        // Check source account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::alice::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            999_000.into(),
            "balance in source account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check destination account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::bob::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            1_000.into(),
            "balance in destination account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );
    });
}

#[test]
fn test_authenticate_tx() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let mut tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
                amount: BaseUnits::new(1_000.into(), Denomination::NATIVE),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: BaseUnits::new(1_000.into(), Denomination::NATIVE),
                gas: 1000,
            },
        },
    };

    // Should succeed with enough funds to pay for fees.
    Accounts::authenticate_tx(&mut ctx, &tx).expect("transaction authentication should succeed");
    // Check source account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        999_000.into(),
        "fees should be subtracted from source account"
    );
    assert_eq!(
        bals.balances.len(),
        1,
        "there should only be one denomination"
    );
    // Check source account nonce.
    let nonce = Accounts::get_nonce(ctx.runtime_state(), keys::alice::address())
        .expect("get_nonce should succeed");
    assert_eq!(nonce, 1, "nonce should be incremented");

    // Should fail with an invalid nonce.
    let result = Accounts::authenticate_tx(&mut ctx, &tx);
    assert!(matches!(result, Err(core::Error::InvalidNonce)));

    // Should fail when there's not enough balance to pay fees.
    tx.auth_info.signer_info[0].nonce = nonce;
    tx.auth_info.fee.amount = BaseUnits::new(1_100_000.into(), Denomination::NATIVE);
    let result = Accounts::authenticate_tx(&mut ctx, &tx);
    assert!(matches!(result, Err(core::Error::InsufficientFeeBalance)));
}

#[test]
fn test_tx_transfer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
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

    // Transfer tokens from one account to the other and check balances.
    ctx.with_tx(tx, |mut tx_ctx, call| {
        Accounts::tx_transfer(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("transfer should succeed");

        // Check source account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::alice::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            999_000.into(),
            "balance in source account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check destination account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::bob::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            1_000.into(),
            "balance in destination account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );
    });
}

#[test]
fn test_fee_disbursement() {
    let mut mock = mock::Mock::default();

    // Configure some good entities so they get the fees.
    mock.runtime_round_results.good_compute_entities = vec![
        keys::bob::pk_ed25519().into(),
        keys::charlie::pk_ed25519().into(),
    ];

    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
                amount: Default::default(),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                // Use an amount that does not split nicely among the good compute entities.
                amount: BaseUnits::new(1_001.into(), Denomination::NATIVE),
                gas: 1000,
            },
        },
    };

    // Authenticate transaction, fees should be moved to accumulator.
    Accounts::authenticate_tx(&mut ctx, &tx).expect("transaction authentication should succeed");
    // Run end block handler.
    Accounts::end_block(&mut ctx);

    // Check source account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        998_999.into(),
        "fees should be subtracted from source account"
    );
    // Nothing should be disbursed yet to good compute entity accounts.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::bob::address())
        .expect("get_balances should succeed");
    assert!(bals.balances.is_empty(), "nothing should be disbursed yet");
    // Check second good compute entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::charlie::address())
        .expect("get_balances should succeed");
    assert!(bals.balances.is_empty(), "nothing should be disbursed yet");

    // Fees should be placed in the fee accumulator address to be disbursed in the next round.
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_FEE_ACCUMULATOR)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1001.into(),
        "fees should be held in the fee accumulator address"
    );

    // Simulate another block happening.
    Accounts::end_block(&mut ctx);

    // Check first good compute entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::bob::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        500.into(),
        "fees should be disbursed to good compute entity"
    );
    // Check second good compute entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::charlie::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        500.into(),
        "fees should be disbursed to good compute entity"
    );

    // Check the common pool which should have the remainder.
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_COMMON_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1.into(),
        "remainder should be disbursed to the common pool"
    );
}
