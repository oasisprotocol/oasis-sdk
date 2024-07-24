use std::collections::{BTreeMap, BTreeSet};

use once_cell::unsync::Lazy;

use crate::{
    context::Context,
    core::{
        common::{version::Version, versioned::Versioned},
        consensus::{roothash, staking},
    },
    crypto::multisig,
    error::Error,
    event::IntoTags,
    handler,
    module::{self, BlockHandler, Module as _, TransactionHandler as _},
    modules::core::min_gas_price_update,
    runtime::Runtime,
    sdk_derive,
    sender::SenderMeta,
    state::{self, CurrentState, Options},
    testing::{configmap, keys, mock},
    types::{
        address::Address, message::MessageEventHookInvocation, token, transaction,
        transaction::CallerAddress,
    },
};

use super::{types, Event, Parameters, API as _};

type Core = super::Module<Config>;

#[test]
fn test_use_gas() {
    const MAX_GAS: u64 = 1000;
    const BLOCK_MAX_GAS: u64 = 3 * MAX_GAS + 2;
    let _mock = mock::Mock::default();

    Core::set_params(Parameters {
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
        dynamic_min_gas_price: Default::default(),
    });

    assert_eq!(Core::max_batch_gas(), BLOCK_MAX_GAS);

    Core::use_batch_gas(1).expect("using batch gas under limit should succeed");
    assert_eq!(Core::remaining_batch_gas(), BLOCK_MAX_GAS - 1);

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = MAX_GAS;

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(MAX_GAS).expect("using gas under limit should succeed");
        assert_eq!(Core::remaining_batch_gas(), BLOCK_MAX_GAS - 1 - MAX_GAS);
        assert_eq!(Core::remaining_tx_gas(), 0);
        assert_eq!(Core::used_tx_gas(), MAX_GAS);
    });

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(MAX_GAS).expect("gas across separate transactions shouldn't accumulate");
        assert_eq!(Core::remaining_batch_gas(), BLOCK_MAX_GAS - 1 - 2 * MAX_GAS);
        assert_eq!(Core::remaining_tx_gas(), 0);
        assert_eq!(Core::used_tx_gas(), MAX_GAS);
    });

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(MAX_GAS).unwrap();
        Core::use_tx_gas(1).expect_err("gas in same transaction should accumulate");
        assert_eq!(Core::remaining_tx_gas(), 0);
        assert_eq!(Core::used_tx_gas(), MAX_GAS);
    });

    assert_eq!(Core::remaining_batch_gas(), BLOCK_MAX_GAS - 1 - 3 * MAX_GAS);
    assert_eq!(Core::max_batch_gas(), BLOCK_MAX_GAS);

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(1).unwrap();
        assert_eq!(
            Core::remaining_batch_gas(),
            BLOCK_MAX_GAS - 1 - 3 * MAX_GAS - 1
        );
        assert_eq!(
            Core::remaining_tx_gas(),
            0,
            "remaining tx gas should take batch limit into account"
        );
        assert_eq!(Core::used_tx_gas(), 1);
        Core::use_tx_gas(u64::MAX).expect_err("overflow should cause error");
        assert_eq!(Core::used_tx_gas(), 1);
    });

    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(u64::MAX).expect_err("batch overflow should cause error");
    });

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::use_tx_gas(1).expect_err("batch gas should accumulate");
    });

    Core::use_batch_gas(1).expect_err("batch gas should accumulate outside tx");

    let mut big_tx = tx;
    big_tx.auth_info.fee.gas = u64::MAX;

    CurrentState::with_transaction_opts(
        state::Options::new()
            .with_mode(state::Mode::Check)
            .with_tx(big_tx.into()),
        || {
            Core::use_tx_gas(u64::MAX).expect("batch overflow should not happen in check-tx");
        },
    );
}

