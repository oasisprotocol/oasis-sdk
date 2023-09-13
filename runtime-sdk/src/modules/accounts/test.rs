//! Tests for the accounts module.
use std::{
    collections::{BTreeMap, BTreeSet},
    iter::FromIterator,
};

use anyhow::anyhow;

use crate::{
    context::{self, BatchContext, Context, TxContext},
    handler,
    module::{self, BlockHandler, InvariantHandler, MethodHandler, Module, TransactionHandler},
    modules::{
        core,
        core::{Error as CoreError, Module as Core, API as _},
    },
    sdk_derive, subcall,
    testing::{keys, mock},
    types::{
        address::Address,
        token::{BaseUnits, Denomination},
        transaction,
    },
    Runtime, Version,
};

use super::{
    types::*, Error, GasCosts, Genesis, Module as Accounts, Parameters, ADDRESS_COMMON_POOL,
    ADDRESS_FEE_ACCUMULATOR, API as _,
};

struct CoreConfig;

impl core::Config for CoreConfig {}

/// Test runtime.
struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core<CoreConfig>;

    type Modules = (Core<CoreConfig>, Accounts, TestModule);

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: 10_000_000,
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 0)]),
                    ..Default::default()
                },
            },
            Genesis {
                balances: BTreeMap::from([(
                    keys::alice::address(),
                    BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                )]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                parameters: Parameters {
                    gas_costs: GasCosts { tx_transfer: 1_000 },
                    ..Default::default()
                },
                ..Default::default()
            },
            (), // Test module has no genesis.
        )
    }
}

/// A module with multiple no-op methods; intended for testing routing.
struct TestModule;

#[sdk_derive(Module)]
impl TestModule {
    const NAME: &'static str = "test";
    type Error = CoreError;
    type Event = ();
    type Parameters = ();
    type Genesis = ();

    #[handler(call = "test.RefundFee")]
    fn refund_fee<C: TxContext>(ctx: &mut C, fail: bool) -> Result<(), CoreError> {
        // Use some gas.
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, 10_000)?;
        // Ask the runtime to refund the rest (even on failures).
        Accounts::set_refund_unused_tx_fee(ctx, true);

        if fail {
            Err(CoreError::Forbidden)
        } else {
            Ok(())
        }
    }

    #[handler(call = "test.Subcall")]
    fn subcall<C: TxContext>(ctx: &mut C, _args: ()) -> Result<(), CoreError> {
        // Use some gas.
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, 1_000)?;

        let max_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx);
        let result = subcall::call(
            ctx,
            subcall::SubcallInfo {
                caller: transaction::CallerAddress::Address(Default::default()),
                method: "test.RefundFee".to_string(),
                body: cbor::to_value(false),
                max_depth: 8,
                max_gas,
            },
            subcall::AllowAllValidator,
        )?;

        // Propagate gas use.
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, result.gas_used)?;

        Ok(())
    }
}

impl module::BlockHandler for TestModule {}
impl module::TransactionHandler for TestModule {}
impl module::InvariantHandler for TestModule {}

#[test]
#[should_panic]
fn test_init_incorrect_total_supply_1() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
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
        ..Default::default()
    });
}

#[test]
#[should_panic]
fn test_init_incorrect_total_supply_2() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(keys::alice::address(), {
                let mut denominations = BTreeMap::new();
                denominations.insert(Denomination::NATIVE, 1_000_000);
                denominations
            });
            // Bob.
            balances.insert(keys::bob::address(), {
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
    });
}

#[cfg(feature = "unsafe-allow-debug")]
#[test]
fn test_debug_option_set() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
        parameters: Parameters {
            debug_disable_nonce_check: true,
            ..Default::default()
        },
        ..Default::default()
    });
}

#[cfg(not(feature = "unsafe-allow-debug"))]
#[test]
#[should_panic]
fn test_debug_option_set() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
        parameters: Parameters {
            debug_disable_nonce_check: true,
            ..Default::default()
        },
        ..Default::default()
    });
}

#[test]
fn test_init_1() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
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
    });
}

