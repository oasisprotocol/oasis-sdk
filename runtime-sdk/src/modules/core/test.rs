use std::collections::BTreeMap;

use oasis_core_runtime::types::BATCH_WEIGHT_LIMIT_QUERY_METHOD;

use crate::{
    context::{BatchContext, Context, Mode, TxContext},
    core::common::version::Version,
    crypto::multisig,
    dispatcher, module,
    module::{AuthHandler as _, BlockHandler, Module as _},
    runtime::Runtime,
    testing::{keys, mock},
    types::{token, transaction, transaction::TransactionWeight},
};

use super::{Module as Core, Parameters, API as _, GAS_WEIGHT_NAME};

#[test]
fn test_use_gas() {
    const MAX_GAS: u64 = 1000;
    const BLOCK_MAX_GAS: u64 = 3 * MAX_GAS + 2;
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: BLOCK_MAX_GAS,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: Default::default(),
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 0);
                mgp
            },
        },
    );

    Core::use_batch_gas(&mut ctx, 1).expect("using batch gas under limit should succeed");

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = MAX_GAS;

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS).expect("using gas under limit should succeed");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS)
            .expect("gas across separate transactions shouldn't accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS).unwrap();
        Core::use_tx_gas(&mut tx_ctx, 1).expect_err("gas in same transaction should accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, 1).unwrap();
        Core::use_tx_gas(&mut tx_ctx, u64::MAX).expect_err("overflow should cause error");
    });

    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    ctx.with_tx(big_tx, |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, u64::MAX).expect_err("batch overflow should cause error");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, 1).expect_err("batch gas should accumulate");
    });

    Core::use_batch_gas(&mut ctx, 1).expect_err("batch gas should accumulate outside tx");

    let mut ctx = mock.create_check_ctx();
    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    ctx.with_tx(big_tx, |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, u64::MAX)
            .expect("batch overflow should not happen in check-tx");
    });
}

// Module that implements the gas waster method.
struct GasWasterModule;

impl GasWasterModule {
    const CALL_GAS: u64 = 100;
    const METHOD_WASTE_GAS: &'static str = "test.WasteGas";
}

impl module::Module for GasWasterModule {
    const NAME: &'static str = "gaswaster";
    type Error = std::convert::Infallible;
    type Event = ();
    type Parameters = ();
}

impl module::MethodHandler for GasWasterModule {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, module::CallResult> {
        match method {
            Self::METHOD_WASTE_GAS => {
                Core::use_tx_gas(ctx, Self::CALL_GAS).expect("use_gas should succeed");
                module::DispatchResult::Handled(module::CallResult::Ok(cbor::Value::Simple(
                    cbor::SimpleValue::NullValue,
                )))
            }
            _ => module::DispatchResult::Unhandled(body),
        }
    }
}

impl module::BlockHandler for GasWasterModule {}
impl module::AuthHandler for GasWasterModule {}
impl module::MigrationHandler for GasWasterModule {
    type Genesis = ();
}
impl module::InvariantHandler for GasWasterModule {}

// Runtime that knows how to waste gas.
struct GasWasterRuntime;

impl GasWasterRuntime {
    const AUTH_SIGNATURE_GAS: u64 = 1;
    const AUTH_MULTISIG_GAS: u64 = 10;
}

impl Runtime for GasWasterRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Modules = (Core, GasWasterModule);

    fn genesis_state() -> (super::Genesis, ()) {
        (
            super::Genesis {
                parameters: Parameters {
                    max_batch_gas: u64::MAX,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: super::GasCosts {
                        auth_signature: Self::AUTH_SIGNATURE_GAS,
                        auth_multisig_signer: Self::AUTH_MULTISIG_GAS,
                        callformat_x25519_deoxysii: 0,
                    },
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(token::Denomination::NATIVE, 0);
                        mgp
                    },
                },
            },
            (),
        )
    }
}