#[test]
fn test_query_min_gas_price() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    Core::set_params(Parameters {
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
        dynamic_min_gas_price: Default::default(),
    });

    assert_eq!(Core::min_gas_price(&token::Denomination::NATIVE), Some(123));
    assert_eq!(Core::min_gas_price(&"SMALLER".parse().unwrap()), Some(1000));

    let mgp = Core::query_min_gas_price(&ctx, ()).expect("query_min_gas_price should succeed");
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

    assert_eq!(
        super::Module::<MinGasPriceOverride>::min_gas_price(&token::Denomination::NATIVE),
        Some(123)
    );
    assert_eq!(
        super::Module::<MinGasPriceOverride>::min_gas_price(&"SMALLER".parse().unwrap()),
        Some(1000)
    );

    let mgp = super::Module::<MinGasPriceOverride>::query_min_gas_price(&ctx, ())
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
    const CALL_GAS_HUGE: u64 = u64::MAX - 10_000;
    const CALL_GAS_SPECIFIC: u64 = 123456;
    const CALL_GAS_SPECIFIC_HUGE: u64 = u64::MAX - 10_000;
    const CALL_GAS_CALLER_ADDR_ZERO: u64 = 424242;
    const CALL_GAS_CALLER_ADDR_ETHZERO: u64 = 101010;
    const CALL_GAS_EXTRA: u64 = 10_000;

    const METHOD_WASTE_GAS: &'static str = "test.WasteGas";
    const METHOD_WASTE_GAS_AND_FAIL: &'static str = "test.WasteGasAndFail";
    const METHOD_WASTE_GAS_AND_FAIL_EXTRA: &'static str = "test.WasteGasAndFailExtra";
    const METHOD_WASTE_GAS_HUGE: &'static str = "test.WasteGasHuge";
    const METHOD_WASTE_GAS_CALLER: &'static str = "test.WasteGasCaller";
    const METHOD_SPECIFIC_GAS_REQUIRED: &'static str = "test.SpecificGasRequired";
    const METHOD_SPECIFIC_GAS_REQUIRED_HUGE: &'static str = "test.SpecificGasRequiredHuge";
    const METHOD_STORAGE_UPDATE: &'static str = "test.StorageUpdate";
    const METHOD_STORAGE_REMOVE: &'static str = "test.StorageRemove";
    const METHOD_EMIT_CONSENSUS_MESSAGE: &'static str = "test.EmitConsensusMessage";
}

#[sdk_derive(Module)]
impl GasWasterModule {
    const NAME: &'static str = "gaswaster";
    const VERSION: u32 = 42;
    type Error = crate::modules::core::Error;
    type Event = ();
    type Parameters = ();
    type Genesis = ();

    #[handler(call = Self::METHOD_WASTE_GAS)]
    fn waste_gas<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS)?;
        Ok(())
    }

    #[handler(call = Self::METHOD_WASTE_GAS_AND_FAIL)]
    fn fail<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS)?;
        Err(<GasWasterModule as module::Module>::Error::Forbidden)
    }

    #[handler(call = Self::METHOD_WASTE_GAS_AND_FAIL_EXTRA)]
    fn fail_extra<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS)?;
        Err(<GasWasterModule as module::Module>::Error::Forbidden)
    }

    #[handler(call = Self::METHOD_WASTE_GAS_HUGE)]
    fn waste_gas_huge<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS_HUGE)?;
        Ok(())
    }

    #[handler(call = Self::METHOD_WASTE_GAS_CALLER)]
    fn waste_gas_caller<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        // Uses a different amount of gas based on the caller.
        let caller = CurrentState::with_env(|env| env.tx_caller_address());
        let addr_zero = Address::default();
        let addr_ethzero = CallerAddress::EthAddress([0u8; 20]).address();

        if caller == addr_zero {
            <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS_CALLER_ADDR_ZERO)?;
        } else if caller == addr_ethzero {
            <C::Runtime as Runtime>::Core::use_tx_gas(Self::CALL_GAS_CALLER_ADDR_ETHZERO)?;
        }
        Ok(())
    }

    #[handler(call = Self::METHOD_SPECIFIC_GAS_REQUIRED)]
    fn specific_gas_required<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        // Fails with an error if less than X gas was specified. (doesn't fail with out-of-gas).
        let gas_limit = CurrentState::with_env(|env| env.tx_auth_info().fee.gas);
        if gas_limit < Self::CALL_GAS_SPECIFIC {
            Err(<GasWasterModule as module::Module>::Error::Forbidden)
        } else {
            Ok(())
        }
    }

    #[handler(call = Self::METHOD_SPECIFIC_GAS_REQUIRED_HUGE)]
    fn specific_gas_required_huge<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        // Fails with an error if less than X gas was specified. (doesn't fail with out-of-gas).
        let gas_limit = CurrentState::with_env(|env| env.tx_auth_info().fee.gas);
        if gas_limit < Self::CALL_GAS_SPECIFIC_HUGE {
            Err(<GasWasterModule as module::Module>::Error::Forbidden)
        } else {
            Ok(())
        }
    }

    #[handler(call = Self::METHOD_STORAGE_UPDATE)]
    fn storage_update<C: Context>(
        _ctx: &C,
        args: (Vec<u8>, Vec<u8>, u64),
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(args.2)?;
        CurrentState::with_store(|store| store.insert(&args.0, &args.1));
        Ok(())
    }

    #[handler(call = Self::METHOD_STORAGE_REMOVE)]
    fn storage_remove<C: Context>(
        _ctx: &C,
        args: Vec<u8>,
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(2)?;
        CurrentState::with_store(|store| store.remove(&args));
        Ok(())
    }

    #[handler(call = Self::METHOD_EMIT_CONSENSUS_MESSAGE)]
    fn emit_consensus_message<C: Context>(
        ctx: &C,
        count: u64,
    ) -> Result<(), <GasWasterModule as module::Module>::Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(2)?;
        CurrentState::with(|state| {
            for _ in 0..count {
                state.emit_message(
                    ctx,
                    roothash::Message::Staking(Versioned::new(
                        0,
                        roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    )),
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )?;
            }
            Ok(())
        })
    }
}