#[test]
fn test_init_2() {
    let _mock = mock::Mock::default();

    Accounts::init(Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(keys::alice::address(), {
                let mut denominations = BTreeMap::new();
                denominations.insert(Denomination::NATIVE, 1_000_000);
                denominations
            });
            // Bob.
            balances.insert(keys::bob::address(), {
                let mut denominations = BTreeMap::new();
                denominations.insert(Denomination::NATIVE, 1_000_000);
                denominations
            });
            balances
        },
        total_supplies: {
            let mut total_supplies = BTreeMap::new();
            total_supplies.insert(Denomination::NATIVE, 2_000_000);
            total_supplies
        },
        ..Default::default()
    });
}

#[test]
fn test_api_tx_transfer_disabled() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Accounts::init(Genesis {
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
        parameters: Parameters {
            transfers_disabled: true,
            debug_disable_nonce_check: false,
            ..Default::default()
        },
        ..Default::default()
    });

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
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
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };

    // Try to transfer.
    ctx.with_tx(tx.into(), |mut tx_ctx, call| {
        assert!(
            matches!(
                Accounts::tx_transfer(&mut tx_ctx, cbor::from_value(call.body).unwrap()),
                Err(Error::Forbidden),
            ),
            "transfers are forbidden",
        )
    });
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
            consensus_messages: 0,
        },
        ..Default::default()
    };

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
            }),
            ..Default::default()
        },
        auth_info: auth_info.clone(),
    };
    // Transfer tokens from one account to the other and check balances.
    ctx.with_tx(tx.into(), |mut _tx_ctx, call| {
        let mut prefixes = BTreeSet::new();
        let result = Accounts::prefetch(&mut prefixes, &call.method, call.body, &auth_info)
            .ok_or(anyhow!("dispatch failure"))
            .expect("prefetch should succeed");

        assert!(matches!(result, Ok(())));
        assert_eq!(
            prefixes.len(),
            4,
            "there should be 4 prefixes to be fetched"
        );
    });
}

pub(crate) fn init_accounts<C: Context>(_ctx: &mut C) {
    Accounts::init(Genesis {
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
        parameters: Parameters {
            denomination_infos: {
                let mut denomination_infos = BTreeMap::new();
                denomination_infos.insert(Denomination::NATIVE, DenominationInfo { decimals: 9 });
                denomination_infos
            },
            ..Default::default()
        },
        ..Default::default()
    });
}

