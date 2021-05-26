use oasis_core_runtime::common::{cbor, version};

use crate::{
    context,
    context::Context,
    crypto::multisig,
    dispatcher, module,
    module::{AuthHandler as _, Module as _},
    runtime,
    testing::{keys, mock},
    types::{token, transaction},
};

use super::{GasCosts, Genesis, Module as Core, Parameters, API as _};

#[test]
fn test_use_gas() {
    const MAX_GAS: u64 = 1000;
    const BLOCK_MAX_GAS: u64 = 3 * MAX_GAS + 2;
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        &super::Parameters {
            max_batch_gas: BLOCK_MAX_GAS,
            max_tx_signers: 8,
            max_multisig_signers: 8,
            gas_costs: Default::default(),
        },
    );

    Core::use_gas(&mut ctx, 1).expect("using batch gas under limit should succeed");

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = MAX_GAS;

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS).expect("using gas under limit should succeed");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS)
            .expect("gas across separate transactions shouldn't accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS).unwrap();
        Core::use_gas(&mut tx_ctx, 1).expect_err("gas in same transaction should accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 1).unwrap();
        Core::use_gas(&mut tx_ctx, u64::MAX).expect_err("overflow should cause error");
    });

    let mut big_tx = tx.clone();
    big_tx.auth_info.fee.gas = u64::MAX;
    ctx.with_tx(big_tx, |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, u64::MAX).expect_err("batch overflow should cause error");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 1).expect_err("batch gas should accumulate");
    });

    Core::use_gas(&mut ctx, 1).expect_err("batch gas should accumulate outside tx");
}

const AUTH_SIGNATURE_GAS: u64 = 1;
const AUTH_MULTISIG_GAS: u64 = 10;
const CALL_GAS: u64 = 100;

struct Runtime1;

impl runtime::Runtime for Runtime1 {
    const VERSION: version::Version = version::Version {
        major: 1,
        minor: 0,
        patch: 0,
    };
    type Modules = (Core,);

    fn genesis_state() -> (Genesis,) {
        (Genesis {
            parameters: Parameters {
                max_batch_gas: u64::MAX,
                max_tx_signers: 8,
                max_multisig_signers: 8,
                gas_costs: GasCosts {
                    auth_signature: AUTH_SIGNATURE_GAS,
                    auth_multisig_signer: AUTH_MULTISIG_GAS,
                },
            },
        },)
    }
}

#[test]
fn test_query_estimate_gas() {
    const METHOD_WASTE_GAS: &str = "test.WasteGas";
    let mut methods = module::MethodRegistry::new();
    methods.register_callable(module::CallableMethodInfo {
        name: METHOD_WASTE_GAS,
        handler: |_mi, ctx, _args| {
            Core::use_gas(ctx, CALL_GAS).expect("use_gas should succeed");
            transaction::CallResult::Ok(cbor::Value::Null)
        },
    });
    let consensus_message_handlers = module::MessageHandlerRegistry::new();
    let dispatcher = dispatcher::Dispatcher::<Runtime1>::new(methods, consensus_message_handlers);

    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    ctx.mode = context::Mode::CheckTx;
    dispatcher.maybe_init_state(&mut ctx);

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Null,
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
                amount: token::BaseUnits::new(0.into(), token::Denomination::NATIVE),
                gas: u64::MAX,
            },
        },
    };

    let est =
        Core::query_estimate_gas(&mut ctx, &dispatcher, tx).expect("estimate_gas should succeed");
    let reference_gas = AUTH_SIGNATURE_GAS + AUTH_MULTISIG_GAS + CALL_GAS;
    assert_eq!(est, reference_gas, "estimated gas should be correct");
}

#[test]
fn test_approve_unverified_tx() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    Core::set_params(
        ctx.runtime_state(),
        &super::Parameters {
            max_batch_gas: u64::MAX,
            max_tx_signers: 2,
            max_multisig_signers: 2,
            gas_costs: Default::default(),
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