impl module::BlockHandler for GasWasterModule {}
impl module::TransactionHandler for GasWasterModule {}
impl module::InvariantHandler for GasWasterModule {}

struct Config;

impl super::Config for Config {
    const ESTIMATE_GAS_EXTRA_FAIL: Lazy<BTreeMap<&'static str, u64>> = Lazy::new(|| {
        [(
            GasWasterModule::METHOD_WASTE_GAS_AND_FAIL_EXTRA,
            GasWasterModule::CALL_GAS_EXTRA,
        )]
        .into()
    });
}

// Runtime that knows how to waste gas.
struct GasWasterRuntime;

impl GasWasterRuntime {
    const AUTH_SIGNATURE_GAS: u64 = 1;
    const AUTH_MULTISIG_GAS: u64 = 10;
}

impl Runtime for GasWasterRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core;
    type Accounts = crate::modules::accounts::Module;

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
                        storage_byte: 0,
                        auth_signature: Self::AUTH_SIGNATURE_GAS,
                        auth_multisig_signer: Self::AUTH_MULTISIG_GAS,
                        callformat_x25519_deoxysii: 0,
                    },
                    min_gas_price: {
                        let mut mgp = BTreeMap::new();
                        mgp.insert(token::Denomination::NATIVE, 0);
                        mgp
                    },
                    dynamic_min_gas_price: Default::default(),
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
    let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);

    GasWasterRuntime::migrate(&ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            ..Default::default()
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
                ..Default::default()
            },
            ..Default::default()
        },
    };

    Core::authenticate_tx(&ctx, &tx).expect("authenticate should pass if all modules accept");
}