#[test]
fn test_api_transfer() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    // Transfer tokens from one account to the other and check balances.
    ctx.with_tx(mock::transaction().into(), |mut tx_ctx, _call| {
        Accounts::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            keys::bob::address(),
            &BaseUnits::new(1_000, Denomination::NATIVE),
        )
        .expect("transfer should succeed");

        let result = Accounts::transfer(
            &mut tx_ctx,
            keys::alice::address(),
            keys::bob::address(),
            &BaseUnits::new(1_000_000, Denomination::NATIVE),
        );
        assert!(matches!(result, Err(Error::InsufficientBalance)));

        // Check source account balances.
        let bals =
            Accounts::get_balances(keys::alice::address()).expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            999_000,
            "balance in source account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check source account balance.
        let balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE)
            .expect("get_balance should succeed");
        assert_eq!(
            balance, 999_000,
            "balance in source account should be correct"
        );

        // Check destination account balances.
        let bals =
            Accounts::get_balances(keys::bob::address()).expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            1_000,
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
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
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
                amount: BaseUnits::new(1_000, Denomination::NATIVE),
                gas: 1000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };

    // Should succeed with enough funds to pay for fees.
    Accounts::authenticate_tx(&mut ctx, &tx).expect("transaction authentication should succeed");
    // Check source account balances.
    let bals = Accounts::get_balances(keys::alice::address()).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        999_000,
        "fees should be subtracted from source account"
    );
    assert_eq!(
        bals.balances.len(),
        1,
        "there should only be one denomination"
    );
    // Check source account nonce.
    let nonce = Accounts::get_nonce(keys::alice::address()).expect("get_nonce should succeed");
    assert_eq!(nonce, 1, "nonce should be incremented");
    // Check priority.
    let priority = core::Module::<mock::Config>::take_priority(&mut ctx);
    assert_eq!(priority, 1, "priority should be equal to gas price");

    // Should fail with an invalid nonce.
    let result = Accounts::authenticate_tx(&mut ctx, &tx);
    assert!(matches!(result, Err(core::Error::InvalidNonce)));

    // Should fail when there's not enough balance to pay fees.
    tx.auth_info.signer_info[0].nonce = nonce;
    tx.auth_info.fee.amount = BaseUnits::new(1_100_000, Denomination::NATIVE);
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
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
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
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };

    // Transfer tokens from one account to the other and check balances.
    ctx.with_tx(tx.into(), |mut tx_ctx, call| {
        Accounts::tx_transfer(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("transfer should succeed");

        // Check source account balances.
        let bals =
            Accounts::get_balances(keys::alice::address()).expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            999_000,
            "balance in source account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check destination account balances.
        let bals =
            Accounts::get_balances(keys::bob::address()).expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            1_000,
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
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
                amount: Default::default(),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                // Use an amount that does not split nicely among the good compute entities.
                amount: BaseUnits::new(1_001, Denomination::NATIVE),
                gas: 1000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };

    // Authenticate transaction, fees should be moved to accumulator.
    Accounts::authenticate_tx(&mut ctx, &tx).expect("transaction authentication should succeed");
    ctx.with_tx(tx.clone().into(), |mut tx_ctx, _call| {
        // Run after call tx handler.
        Accounts::after_handle_call(
            &mut tx_ctx,
            module::CallResult::Ok(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
        )
        .expect("after_handle_call should succeed");
        tx_ctx.commit()
    });

    // Run after dispatch hooks.
    Accounts::after_dispatch_tx(
        &mut ctx,
        &tx.auth_info,
        &module::CallResult::Ok(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
    );
    // Run end block handler.
    Accounts::end_block(&mut ctx);

    // Check source account balances.
    let bals = Accounts::get_balances(keys::alice::address()).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        998_999,
        "fees should be subtracted from source account"
    );
    // Nothing should be disbursed yet to good compute entity accounts.
    let bals = Accounts::get_balances(keys::bob::address()).expect("get_balances should succeed");
    assert!(bals.balances.is_empty(), "nothing should be disbursed yet");
    // Check second good compute entity account balances.
    let bals =
        Accounts::get_balances(keys::charlie::address()).expect("get_balances should succeed");
    assert!(bals.balances.is_empty(), "nothing should be disbursed yet");

    // Fees should be placed in the fee accumulator address to be disbursed in the next round.
    let bals =
        Accounts::get_balances(*ADDRESS_FEE_ACCUMULATOR).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1001,
        "fees should be held in the fee accumulator address"
    );

    // Simulate another block happening.
    Accounts::end_block(&mut ctx);

    // Fees should be removed from the fee accumulator address.
    let bals =
        Accounts::get_balances(*ADDRESS_FEE_ACCUMULATOR).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        0,
        "fees should have moved from the fee accumulator address"
    );

    // Check first good compute entity account balances.
    let bals = Accounts::get_balances(keys::bob::address()).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        500,
        "fees should be disbursed to good compute entity"
    );
    // Check second good compute entity account balances.
    let bals =
        Accounts::get_balances(keys::charlie::address()).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        500,
        "fees should be disbursed to good compute entity"
    );

    // Check the common pool which should have the remainder.
    let bals = Accounts::get_balances(*ADDRESS_COMMON_POOL).expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1,
        "remainder should be disbursed to the common pool"
    );
}

#[test]
fn test_query_addresses() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    let dn = Denomination::NATIVE;
    let d1: Denomination = "den1".parse().unwrap();

    let accs = Accounts::query_addresses(
        &mut ctx,
        AddressesQuery {
            denomination: dn.clone(),
        },
    )
    .expect("query accounts should succeed");
    assert_eq!(accs.len(), 0, "there should be no accounts initially");

    let gen = Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(keys::alice::address(), {
                let mut denominations = BTreeMap::new();
                denominations.insert(dn.clone(), 1_000_000);
                denominations.insert(d1.clone(), 1_000);
                denominations
            });
            // Bob.
            balances.insert(keys::bob::address(), {
                let mut denominations = BTreeMap::new();
                denominations.insert(d1.clone(), 2_000);
                denominations
            });
            balances
        },
        total_supplies: {
            let mut total_supplies = BTreeMap::new();
            total_supplies.insert(dn.clone(), 1_000_000);
            total_supplies.insert(d1.clone(), 3_000);
            total_supplies
        },
        ..Default::default()
    };

    Accounts::init(gen);

    ctx.with_tx(mock::transaction().into(), |mut tx_ctx, _call| {
        let accs = Accounts::query_addresses(&mut tx_ctx, AddressesQuery { denomination: d1 })
            .expect("query accounts should succeed");
        assert_eq!(accs.len(), 2, "there should be two addresses");
        assert_eq!(
            accs,
            Vec::from_iter([keys::bob::address(), keys::alice::address()]),
            "addresses should be correct"
        );

        let accs = Accounts::query_addresses(&mut tx_ctx, AddressesQuery { denomination: dn })
            .expect("query accounts should succeed");
        assert_eq!(accs.len(), 1, "there should be one address");
        assert_eq!(
            accs,
            Vec::from_iter([keys::alice::address()]),
            "addresses should be correct"
        );
    });
}

