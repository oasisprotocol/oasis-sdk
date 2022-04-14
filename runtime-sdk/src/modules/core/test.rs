use std::collections::{BTreeMap, BTreeSet};

use oasis_core_runtime::types::BATCH_WEIGHT_LIMIT_QUERY_METHOD;

use crate::{
    context::{BatchContext, Context, Mode, TxContext},
    core::common::version::Version,
    crypto::multisig,
    dispatcher,
    event::IntoTags,
    handler,
    module::{self, BlockHandler, Module as _, TransactionHandler as _},
    runtime::Runtime,
    sdk_derive,
    testing::{configmap, keys, mock},
    types::{
        token, transaction,
        transaction::{CallerAddress, TransactionWeight},
    },
};

use super::{types, Event, Parameters, API as _, GAS_WEIGHT_NAME};

type Core = super::Module<Config>;

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
            max_tx_size: 32 * 1024,
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
    assert_eq!(Core::remaining_batch_gas(&mut ctx), BLOCK_MAX_GAS - 1);

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = MAX_GAS;

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS).expect("using gas under limit should succeed");
        assert_eq!(
            Core::remaining_batch_gas(&mut tx_ctx),
            BLOCK_MAX_GAS - 1 - MAX_GAS
        );
        assert_eq!(Core::remaining_tx_gas(&mut tx_ctx), 0);
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), MAX_GAS);
    });

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS)
            .expect("gas across separate transactions shouldn't accumulate");
        assert_eq!(
            Core::remaining_batch_gas(&mut tx_ctx),
            BLOCK_MAX_GAS - 1 - 2 * MAX_GAS
        );
        assert_eq!(Core::remaining_tx_gas(&mut tx_ctx), 0);
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), MAX_GAS);
    });

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, MAX_GAS).unwrap();
        Core::use_tx_gas(&mut tx_ctx, 1).expect_err("gas in same transaction should accumulate");
        assert_eq!(Core::remaining_tx_gas(&mut tx_ctx), 0);
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), MAX_GAS);
    });

    assert_eq!(
        Core::remaining_batch_gas(&mut ctx),
        BLOCK_MAX_GAS - 1 - 3 * MAX_GAS
    );

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, 1).unwrap();
        assert_eq!(
            Core::remaining_batch_gas(&mut tx_ctx),
            BLOCK_MAX_GAS - 1 - 3 * MAX_GAS - 1
        );
        assert_eq!(
            Core::remaining_tx_gas(&mut tx_ctx),
            0,
            "remaining tx gas should take batch limit into account"
        );
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), 1);
        Core::use_tx_gas(&mut tx_ctx, u64::MAX).expect_err("overflow should cause error");
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), 1);
    });

    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    ctx.with_tx(0, big_tx, |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, u64::MAX).expect_err("batch overflow should cause error");
    });

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, 1).expect_err("batch gas should accumulate");
    });

    Core::use_batch_gas(&mut ctx, 1).expect_err("batch gas should accumulate outside tx");

    let mut ctx = mock.create_check_ctx();
    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    ctx.with_tx(0, big_tx, |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, u64::MAX)
            .expect("batch overflow should not happen in check-tx");
    });
}

#[test]
fn test_query_min_gas_price() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: 10000,
            max_tx_size: 32 * 1024,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: Default::default(),
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 123);
                mgp.insert("SMALLER".parse().unwrap(), 1000);
                mgp
            },
        },
    );

    let mgp = Core::query_min_gas_price(&mut ctx, ()).expect("query_min_gas_price should succeed");
    assert!(mgp.len() == 2);
    assert!(mgp.contains_key(&token::Denomination::NATIVE));
    assert!(*mgp.get(&token::Denomination::NATIVE).unwrap() == 123);
    assert!(mgp.contains_key(&"SMALLER".parse().unwrap()));
    assert!(*mgp.get(&"SMALLER".parse().unwrap()).unwrap() == 1000);

    // Test local override.
    struct MinGasPriceOverride;

    impl super::Config for MinGasPriceOverride {
        const DEFAULT_LOCAL_MIN_GAS_PRICE: once_cell::unsync::Lazy<
            BTreeMap<token::Denomination, u128>,
        > = once_cell::unsync::Lazy::new(|| {
            BTreeMap::from([
                (token::Denomination::NATIVE, 10_000),
                ("TEST".parse().unwrap(), 1_000),
                ("SMALLER".parse().unwrap(), 10),
            ])
        });
    }

    let mgp = super::Module::<MinGasPriceOverride>::query_min_gas_price(&mut ctx, ())
        .expect("query_min_gas_price should succeed");
    assert!(mgp.len() == 2);
    assert!(mgp.contains_key(&token::Denomination::NATIVE));
    assert!(*mgp.get(&token::Denomination::NATIVE).unwrap() == 10_000);
    assert!(mgp.contains_key(&"SMALLER".parse().unwrap()));
    assert!(*mgp.get(&"SMALLER".parse().unwrap()).unwrap() == 1000);
}