#[test]
fn test_query_estimate_gas() {
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            ..Default::default()
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
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let mut tx_fail = tx.clone();
    tx_fail.call.method = GasWasterModule::METHOD_WASTE_GAS_AND_FAIL.to_owned();

    let mut tx_fail_extra = tx.clone();
    tx_fail_extra.call.method = GasWasterModule::METHOD_WASTE_GAS_AND_FAIL_EXTRA.to_owned();

    let mut tx_huge = tx.clone();
    tx_huge.call.method = GasWasterModule::METHOD_WASTE_GAS_HUGE.to_owned();

    let mut tx_caller_specific = tx.clone();
    tx_caller_specific.call.method = GasWasterModule::METHOD_WASTE_GAS_CALLER.to_owned();

    let mut tx_specific_gas = tx.clone();
    tx_specific_gas.call.method = GasWasterModule::METHOD_SPECIFIC_GAS_REQUIRED.to_owned();

    let mut tx_specific_gas_huge = tx.clone();
    tx_specific_gas_huge.call.method =
        GasWasterModule::METHOD_SPECIFIC_GAS_REQUIRED_HUGE.to_owned();

    // Gas that we expect transactions to use.
    let tx_reference_gas = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS;
    let tx_huge_reference_gas = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS_HUGE;
    let tx_specific_gas_reference_gas = GasWasterModule::CALL_GAS_SPECIFIC;
    let tx_specific_gas_huge_reference_gas = GasWasterModule::CALL_GAS_SPECIFIC_HUGE;
    let tx_caller_gas_addr_zero = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS_CALLER_ADDR_ZERO;
    let tx_caller_gas_addr_ethzero = GasWasterRuntime::AUTH_SIGNATURE_GAS
        + GasWasterRuntime::AUTH_MULTISIG_GAS
        + GasWasterModule::CALL_GAS_CALLER_ADDR_ETHZERO;

    CurrentState::init_local_fallback();

    CurrentState::with_transaction_opts(Options::new().with_mode(state::Mode::Check), || {
        // Test happy-path execution with default settings.
        {
            let mut mock = mock::Mock::default();
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);
            GasWasterRuntime::migrate(&ctx);

            // Test estimation with caller derived from the transaction.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");

            // Test estimation with specified caller.
            let args = types::EstimateGasQuery {
                caller: Some(CallerAddress::Address(keys::alice::address())),
                tx: tx.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");
        }

        // Test extra gas estimation.
        {
            let mut mock = mock::Mock::default();
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);
            GasWasterRuntime::migrate(&ctx);

            // Test estimation on failure.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_fail_extra.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est,
                tx_reference_gas + GasWasterModule::CALL_GAS_EXTRA,
                "estimated gas should be correct"
            );
        }

        // Test expensive estimates.
        {
            let max_estimated_gas = tx_reference_gas - 1;
            let local_config = configmap! {
                "core" => configmap! {
                    "max_estimated_gas" => max_estimated_gas,
                },
            };
            let mut mock = mock::Mock::with_local_config(local_config);
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);
            GasWasterRuntime::migrate(&ctx);

            // Test with limited max_estimated_gas.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx.clone(),
                propagate_failures: false,
            };
            let est = Core::query_estimate_gas(&ctx, args)
                .expect("query_estimate_gas should succeed even with limited max_estimated_gas");
            assert!(
                est <= max_estimated_gas,
                "estimated gas should be at most max_estimated_gas={}, was {}",
                max_estimated_gas,
                est
            );

            // Test with limited max_estimated_gas and propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx.clone(),
                propagate_failures: true,
            };
            let result = Core::query_estimate_gas(&ctx, args).expect_err(
            "query_estimate_gas should fail with limited max_estimated_gas and propagate failures enabled",
        );
            assert_eq!(result.module_name(), "core");
            assert_eq!(result.code(), 12);
            assert_eq!(
                result.to_string(),
                format!(
                    "out of gas (limit: {} wanted: {})",
                    max_estimated_gas, tx_reference_gas
                )
            );
        }

        // Test transactions that fail.
        {
            let mut mock = mock::Mock::default();
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);
            GasWasterRuntime::migrate(&ctx);

            // Test with propagate failures disabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_fail.clone(),
                propagate_failures: false,
            };
            let est = Core::query_estimate_gas(&ctx, args)
                .expect("query_estimate_gas should succeed even with a transaction that fails");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");

            // Test with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_fail.clone(),
                propagate_failures: true,
            };
            let result = Core::query_estimate_gas(&ctx, args)
            .expect_err("query_estimate_gas should fail with a transaction that fails and propagate failures enabled");
            assert_eq!(result.module_name(), "core");
            assert_eq!(result.code(), 22);
            assert_eq!(result.to_string(), "forbidden by node policy",);
        }

        // Test binary search of expensive transactions.
        {
            let local_config = configmap! {
                "core" => configmap! {
                    "estimate_gas_search_max_iters" => 64,
                },
            };
            let mut mock = mock::Mock::with_local_config(local_config);
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);
            GasWasterRuntime::migrate(&ctx);

            // Test tx estimation.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");

            // Test tx estimation with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx,
                propagate_failures: true,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");

            // Test a failing transaction with propagate failures disabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_fail.clone(),
                propagate_failures: false,
            };
            let est = Core::query_estimate_gas(&ctx, args)
                .expect("query_estimate_gas should succeed even with a transaction that fails");
            assert_eq!(est, tx_reference_gas, "estimated gas should be correct");

            // Test a failing transaction with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_fail,
                propagate_failures: true,
            };
            let result = Core::query_estimate_gas(&ctx, args)
            .expect_err("query_estimate_gas should fail with a transaction that fails and propagate failures enabled");
            assert_eq!(result.module_name(), "core");
            assert_eq!(result.code(), 22);
            assert_eq!(result.to_string(), "forbidden by node policy",);

            // Test huge tx estimation.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_huge.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_huge_reference_gas,
                "estimated gas should be correct"
            );

            // Test huge tx estimation with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_huge,
                propagate_failures: true,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_huge_reference_gas,
                "estimated gas should be correct"
            );

            // Test a transaction that requires specific amount of gas, but doesn't fail with out-of-gas.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_specific_gas.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_specific_gas_reference_gas,
                "estimated gas should be correct"
            );

            // Test a transaction that requires specific amount of gas, with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_specific_gas,
                propagate_failures: true,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_specific_gas_reference_gas,
                "estimated gas should be correct"
            );

            // Test a transaction that requires specific huge amount of gas, but doesn't fail with out-of-gas.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_specific_gas_huge.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_specific_gas_huge_reference_gas,
                "estimated gas should be correct"
            );

            // Test a transaction that requires specific amount of gas, with propagate failures enabled.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_specific_gas_huge,
                propagate_failures: true,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_specific_gas_huge_reference_gas,
                "estimated gas should be correct"
            );
        }

        // Test confidential estimation that should zeroize the caller.
        {
            let mut mock = mock::Mock::default();
            let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(true);
            GasWasterRuntime::migrate(&ctx);

            // Test estimation with caller derived from the transaction.
            let args = types::EstimateGasQuery {
                caller: None,
                tx: tx_caller_specific.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_caller_gas_addr_zero,
                "estimated gas should be correct"
            );

            // Test estimation with specified caller.
            let args = types::EstimateGasQuery {
                caller: Some(CallerAddress::Address(keys::alice::address())),
                tx: tx_caller_specific.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_caller_gas_addr_zero,
                "estimated gas should be correct"
            );

            // Test estimation with specified caller (eth address).
            let args = types::EstimateGasQuery {
                caller: Some(CallerAddress::EthAddress([42u8; 20])),
                tx: tx_caller_specific.clone(),
                propagate_failures: false,
            };
            let est =
                Core::query_estimate_gas(&ctx, args).expect("query_estimate_gas should succeed");
            assert_eq!(
                est, tx_caller_gas_addr_ethzero,
                "estimated gas should be correct"
            );
        }
    });
}