#[test]
fn test_reject_txs() {
    // The gas waster runtime doesn't implement any authenticate_tx handler,
    // so it should reject all transactions.
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    GasWasterRuntime::migrate(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![
                transaction::SignerInfo::new(keys::alice::pk(), 0),
                transaction::SignerInfo::new_multisig(
                    multisig::Config {
                        signers: vec![multisig::Signer {
                            public_key: keys::bob::pk(),
                            weight: 1,
                        }],
                        threshold: 1,
                    },
                    0,
                ),
            ],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                gas: u64::MAX,
                consensus_messages: 0,
            },
        },
    };

    Core::authenticate_tx(&mut ctx, &tx).expect_err("no module could authenticate the transaction");
}

#[test]
fn test_query_estimate_gas() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    GasWasterRuntime::migrate(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![
                transaction::SignerInfo::new(keys::alice::pk(), 0),
                transaction::SignerInfo::new_multisig(
                    multisig::Config {
                        signers: vec![multisig::Signer {
                            public_key: keys::bob::pk(),
                            weight: 1,
                        }],
                        threshold: 1,
                    },
                    0,
                ),
            ],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                gas: u64::MAX,
                consensus_messages: 0,
            },
        },
    };

    let est = Core::query_estimate_gas(&mut ctx, tx).expect("query_estimate_gas should succeed");
    let reference_gas = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS;
    assert_eq!(est, reference_gas, "estimated gas should be correct");
}

#[test]
fn test_approve_unverified_tx() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: u64::MAX,
            max_tx_signers: 2,
            max_multisig_signers: 2,
            gas_costs: Default::default(),
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 0);
                mgp
            },
        },
    );
    let dummy_bytes = b"you look, you die".to_vec();
    Core::approve_unverified_tx(
        &mut ctx,
        &transaction::UnverifiedTransaction(
            dummy_bytes.clone(),
            vec![
                transaction::AuthProof::Signature(dummy_bytes.clone().into()),
                transaction::AuthProof::Multisig(vec![None, None]),
            ],
        ),
    )
    .expect("at max");
    Core::approve_unverified_tx(
        &mut ctx,
        &transaction::UnverifiedTransaction(
            dummy_bytes.clone(),
            vec![
                transaction::AuthProof::Signature(dummy_bytes.clone().into()),
                transaction::AuthProof::Multisig(vec![None, None]),
                transaction::AuthProof::Signature(dummy_bytes.clone().into()),
            ],
        ),
    )
    .expect_err("too many authentication slots");
    Core::approve_unverified_tx(
        &mut ctx,
        &transaction::UnverifiedTransaction(
            dummy_bytes.clone(),
            vec![
                transaction::AuthProof::Signature(dummy_bytes.clone().into()),
                transaction::AuthProof::Multisig(vec![None, None, None]),
            ],
        ),
    )
    .expect_err("multisig too many signers");
}

#[test]
fn test_add_priority() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    assert_eq!(
        0,
        Core::take_priority(&mut ctx),
        "default priority should be 0"
    );

    Core::add_priority(&mut ctx, 1).expect("adding priority should succeed");
    Core::add_priority(&mut ctx, 11).expect("adding priority should succeed");

    let tx = mock::transaction();
    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::add_priority(&mut tx_ctx, 10)
            .expect("adding priority from tx context should succeed");
    });

    assert_eq!(
        22,
        Core::take_priority(&mut ctx),
        "adding priority should work"
    );
}

#[test]
fn test_add_priority_overflow() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::add_priority(&mut ctx, u64::MAX).expect("adding priority should succeed");
    Core::add_priority(&mut ctx, u64::MAX).expect("adding priority should succeed");

    assert_eq!(
        u64::MAX,
        Core::take_priority(&mut ctx),
        "adding priority should work"
    );
}