// Module that implements the gas waster method.
struct GasWasterModule;

impl GasWasterModule {
    const CALL_GAS: u64 = 100;
    const METHOD_WASTE_GAS: &'static str = "test.WasteGas";
}

impl module::Module for GasWasterModule {
    const NAME: &'static str = "gaswaster";
    const VERSION: u32 = 42;
    type Error = crate::modules::core::Error;
    type Event = ();
    type Parameters = ();
}

#[sdk_derive(MethodHandler)]
impl GasWasterModule {
    #[handler(call = Self::METHOD_WASTE_GAS)]
    fn waste_gas<C: TxContext>(
        ctx: &mut C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, Self::CALL_GAS)?;
        Ok(())
    }
}

impl module::BlockHandler for GasWasterModule {}
impl module::TransactionHandler for GasWasterModule {}
impl module::MigrationHandler for GasWasterModule {
    type Genesis = ();
}
impl module::InvariantHandler for GasWasterModule {}

struct Config;

impl super::Config for Config {}

// Runtime that knows how to waste gas.
struct GasWasterRuntime;

impl GasWasterRuntime {
    const AUTH_SIGNATURE_GAS: u64 = 1;
    const AUTH_MULTISIG_GAS: u64 = 10;
}

impl Runtime for GasWasterRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core;

    type Modules = (Core, GasWasterModule);

    fn genesis_state() -> (super::Genesis, ()) {
        (
            super::Genesis {
                parameters: Parameters {
                    max_batch_gas: u64::MAX,
                    max_tx_size: 32 * 1024,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    gas_costs: super::GasCosts {
                        tx_byte: 0,
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
    // so it should accept all transactions.
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
                transaction::SignerInfo::new_sigspec(keys::alice::sigspec(), 0),
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

    Core::authenticate_tx(&mut ctx, &tx).expect("authenticate should pass if all modules accept");
}

#[test]
fn test_query_estimate_gas() {
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![
                transaction::SignerInfo::new_sigspec(keys::alice::sigspec(), 0),
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

    // Gas that we expect `tx` to use.
    let reference_gas = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS;

    // Test happy-path execution with default settings.
    {
        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);
        GasWasterRuntime::migrate(&mut ctx);

        // Test estimation with caller derived from the transaction.
        let args = types::EstimateGasQuery {
            caller: None,
            tx: tx.clone(),
        };
        let est =
            Core::query_estimate_gas(&mut ctx, args).expect("query_estimate_gas should succeed");
        assert_eq!(est, reference_gas, "estimated gas should be correct");

        // Test estimation with specified caller.
        let args = types::EstimateGasQuery {
            caller: Some(CallerAddress::Address(keys::alice::address())),
            tx: tx.clone(),
        };
        let est =
            Core::query_estimate_gas(&mut ctx, args).expect("query_estimate_gas should succeed");
        assert_eq!(est, reference_gas, "estimated gas should be correct");
    }

    // Test expensive estimates.
    {
        let max_estimated_gas = reference_gas - 1;
        let local_config = configmap! {
            "core" => configmap! {
                "max_estimated_gas" => max_estimated_gas,
            },
        };
        let mut mock = mock::Mock::with_local_config(local_config);
        let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);
        GasWasterRuntime::migrate(&mut ctx);

        // Test with limited max_estimated_gas.
        let args = types::EstimateGasQuery {
            caller: None,
            tx: tx.clone(),
        };
        let est = Core::query_estimate_gas(&mut ctx, args)
            .expect("query_estimate_gas should succeed even with limited max_estimated_gas");
        assert!(
            est <= max_estimated_gas,
            "estimated gas should be at most max_estimated_gas={}, was {}",
            max_estimated_gas,
            est
        );
    }
}

#[test]
fn test_approve_unverified_tx() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: u64::MAX,
            max_tx_size: 32 * 1024,
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
    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
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
    ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
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

    GasWasterRuntime::migrate(&mut ctx);

    // Max batch gas as in runtime genesis (u64::MAX).
    let mut expected = BTreeMap::new();
    expected.insert(GAS_WEIGHT_NAME.into(), u64::MAX);

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

    // Max batch gas as in runtime genesis (u64::MAX).
    let mut expected: BTreeMap<TransactionWeight, u64> = BTreeMap::new();
    expected.insert(GAS_WEIGHT_NAME.into(), u64::MAX);

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
fn test_check_weights() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: u64::MAX,
            max_tx_size: 32 * 1024,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: super::GasCosts {
                tx_byte: 0,
                auth_signature: GasWasterRuntime::AUTH_SIGNATURE_GAS,
                auth_multisig_signer: GasWasterRuntime::AUTH_MULTISIG_GAS,
                callformat_x25519_deoxysii: 0,
            },
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 0);
                mgp
            },
        },
    );

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(10_000, token::Denomination::NATIVE),
                gas: 1_000,
                consensus_messages: 1,
            },
        },
    };

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        Core::before_handle_call(&mut tx_ctx, &call).unwrap();

        let weights = Core::take_weights(&mut tx_ctx);

        assert_eq!(
            weights.get(&GAS_WEIGHT_NAME.into()),
            Some(&1_000),
            "gas weight should be correct"
        );
        assert_eq!(
            weights.get(&TransactionWeight::ConsensusMessages),
            Some(&1),
            "consensus messages weight should be correct"
        );
    });
}