#[test]
fn test_approve_unverified_tx() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    Core::set_params(Parameters {
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
        dynamic_min_gas_price: Default::default(),
    });

    let dummy_bytes = b"you look, you die".to_vec();
    Core::approve_unverified_tx(
        &ctx,
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
        &ctx,
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
        &ctx,
        &transaction::UnverifiedTransaction(
            dummy_bytes.clone(),
            vec![
                transaction::AuthProof::Signature(dummy_bytes.into()),
                transaction::AuthProof::Multisig(vec![None, None, None]),
            ],
        ),
    )
    .expect_err("multisig too many signers");
}

#[test]
fn test_transaction_expiry() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "test.Test".to_owned(),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: Default::default(),
            not_before: Some(10),
            not_after: Some(42),
        },
    };

    // Authenticate transaction, should be expired.
    let err = Core::authenticate_tx(&ctx, &tx).expect_err("tx should be expired (early)");
    assert!(matches!(
        err,
        crate::modules::core::Error::ExpiredTransaction
    ));

    // Move the round forward.
    mock.runtime_header.round = 15;

    // Authenticate transaction, should succeed.
    let ctx = mock.create_ctx();
    Core::authenticate_tx(&ctx, &tx).expect("tx should be valid");

    // Move the round forward again.
    mock.runtime_header.round = 50;

    // Authenticate transaction, should be expired.
    let ctx = mock.create_ctx();
    let err = Core::authenticate_tx(&ctx, &tx).expect_err("tx should be expired");
    assert!(matches!(
        err,
        crate::modules::core::Error::ExpiredTransaction
    ));
}

#[test]
fn test_set_priority() {
    let _mock = mock::Mock::default();

    assert_eq!(0, Core::take_priority(), "default priority should be 0");

    Core::set_priority(1);
    Core::set_priority(11);

    CurrentState::with_transaction(|| {
        Core::set_priority(10);
    });

    assert_eq!(10, Core::take_priority(), "setting priority should work");
}

#[test]
fn test_set_sender_meta() {
    let _mock = mock::Mock::default();

    let sender_meta = SenderMeta {
        address: keys::alice::address(),
        tx_nonce: 42,
        state_nonce: 43,
    };
    Core::set_sender_meta(sender_meta.clone());

    let taken_sender_meta = Core::take_sender_meta();
    assert_eq!(
        taken_sender_meta, sender_meta,
        "setting sender metadata should work"
    );
}