#[test]
fn test_get_all_balances_and_total_supplies_basic() {
    let _mock = mock::Mock::default();

    let alice = keys::alice::address();
    let bob = keys::bob::address();

    let gen = Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(alice, {
                let mut denominations = BTreeMap::new();
                denominations.insert(Denomination::NATIVE, 1_000_000);
                denominations
            });
            // Bob.
            balances.insert(bob, {
                let mut denominations = BTreeMap::new();
                denominations.insert(Denomination::NATIVE, 2_000_000);
                denominations
            });
            balances
        },
        total_supplies: {
            let mut total_supplies = BTreeMap::new();
            total_supplies.insert(Denomination::NATIVE, 3_000_000);
            total_supplies
        },
        ..Default::default()
    };

    Accounts::init(gen);

    let all_bals = Accounts::get_all_balances().expect("get_all_balances should succeed");
    for (addr, bals) in &all_bals {
        assert_eq!(bals.len(), 1, "exactly one denomination should be present");
        assert!(
            bals.contains_key(&Denomination::NATIVE),
            "only native denomination should be present"
        );
        if addr == &alice {
            assert_eq!(
                bals[&Denomination::NATIVE],
                1_000_000,
                "Alice's balance should be 1000000"
            );
        } else if addr == &bob {
            assert_eq!(
                bals[&Denomination::NATIVE],
                2_000_000,
                "Bob's balance should be 2000000"
            );
        } else {
            panic!("invalid address");
        }
    }

    let ts = Accounts::get_total_supplies().expect("get_total_supplies should succeed");
    assert_eq!(
        ts.len(),
        1,
        "exactly one denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&Denomination::NATIVE),
        "only native denomination should be present in total supplies"
    );
    assert_eq!(
        ts[&Denomination::NATIVE],
        3_000_000,
        "total supply should be 3000000"
    );
}

#[test]
fn test_get_all_balances_and_total_supplies_more() {
    let _mock = mock::Mock::default();

    let dn = Denomination::NATIVE;
    let d1: Denomination = "den1".parse().unwrap();
    let d2: Denomination = "den2".parse().unwrap();
    let d3: Denomination = "den3".parse().unwrap();

    let alice = keys::alice::address();
    let bob = keys::bob::address();

    let gen = Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(alice, {
                let mut denominations = BTreeMap::new();
                denominations.insert(dn.clone(), 1_000_000);
                denominations.insert(d1.clone(), 1_000);
                denominations.insert(d2.clone(), 100);
                denominations
            });
            // Bob.
            balances.insert(bob, {
                let mut denominations = BTreeMap::new();
                denominations.insert(d1.clone(), 2_000);
                denominations.insert(d3.clone(), 200);
                denominations
            });
            balances
        },
        total_supplies: {
            let mut total_supplies = BTreeMap::new();
            total_supplies.insert(dn.clone(), 1_000_000);
            total_supplies.insert(d1.clone(), 3_000);
            total_supplies.insert(d2.clone(), 100);
            total_supplies.insert(d3.clone(), 200);
            total_supplies
        },
        ..Default::default()
    };

    Accounts::init(gen);

    let all_bals = Accounts::get_all_balances().expect("get_all_balances should succeed");
    for (addr, bals) in &all_bals {
        if addr == &alice {
            assert_eq!(bals.len(), 3, "Alice should have exactly 3 denominations");
            assert_eq!(
                bals[&dn], 1_000_000,
                "Alice's native balance should be 1000000"
            );
            assert_eq!(bals[&d1], 1_000, "Alice's den1 balance should be 1000");
            assert_eq!(bals[&d2], 100, "Alice's den2 balance should be 100");
        } else if addr == &bob {
            assert_eq!(bals.len(), 2, "Bob should have exactly 2 denominations");
            assert_eq!(bals[&d1], 2_000, "Bob's den1 balance should be 2000");
            assert_eq!(bals[&d3], 200, "Bob's den3 balance should be 200");
        } else {
            panic!("invalid address");
        }
    }

    let ts = Accounts::get_total_supplies().expect("get_total_supplies should succeed");
    assert_eq!(
        ts.len(),
        4,
        "exactly 4 denominations should be present in total supplies"
    );
    assert!(
        ts.contains_key(&dn),
        "native denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&d1),
        "den1 denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&d2),
        "den2 denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&d3),
        "den3 denomination should be present in total supplies"
    );
    assert_eq!(ts[&dn], 1_000_000, "native total supply should be 1000000");
    assert_eq!(ts[&d1], 3_000, "den1 total supply should be 3000");
    assert_eq!(ts[&d2], 100, "den2 total supply should be 100");
    assert_eq!(ts[&d3], 200, "den3 total supply should be 200");
}