#[test]
fn test_min_gas_price() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: u64::MAX,
            max_tx_size: 32 * 1024,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: super::GasCosts {
                tx_byte: 0,
                auth_signature: GasWasterRuntime::AUTH_SIGNATURE_GAS,
                auth_multisig_signer: GasWasterRuntime::AUTH_MULTISIG_GAS,
                callformat_x25519_deoxysii: 0,
            },
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 1000);
                mgp.insert("SMALLER".parse().unwrap(), 100);
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
                transaction::SignerInfo::new_sigspec(keys::alice::sigspec(), 0),
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

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        Core::before_handle_call(&mut tx_ctx, &call).expect_err("gas price should be too low");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(100000, token::Denomination::NATIVE);

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        Core::before_handle_call(&mut tx_ctx, &call).expect("gas price should be ok");
    });

    // Test local override.
    struct MinGasPriceOverride;

    impl super::Config for MinGasPriceOverride {
        const DEFAULT_LOCAL_MIN_GAS_PRICE: once_cell::unsync::Lazy<
            BTreeMap<token::Denomination, u128>,
        > = once_cell::unsync::Lazy::new(|| {
            BTreeMap::from([
                (token::Denomination::NATIVE, 10_000),
                ("SMALLER".parse().unwrap(), 10),
            ])
        });

        const MIN_GAS_PRICE_EXEMPT_METHODS: once_cell::unsync::Lazy<BTreeSet<&'static str>> =
            once_cell::unsync::Lazy::new(|| BTreeSet::from(["exempt.Method"]));
    }

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect_err("gas price should be too low");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(1_000_000, token::Denomination::NATIVE);

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect("gas price should be ok");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(1_000, "SMALLER".parse().unwrap());

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect_err("gas price should be too low");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(10_000, "SMALLER".parse().unwrap());

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect("gas price should be ok");
    });

    // Test exempt methods.
    tx.call.method = "exempt.Method".into();
    tx.auth_info.fee.amount = token::BaseUnits::new(100_000, token::Denomination::NATIVE);

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect("method should be gas price exempt");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(0, token::Denomination::NATIVE);

    ctx.with_tx(0, tx.clone(), |mut tx_ctx, call| {
        super::Module::<MinGasPriceOverride>::before_handle_call(&mut tx_ctx, &call)
            .expect("method should be gas price exempt");
    });
}