#[test]
fn test_min_gas_price() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);

    Core::set_params(Parameters {
        max_batch_gas: u64::MAX,
        max_tx_size: 32 * 1024,
        max_tx_signers: 8,
        max_multisig_signers: 8,
        gas_costs: super::GasCosts {
            tx_byte: 0,
            storage_byte: 0,
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
        dynamic_min_gas_price: Default::default(),
    });

    let mut tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            ..Default::default()
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
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let call = tx.call.clone();

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::before_handle_call(&ctx, &call).expect_err("gas price should be too low");
    });

    tx.auth_info.fee.amount = token::BaseUnits::new(100000, token::Denomination::NATIVE);

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
        Core::before_handle_call(&ctx, &call).expect("gas price should be ok");
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

    CurrentState::with_transaction_opts(
        state::Options::new().with_mode(state::Mode::Check),
        || {
            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect_err("gas price should be too low");
            });

            tx.auth_info.fee.amount = token::BaseUnits::new(1_000_000, token::Denomination::NATIVE);

            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect("gas price should be ok");
            });

            tx.auth_info.fee.amount = token::BaseUnits::new(1_000, "SMALLER".parse().unwrap());

            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect_err("gas price should be too low");
            });

            tx.auth_info.fee.amount = token::BaseUnits::new(10_000, "SMALLER".parse().unwrap());

            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect("gas price should be ok");
            });

            // Test exempt methods.
            tx.call.method = "exempt.Method".into();
            tx.auth_info.fee.amount = token::BaseUnits::new(100_000, token::Denomination::NATIVE);
            let call = tx.call.clone();

            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect("method should be gas price exempt");
            });

            tx.auth_info.fee.amount = token::BaseUnits::new(0, token::Denomination::NATIVE);

            CurrentState::with_transaction_opts(Options::new().with_tx(tx.clone().into()), || {
                super::Module::<MinGasPriceOverride>::before_handle_call(&ctx, &call)
                    .expect("method should be gas price exempt");
            });
        },
    );
}

#[test]
fn test_gas_used_events() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    Core::set_params(Parameters {
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
        dynamic_min_gas_price: Default::default(),
    });

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = 100_000;

    CurrentState::with_transaction_opts(Options::new().with_tx(tx.into()), || {
        Core::use_tx_gas(10).expect("using gas under limit should succeed");
        assert_eq!(Core::used_tx_gas(), 10);
        Core::after_handle_call(
            &ctx,
            module::CallResult::Ok(cbor::Value::Simple(cbor::SimpleValue::NullValue)),
        )
        .expect("after_handle_call should succeed");

        let tags = CurrentState::with(|state| state.take_all_events().into_tags());
        assert_eq!(tags.len(), 1, "1 emitted tag expected");

        let expected = cbor::to_vec(vec![Event::GasUsed { amount: 10 }]);
        assert_eq!(tags[0].value, expected, "expected events emitted");
    });
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
    let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);

    // Set bogus params on the core module; we want to see them reflected in response to the `runtime_info()` query.
    let core_params = Parameters {
        max_batch_gas: 123,
        max_tx_signers: 4,
        max_multisig_signers: 567,
        ..Default::default()
    };
    Core::set_params(core_params.clone());

    let info = Core::query_runtime_info(&ctx, ()).unwrap();
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
                            MethodHandlerInfo { kind: MethodHandlerKind::Call, name: "core.CallDataPublicKey".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Call, name: "core.CurrentEpoch".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.MinGasPrice".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.RuntimeInfo".to_string() },
                            MethodHandlerInfo { kind: MethodHandlerKind::Query, name: "core.ExecuteReadOnlyTx".to_string() },
                        ]
                    },
                "gaswaster" =>
                    types::ModuleInfo {
                        version: 42,
                        params: ().into_cbor_value(),
                        methods: vec![
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGas".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGasAndFail".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGasAndFailExtra".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGasHuge".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.WasteGasCaller".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.SpecificGasRequired".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.SpecificGasRequiredHuge".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.StorageUpdate".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.StorageRemove".to_string() },
                            MethodHandlerInfo { kind: types::MethodHandlerKind::Call, name: "test.EmitConsensusMessage".to_string() },
                        ],
                    },
            }
        }
    );
}