#[test]
fn test_check_invariants_basic() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    assert!(
        Accounts::check_invariants(&mut ctx).is_ok(),
        "invariants check should succeed"
    );
}

#[test]
fn test_check_invariants_more() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    let dn = Denomination::NATIVE;
    let d1: Denomination = "den1".parse().unwrap();
    let d2: Denomination = "den2".parse().unwrap();
    let d3: Denomination = "den3".parse().unwrap();

    let alice = keys::alice::address();
    let bob = keys::bob::address();
    let charlie = keys::charlie::address();

    let gen = Genesis {
        balances: {
            let mut balances = BTreeMap::new();
            // Alice.
            balances.insert(alice, {
                let mut denominations = BTreeMap::new();
                denominations.insert(dn.clone(), 1_000_000);
                denominations.insert(d1.clone(), 1_000);
                denominations.insert(d2.clone(), 100);
                denominations
            });
            // Bob.
            balances.insert(bob, {
                let mut denominations = BTreeMap::new();
                denominations.insert(d1.clone(), 2_000);
                denominations.insert(d3.clone(), 200);
                denominations
            });
            balances
        },
        total_supplies: {
            let mut total_supplies = BTreeMap::new();
            total_supplies.insert(dn, 1_000_000);
            total_supplies.insert(d1.clone(), 3_000);
            total_supplies.insert(d2, 100);
            total_supplies.insert(d3, 200);
            total_supplies
        },
        ..Default::default()
    };

    Accounts::init(gen);
    assert!(
        Accounts::check_invariants(&mut ctx).is_ok(),
        "initial inv chk should succeed"
    );

    assert!(
        Accounts::add_amount(charlie, &BaseUnits::new(100, d1.clone())).is_ok(),
        "giving Charlie money should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_err(),
        "inv chk 1 should fail"
    );

    assert!(
        Accounts::inc_total_supply(&BaseUnits::new(100, d1)).is_ok(),
        "increasing total supply should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_ok(),
        "inv chk 2 should succeed"
    );

    let d4: Denomination = "den4".parse().unwrap();

    assert!(
        Accounts::add_amount(charlie, &BaseUnits::new(300, d4.clone())).is_ok(),
        "giving Charlie more money should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_err(),
        "inv chk 3 should fail"
    );

    assert!(
        Accounts::inc_total_supply(&BaseUnits::new(300, d4)).is_ok(),
        "increasing total supply should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_ok(),
        "inv chk 4 should succeed"
    );

    let d5: Denomination = "den5".parse().unwrap();

    assert!(
        Accounts::inc_total_supply(&BaseUnits::new(123, d5.clone())).is_ok(),
        "increasing total supply should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_err(),
        "inv chk 5 should fail"
    );

    assert!(
        Accounts::add_amount(charlie, &BaseUnits::new(123, d5)).is_ok(),
        "giving Charlie more money should succeed"
    );
    assert!(
        Accounts::check_invariants(&mut ctx).is_ok(),
        "inv chk 6 should succeed"
    );
}

#[test]
fn test_fee_manager_normal() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    // Check that Accounts::charge_tx_fee works.
    ctx.with_tx(mock::transaction().into(), |mut tx_ctx, _call| {
        Accounts::charge_tx_fee(
            &mut tx_ctx,
            keys::alice::address(),
            &BaseUnits::new(1_000, Denomination::NATIVE),
        )
        .expect("charge tx fee should succeed");

        let ab = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE)
            .expect("get_balance should succeed");
        assert_eq!(ab, 999_000, "balance in source account should be correct");

        // Setting the refund request should have no effect.
        Accounts::set_refund_unused_tx_fee(&mut tx_ctx, true);

        let ab = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE)
            .expect("get_balance should succeed");
        assert_eq!(ab, 999_000, "balance in source account should be correct");
    });
}