#[test]
fn test_emit_events() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    #[derive(Debug, cbor::Encode, cbor::Decode, PartialEq, Eq)]
    struct TestEvent {
        i: u64,
    }

    impl crate::event::Event for TestEvent {
        fn module_name() -> &'static str {
            "testevent"
        }
        fn code(&self) -> u32 {
            match self {
                TestEvent { .. } => 0u32,
            }
        }
    }

    ctx.emit_event(TestEvent { i: 42 });
    let etags = ctx.with_tx(0, mock::transaction(), |mut ctx, _| {
        ctx.emit_event(TestEvent { i: 2 });
        ctx.emit_event(TestEvent { i: 3 });
        ctx.emit_event(TestEvent { i: 1 });

        let (etags, _) = ctx.commit();
        let tags = etags.clone().into_tags();
        assert_eq!(tags.len(), 1, "1 emitted tag expected");

        let events: Vec<TestEvent> = cbor::from_slice(&tags[0].value).unwrap();
        assert_eq!(events.len(), 3, "3 emitted events expected");
        assert_eq!(TestEvent { i: 2 }, events[0], "expected events emitted");
        assert_eq!(TestEvent { i: 3 }, events[1], "expected events emitted");
        assert_eq!(TestEvent { i: 1 }, events[2], "expected events emitted");

        etags
    });
    // Forward tx emitted etags.
    ctx.emit_etags(etags);
    // Emit one more event.
    ctx.emit_event(TestEvent { i: 0 });

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 1, "1 emitted tag expected");

    let events: Vec<TestEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 5, "5 emitted events expected");
    assert_eq!(TestEvent { i: 42 }, events[0], "expected events emitted");
    assert_eq!(TestEvent { i: 2 }, events[1], "expected events emitted");
    assert_eq!(TestEvent { i: 3 }, events[2], "expected events emitted");
    assert_eq!(TestEvent { i: 1 }, events[3], "expected events emitted");
    assert_eq!(TestEvent { i: 0 }, events[4], "expected events emitted");
}

#[test]
fn test_gas_used_events() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        Parameters {
            max_batch_gas: 1_000_000,
            max_tx_size: 32 * 1024,
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

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = 100_000;

    let etags = ctx.with_tx(0, tx.clone(), |mut tx_ctx, _call| {
        Core::use_tx_gas(&mut tx_ctx, 10).expect("using gas under limit should succeed");
        assert_eq!(Core::used_tx_gas(&mut tx_ctx), 10);
        Core::after_handle_call(&mut tx_ctx).unwrap();

        let (etags, _) = tx_ctx.commit();
        let tags = etags.clone().into_tags();
        assert_eq!(tags.len(), 1, "1 emitted tag expected");

        let expected = cbor::to_vec(vec![Event::GasUsed { amount: 10 }]);
        assert_eq!(tags[0].value, expected, "expected events emitted");

        etags
    });
    // Forward tx emitted etags.
    ctx.emit_etags(etags);

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 1, "1 emitted tags expected");

    let expected = cbor::to_vec(vec![Event::GasUsed { amount: 10 }]);
    assert_eq!(tags[0].value, expected, "expected events emitted");
}

/// Constructs a BTreeMap using a `btreemap! { key => value, ... }` syntax.
macro_rules! btreemap {
    // allow trailing comma
    ( $($key:expr => $value:expr,)+ ) => (btreemap!($($key => $value),+));
    ( $($key:expr => $value:expr),* ) => {
        {
            let mut m = BTreeMap::new();
            $( m.insert($key.into(), $value); )*
            m
        }
    };
}

#[test]
fn test_module_info() {
    use cbor::Encode;
    use types::{MethodHandlerInfo, MethodHandlerKind};

    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(Mode::CheckTx);

    // Set bogus params on the core module; we want to see them reflected in response to the `runtime_info()` query.
    let core_params = Parameters {
        max_batch_gas: 123,
        max_tx_signers: 4,
        max_multisig_signers: 567,
        ..Default::default()
    };
    Core::set_params(ctx.runtime_state(), core_params.clone());

    let info = Core::query_runtime_info(&mut ctx, ()).unwrap();
    assert_eq!(
        info,
        types::RuntimeInfoResponse {
            runtime_version: Version::new(0, 0, 0),
            state_version: 0,
            modules: btreemap! {
                "core" =>
                    types::ModuleInfo {
                        version: 1,
                        params: core_params.into_cbor_value(),
                        methods: vec![
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.EstimateGas".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.CheckInvariants".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.CallDataPublicKey".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.MinGasPrice".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.RuntimeInfo".to_string() }
                        ]
                    },
                "gaswaster" =>
                    types::ModuleInfo {
                        version: 42,
                        params: ().into_cbor_value(),
                        methods: vec![
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGas".to_string() }
                        ],
                    },
            }
        }
    );
}