#[test]
fn test_min_gas_price_update() {
    let cases: Vec<(u128, u128, u128, u128, u128)> = vec![
        // (gas_used, target_gas_used, price_max_change_denominator, price, expected_price)
        // General cases.
        (50, 50, 1, 100, 100),  // No change.
        (100, 50, 1, 100, 200), // Increase.
        (50, 100, 1, 100, 50),  // Decrease.
        (0, 50, 1, 100, 0),     // Decrease.
        // Non base price_max_change_denominator.
        (100, 50, 2, 100, 150),           // Increase by 50.
        (100, 50, 8, 100, 112),           // Increase by 12.
        (50, 100, 2, 100, 75),            // Decrease by 25.
        (50, 100, 8, 100, 94),            // Decrease by 6.
        (0, 100, 2, 100, 50),             // Decrease by 50.
        (0, 100, 8, 100, 88),             // Decrease by 12.
        (0, u64::MAX as u128, 1, 100, 0), // Decrease by 100%.
        // Invalid configurations (should be handled gracefully)
        (100, 100, 0, 100, 100), // price_max_change_denominator == 0.
        (100, 0, 1, 100, 100),   // target_gas_used == 0
        (1000, 100, 1, 0, 0),    // price == 0.
        (u128::MAX, u128::MAX, 1, u128::MAX, u128::MAX), // Overflow.
        (0, u128::MAX, 1, 100, 99), // Overflow (target_gas_used).
        (0, u128::MAX / 2, 1, 100, 98), // Overflow. (target_gas_used).
    ];
    for (i, (gas_used, target_gas, max_change_denominator, price, expected_price)) in
        cases.into_iter().enumerate()
    {
        let new_price = min_gas_price_update(gas_used, target_gas, max_change_denominator, price);
        assert_eq!(
            new_price, expected_price,
            "dynamic price should match expected price (test case: {:?})",
            i
        );
    }
}

#[test]
fn test_dynamic_min_gas_price() {
    let mut mock = mock::Mock::default();

    let denom: token::Denomination = "SMALLER".parse().unwrap();
    Core::set_params(Parameters {
        max_batch_gas: 10_000,
        max_tx_size: 32 * 1024,
        max_tx_signers: 8,
        max_multisig_signers: 8,
        gas_costs: super::GasCosts {
            tx_byte: 0,
            storage_byte: 0,
            auth_signature: GasWasterRuntime::AUTH_SIGNATURE_GAS,
            auth_multisig_signer: GasWasterRuntime::AUTH_MULTISIG_GAS,
            callformat_x25519_deoxysii: 0,
        },
        min_gas_price: {
            let mut mgp = BTreeMap::new();
            mgp.insert(token::Denomination::NATIVE, 1000);
            mgp.insert(denom.clone(), 100);
            mgp
        },
        dynamic_min_gas_price: super::DynamicMinGasPrice {
            enabled: true,
            target_block_gas_usage_percentage: 50,
            min_price_max_change_denominator: 8,
        },
    });

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_WASTE_GAS.to_owned(),
            ..Default::default()
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
                amount: token::BaseUnits::new(1_000_000_000, token::Denomination::NATIVE),
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let call = tx.call.clone();
    assert_eq!(
        Core::min_gas_price(&token::Denomination::NATIVE),
        Some(1000)
    );
    assert_eq!(Core::min_gas_price(&denom), Some(100));

    // Simulate some full blocks (with max gas usage).
    for round in 0..=10 {
        mock.runtime_header.round = round;

        let ctx = mock.create_ctx();
        CurrentState::with_transaction(|| {
            // Simulate a new block starting by starting with fresh per-block values.
            CurrentState::with(|state| state.hide_block_values());

            Core::begin_block(&ctx);

            for _ in 0..909 {
                // Each tx uses 11 gas, this makes it 9999/10_000 block gas used.
                CurrentState::with_transaction_opts(
                    Options::new().with_tx(tx.clone().into()),
                    || {
                        Core::before_handle_call(&ctx, &call).expect("gas price should be ok");
                    },
                );
            }

            Core::end_block(&ctx);
        });
    }

    assert_eq!(
        Core::min_gas_price(&token::Denomination::NATIVE),
        Some(3598) // Gas price should increase.
    );
    assert_eq!(Core::min_gas_price(&denom), Some(350));

    // Simulate some empty blocks.
    for round in 10..=100 {
        mock.runtime_header.round = round;

        let ctx = mock.create_ctx();
        Core::begin_block(&ctx);
        Core::end_block(&ctx);
    }

    assert_eq!(
        Core::min_gas_price(&token::Denomination::NATIVE),
        Some(1000) // Gas price should decrease to the configured min gas price.
    );
    assert_eq!(Core::min_gas_price(&denom), Some(100));
}