#[test]
fn test_fee_manager_sim() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    // Check that Accounts::charge_tx_fee doesn't do anything in simulation mode.
    ctx.with_simulation(|mut sctx| {
        sctx.with_tx(mock::transaction().into(), |mut tx_ctx, _call| {
            Accounts::charge_tx_fee(
                &mut tx_ctx,
                keys::alice::address(),
                &BaseUnits::new(1_000, Denomination::NATIVE),
            )
            .expect("charge tx fee should succeed");

            let ab = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE)
                .expect("get_balance should succeed");
            assert_eq!(ab, 1_000_000, "balance in source account should be correct");
        });
    });
}

#[test]
fn test_get_set_nonce() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let nonce = Accounts::get_nonce(keys::alice::address()).unwrap();
    assert_eq!(nonce, 0);

    Accounts::set_nonce(keys::alice::address(), 2);

    let nonce = Accounts::get_nonce(keys::alice::address()).unwrap();
    assert_eq!(nonce, 2);
}

#[test]
fn test_get_set_balance() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 1_000_000);

    Accounts::set_balance(
        keys::alice::address(),
        &BaseUnits::new(500_000, Denomination::NATIVE),
    );

    let balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 500_000);
}

#[test]
fn test_get_set_total_supply() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let ts = Accounts::get_total_supplies().expect("get_total_supplies should succeed");
    assert_eq!(
        ts.len(),
        1,
        "exactly one denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&Denomination::NATIVE),
        "only native denomination should be present in total supplies"
    );
    assert_eq!(
        ts[&Denomination::NATIVE],
        1_000_000,
        "total supply should be 1000000"
    );

    // Set total supply to 2m, note that this violates invariants.
    Accounts::set_total_supply(&BaseUnits::new(2_000_000, Denomination::NATIVE));

    let ts = Accounts::get_total_supplies().expect("get_total_supplies should succeed");
    assert_eq!(
        ts.len(),
        1,
        "exactly one denomination should be present in total supplies"
    );
    assert!(
        ts.contains_key(&Denomination::NATIVE),
        "only native denomination should be present in total supplies"
    );
    assert_eq!(
        ts[&Denomination::NATIVE],
        2_000_000,
        "total supply should be 2000000"
    );
}

#[test]
fn test_query_denomination_info() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let di = Accounts::query_denomination_info(
        &mut ctx,
        DenominationInfoQuery {
            denomination: Denomination::NATIVE,
        },
    )
    .unwrap();
    assert_eq!(di.decimals, 9);

    // Query for missing info should fail.
    Accounts::query_denomination_info(
        &mut ctx,
        DenominationInfoQuery {
            denomination: "MISSING".parse().unwrap(),
        },
    )
    .unwrap_err();
}

#[test]
fn test_transaction_expiry() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    let mut tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(Transfer {
                to: keys::bob::address(),
                amount: Default::default(),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                // Use an amount that does not split nicely among the good compute entities.
                amount: BaseUnits::new(1_001, Denomination::NATIVE),
                gas: 1000,
                consensus_messages: 0,
            },
            not_before: Some(10),
            not_after: Some(42),
        },
    };

    // Authenticate transaction, should be expired.
    let err = Accounts::authenticate_tx(&mut ctx, &tx).expect_err("tx should be expired (early)");
    assert!(matches!(err, core::Error::ExpiredTransaction));

    // Move the round forward.
    mock.runtime_header.round = 15;

    // Authenticate transaction, should succeed.
    let mut ctx = mock.create_ctx();
    Accounts::authenticate_tx(&mut ctx, &tx).expect("tx should be valid");

    // Move the round forward and also update the transaction nonce.
    mock.runtime_header.round = 50;
    tx.auth_info.signer_info[0].nonce = 1;

    // Authenticate transaction, should be expired.
    let mut ctx = mock.create_ctx();
    let err = Accounts::authenticate_tx(&mut ctx, &tx).expect_err("tx should be expired");
    assert!(matches!(err, core::Error::ExpiredTransaction));
}