#[test]
fn test_add_weights() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    assert!(
        Core::take_weights(&mut ctx).is_empty(),
        "default weights should be empty"
    );

    let tx = mock::transaction();
    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::add_weight(&mut tx_ctx, "test_weight".into(), 1)
            .expect("adding weight should succeed");
        Core::add_weight(&mut tx_ctx, "test_weight2".into(), 10)
            .expect("adding weight should succeed");
        Core::add_weight(&mut tx_ctx, "test_weight".into(), 15)
            .expect("adding weight should succeed");
        Core::add_weight(&mut tx_ctx, "test_weight3".into(), 20)
            .expect("adding weight should succeed");
        Core::add_weight(&mut tx_ctx, "test_weight4".into(), u64::MAX)
            .expect("adding weight should succeed");
        Core::add_weight(&mut tx_ctx, "test_weight4".into(), 5)
            .expect("adding weight should succeed");
    });

    let mut expected = BTreeMap::new();
    expected.insert("test_weight".into(), 1 + 15);
    expected.insert("test_weight2".into(), 10);
    expected.insert("test_weight3".into(), 20);
    expected.insert("test_weight4".into(), u64::MAX);

    assert_eq!(
        expected,
        Core::take_weights(&mut ctx),
        "adding weights should work"
    );
}

#[test]
fn test_get_batch_weight_limits() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    // Empty max batch gas.
    let mut expected = BTreeMap::new();
    expected.insert(GAS_WEIGHT_NAME.into(), 0);

    assert_eq!(
        expected,
        Core::get_block_weight_limits(&mut ctx),
        "querying empty weights limits should work"
    );

    // Update max_batch_gas.
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: 100,
            ..Default::default()
        },
    );
    expected.insert(GAS_WEIGHT_NAME.into(), 100);
    assert_eq!(
        expected,
        Core::get_block_weight_limits(&mut ctx),
        "querying weights limits should work"
    );
}

#[test]
fn test_get_batch_weight_limits_query() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    // Empty max batch gas.
    let mut expected: BTreeMap<TransactionWeight, u64> = BTreeMap::new();
    expected.insert(GAS_WEIGHT_NAME.into(), 0);

    let res = dispatcher::Dispatcher::<GasWasterRuntime>::dispatch_query(
        &mut ctx,
        BATCH_WEIGHT_LIMIT_QUERY_METHOD,
        cbor::to_vec(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
    )
    .expect("batch weight limit query should work");
    assert_eq!(
        expected,
        cbor::from_slice(&res).unwrap(),
        "querying empty weights should return correct limits"
    );

    // Update max_batch_gas.
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: 100,
            ..Default::default()
        },
    );
    expected.insert(GAS_WEIGHT_NAME.into(), 100);

    let res = dispatcher::Dispatcher::<GasWasterRuntime>::dispatch_query(
        &mut ctx,
        BATCH_WEIGHT_LIMIT_QUERY_METHOD,
        cbor::to_vec(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
    )
    .expect("batch weight limit query should work");
    assert_eq!(
        expected,
        cbor::from_slice(&res).unwrap(),
        "querying weights should return correct limits"
    );
}

#[test]
fn test_min_gas_price() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: u64::MAX,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: super::GasCosts {
                auth_signature: GasWasterRuntime::AUTH_SIGNATURE_GAS,
                auth_multisig_signer: GasWasterRuntime::AUTH_MULTISIG_GAS,
                callformat_x25519_deoxysii: 0,
            },
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 1000);
                mgp
            },
        },
    );

    let mut tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![
                transaction::SignerInfo::new(keys::alice::pk(), 0),
                transaction::SignerInfo::new_multisig(
                    multisig::Config {
                        signers: vec![multisig::Signer {
                            public_key: keys::bob::pk(),
                            weight: 1,
                        }],
                        threshold: 1,
                    },
                    0,
                ),
            ],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                gas: 100,
                consensus_messages: 0,
            },
        },
    };

    ctx.with_tx(tx.clone(), |mut tx_ctx, call| {
        Core::before_handle_call(&mut tx_ctx, &call).expect_err("gas price should be too low");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(100000, token::Denomination::NATIVE);

    ctx.with_tx(tx.clone(), |mut tx_ctx, call| {
        Core::before_handle_call(&mut tx_ctx, &call).expect("gas price should be ok");
    });
}