#[test]
fn test_storage_gas() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);

    GasWasterRuntime::migrate(&ctx);

    let storage_byte_cost = 1;
    Core::set_params(Parameters {
        max_batch_gas: 10_000,
        gas_costs: super::GasCosts {
            tx_byte: 0,
            storage_byte: storage_byte_cost,
            ..Default::default()
        },
        ..Core::params()
    });

    let mut signer = mock::Signer::new(0, keys::alice::sigspec());

    let key = b"foo".to_vec();
    let value = b"bar".to_vec();

    // Insert (non-storage gas smaller than storage gas).
    let expected_gas_use = (key.len() + value.len()) as u64 * storage_byte_cost;
    let dispatch_result = signer.call_opts(
        &ctx,
        GasWasterModule::METHOD_STORAGE_UPDATE,
        (key.clone(), value.clone(), 2), // Use 2 extra gas, make sure it is not charged.
        mock::CallOptions {
            fee: transaction::Fee {
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 1, "one event should have been emitted");
    assert_eq!(tags[0].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    #[derive(Debug, Default, cbor::Decode)]
    struct GasUsedEvent {
        amount: u64,
    }

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, expected_gas_use);

    // Insert (non-storage gas larger than storage gas)
    let expected_gas_use = 42; // No storage gas should be charged.
    let dispatch_result = signer.call_opts(
        &ctx,
        GasWasterModule::METHOD_STORAGE_UPDATE,
        (key.clone(), value.clone(), 42), // Use 42 extra gas, it should be charged.
        mock::CallOptions {
            fee: transaction::Fee {
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    let tags = &dispatch_result.tags;
    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, expected_gas_use);

    // Remove.
    let expected_gas_use = key.len() as u64 * storage_byte_cost;
    let dispatch_result = signer.call_opts(
        &ctx,
        GasWasterModule::METHOD_STORAGE_REMOVE,
        key,
        mock::CallOptions {
            fee: transaction::Fee {
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    let tags = &dispatch_result.tags;
    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, expected_gas_use);
}

#[test]
fn test_message_gas() {
    let mut mock = mock::Mock::default();
    let max_messages = 32;
    mock.max_messages = max_messages;

    let ctx = mock.create_ctx_for_runtime::<GasWasterRuntime>(false);

    GasWasterRuntime::migrate(&ctx);

    let max_batch_gas = 10_000;
    Core::set_params(Parameters {
        max_batch_gas,
        gas_costs: super::GasCosts {
            tx_byte: 0,
            ..Default::default()
        },
        ..Core::params()
    });

    let mut signer = mock::Signer::new(0, keys::alice::sigspec());

    // Emit 10 messages which is greater than the transaction compute gas cost.
    let num_messages = 10u64;
    let mut total_messages = num_messages;
    let expected_gas_use = num_messages * (max_batch_gas / (max_messages as u64));
    let dispatch_result = signer.call_opts(
        &ctx,
        GasWasterModule::METHOD_EMIT_CONSENSUS_MESSAGE,
        num_messages,
        mock::CallOptions {
            fee: transaction::Fee {
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Simulate multiple transactions in a batch by not taking any messages.

    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 1, "one event should have been emitted");
    assert_eq!(tags[0].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    #[derive(Debug, Default, cbor::Decode)]
    struct GasUsedEvent {
        amount: u64,
    }

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, expected_gas_use);

    // Emit no messages so just the compute gas cost should be charged.
    let num_messages = 0u64;
    total_messages += num_messages;
    let expected_gas_use = 2; // Just compute gas cost.
    let dispatch_result = signer.call_opts(
        &ctx,
        GasWasterModule::METHOD_EMIT_CONSENSUS_MESSAGE,
        num_messages,
        mock::CallOptions {
            fee: transaction::Fee {
                gas: 10_000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(dispatch_result.result.is_success(), "call should succeed");

    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 1, "one event should have been emitted");
    assert_eq!(tags[0].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just one gas used event.
    assert_eq!(events[0].amount, expected_gas_use);

    // Take all messages emitted by the above two transactions.
    let messages = CurrentState::with(|state| state.take_messages());
    assert_eq!(total_messages as usize, messages.len());

    // Ensure gas estimation works.
    let num_messages = 10;
    let expected_gas_use = num_messages * (max_batch_gas / (max_messages as u64));
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: GasWasterModule::METHOD_EMIT_CONSENSUS_MESSAGE.to_owned(),
            body: cbor::to_value(num_messages),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(0, token::Denomination::NATIVE),
                gas: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        },
    };
    let estimated_gas: u64 = signer
        .query(
            &ctx,
            "core.EstimateGas",
            types::EstimateGasQuery {
                caller: None,
                tx,
                propagate_failures: false,
            },
        )
        .expect("gas estimation should succeed");
    assert_eq!(
        estimated_gas, expected_gas_use,
        "gas should be estimated correctly"
    );
}