#[test]
fn test_fee_disbursement_2() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<TestRuntime>(context::Mode::ExecuteTx, false);
    let mut signer = mock::Signer::new(0, keys::alice::sigspec());

    TestRuntime::migrate(&mut ctx);

    // Do a simple transfer.
    let dispatch_result = signer.call_opts(
        &mut ctx,
        "accounts.Transfer",
        Transfer {
            to: keys::bob::address(),
            amount: BaseUnits::new(5_000, Denomination::NATIVE),
        },
        mock::CallOptions {
            fee: transaction::Fee {
                amount: BaseUnits::new(1_500, Denomination::NATIVE),
                gas: 1_500,
                ..Default::default()
            },
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Make sure two events were emitted and are properly formatted.
    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 2, "two events should have been emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    #[derive(Debug, Default, cbor::Decode)]
    struct TransferEvent {
        from: Address,
        to: Address,
        amount: BaseUnits,
    }

    let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 2); // One event for transfer, one event for fee payment.
    let event = &events[0];
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.to, keys::bob::address());
    assert_eq!(event.amount, BaseUnits::new(5_000, Denomination::NATIVE));
    let event = &events[1];
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.to, *ADDRESS_FEE_ACCUMULATOR);
    assert_eq!(event.amount, BaseUnits::new(1_500, Denomination::NATIVE));

    // Make sure only one gas used event was emitted.
    #[derive(Debug, Default, cbor::Decode)]
    struct GasUsedEvent {
        amount: u64,
    }

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, 1_000);
}

#[test]
fn test_fee_refund() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<TestRuntime>(context::Mode::ExecuteTx, false);
    let mut signer = mock::Signer::new(0, keys::alice::sigspec());

    TestRuntime::migrate(&mut ctx);

    // Test refund on success and failure.
    for fail in [false, true] {
        let dispatch_result = signer.call_opts(
            &mut ctx,
            "test.RefundFee",
            fail,
            mock::CallOptions {
                fee: transaction::Fee {
                    amount: BaseUnits::new(100_000, Denomination::NATIVE),
                    gas: 100_000,
                    ..Default::default()
                },
            },
        );

        assert_eq!(dispatch_result.result.is_success(), !fail);

        // Make sure two events were emitted and are properly formatted.
        let tags = &dispatch_result.tags;
        assert_eq!(tags.len(), 2, "two events should have been emitted");
        assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
        assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

        #[derive(Debug, Default, cbor::Decode)]
        struct TransferEvent {
            from: Address,
            to: Address,
            amount: BaseUnits,
        }

        let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
        assert_eq!(events.len(), 1); // One event for fee payment.
        let event = &events[0];
        assert_eq!(event.from, keys::alice::address());
        assert_eq!(event.to, *ADDRESS_FEE_ACCUMULATOR);
        assert_eq!(event.amount, BaseUnits::new(10_000, Denomination::NATIVE));

        #[derive(Debug, Default, cbor::Decode)]
        struct GasUsedEvent {
            amount: u64,
        }

        let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].amount, 10_000);
    }
}

#[test]
fn test_fee_refund_subcall() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<TestRuntime>(context::Mode::ExecuteTx, false);
    let mut signer = mock::Signer::new(0, keys::alice::sigspec());

    TestRuntime::migrate(&mut ctx);

    // Make sure that having a subcall that refunds fees does not affect the transaction.
    let dispatch_result = signer.call_opts(
        &mut ctx,
        "test.Subcall",
        (),
        mock::CallOptions {
            fee: transaction::Fee {
                amount: BaseUnits::new(100_000, Denomination::NATIVE),
                gas: 100_000,
                ..Default::default()
            },
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Make sure two events were emitted and are properly formatted.
    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 2, "two events should have been emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    #[derive(Debug, Default, cbor::Decode)]
    struct TransferEvent {
        from: Address,
        to: Address,
        amount: BaseUnits,
    }

    let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // One event for fee payment.
    let event = &events[0];
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.to, *ADDRESS_FEE_ACCUMULATOR);
    assert_eq!(
        event.amount,
        BaseUnits::new(100_000, Denomination::NATIVE),
        "no fee refunds"
    );

    #[derive(Debug, Default, cbor::Decode)]
    struct GasUsedEvent {
        amount: u64,
    }

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].amount, 11_000);
}
