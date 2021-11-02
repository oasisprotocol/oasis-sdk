//! Tests for the EVM module.
use std::collections::BTreeMap;

use sha3::Digest as _;
use uint::hex::FromHex;

use oasis_runtime_sdk::{
    context,
    crypto::signature::secp256k1,
    module::{self, AuthHandler as _, InvariantHandler as _},
    modules::{
        accounts::{self, Module as Accounts},
        core::{self, Module as Core},
    },
    testing::{keys, mock},
    types::{address::SignatureAddressSpec, token::Denomination, transaction},
    BatchContext, Context, Runtime, Version,
};

use crate::{
    derive_caller,
    types::{self, H160},
    Config, Genesis, Module as EVMModule,
};

/// Test contract code.
static TEST_CONTRACT_CODE_HEX: &str =
    include_str!("../../../../tests/e2e/contracts/evm_erc20_test_compiled.hex");

struct EVMConfig;

impl Config for EVMConfig {
    type Accounts = Accounts;

    const CHAIN_ID: u64 = 0xa515;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;
}

type EVM = EVMModule<EVMConfig>;

fn load_erc20() -> Vec<u8> {
    Vec::from_hex(
        TEST_CONTRACT_CODE_HEX
            .split_whitespace()
            .collect::<String>(),
    )
    .expect("compiled ERC20 contract should be a valid hex string")
}

fn check_derivation(seed: &str, priv_hex: &str, addr_hex: &str) {
    let priv_bytes = sha3::Keccak256::digest(seed.as_bytes());
    assert_eq!(
        priv_bytes.as_slice(),
        Vec::from_hex(priv_hex).unwrap().as_slice()
    );
    let priv_key = k256::ecdsa::SigningKey::from_bytes(&priv_bytes).unwrap();
    let pub_key = priv_key.verifying_key();
    let sdk_pub_key =
        secp256k1::PublicKey::from_bytes(k256::EncodedPoint::from(&pub_key).as_bytes()).unwrap();
    let addr =
        derive_caller::from_sigspec(&SignatureAddressSpec::Secp256k1Eth(sdk_pub_key)).unwrap();
    assert_eq!(addr.as_bytes(), Vec::from_hex(addr_hex).unwrap().as_slice());
}

#[test]
fn test_evm_caller_addr_derivation() {
    // https://github.com/ethereum/tests/blob/v10.0/BasicTests/keyaddrtest.json
    check_derivation(
        "cow",
        "c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4",
        "cd2a3d9f938e13cd947ec05abc7fe734df8dd826",
    );
    check_derivation(
        "horse",
        "c87f65ff3f271bf5dc8643484f66b200109caffe4bf98c4cb393dc35740b28c0",
        "13978aee95f38490e9769c39b2773ed763d9cd5f",
    );

    let expected =
        H160::from_slice(&Vec::<u8>::from_hex("dce075e1c39b1ae0b75d554558b6451a226ffe00").unwrap());
    let derived = derive_caller::from_sigspec(&keys::dave::sigspec()).unwrap();
    assert_eq!(derived, expected);
}

#[test]
fn test_evm_calls() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::init(
        &mut ctx,
        core::Genesis {
            parameters: core::Parameters {
                max_batch_gas: 10_000_000,
                ..Default::default()
            },
        },
    );

    Accounts::init(
        &mut ctx,
        accounts::Genesis {
            balances: {
                let mut b = BTreeMap::new();
                // Dave.
                b.insert(keys::dave::address(), {
                    let mut d = BTreeMap::new();
                    d.insert(Denomination::NATIVE, 1_000_000);
                    d
                });
                b
            },
            total_supplies: {
                let mut ts = BTreeMap::new();
                ts.insert(Denomination::NATIVE, 1_000_000);
                ts
            },
            ..Default::default()
        },
    );

    EVM::init(
        &mut ctx,
        Genesis {
            parameters: Default::default(),
        },
    );

    let erc20 = load_erc20();

    // Test the Create transaction.
    let create_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Create".to_owned(),
            body: cbor::to_value(types::Create {
                value: 0.into(),
                init_code: erc20.clone(),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000000,
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    Accounts::authenticate_tx(&mut ctx, &create_tx).unwrap();

    let erc20_addr = ctx.with_tx(0, create_tx, |mut tx_ctx, call| {
        let addr = H160::from_slice(
            &EVM::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("create should succeed"),
        );

        EVM::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        addr
    });

    // Test the Call transaction.
    let name_method: Vec<u8> = Vec::from_hex("06fdde03".to_owned() + &"0".repeat(64 - 8)).unwrap();
    let call_name_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Call".to_owned(),
            body: cbor::to_value(types::Call {
                address: erc20_addr,
                value: 0.into(),
                data: name_method,
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                1,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 25000,
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    Accounts::authenticate_tx(&mut ctx, &call_name_tx).unwrap();

    let erc20_name = ctx.with_tx(0, call_name_tx, |mut tx_ctx, call| {
        let name = EVM::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call name should succeed");

        EVM::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        name
    });
    assert_eq!(erc20_name.len(), 96);
    assert_eq!(erc20_name[63], 0x04); // Name is 4 bytes long.
    assert_eq!(erc20_name[64..68], vec![0x54, 0x65, 0x73, 0x74]); // "Test".
}

/// EVM test runtime.
struct EVMRuntime;

impl Runtime for EVMRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Modules = (Core, Accounts, EVM);

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: 10_000_000,
                    ..Default::default()
                },
            },
            accounts::Genesis {
                balances: {
                    let mut b = BTreeMap::new();
                    // Dave.
                    b.insert(keys::dave::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 1_000_000);
                        d
                    });
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 1_000_000);
                    ts
                },
                ..Default::default()
            },
            Genesis {
                parameters: Default::default(),
            },
        )
    }
}

#[test]
fn test_evm_runtime() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<EVMRuntime>(context::Mode::ExecuteTx);
    let mut mock = mock::Mock::default();
    let mut check_ctx = mock.create_ctx_for_runtime::<EVMRuntime>(context::Mode::CheckTx);

    EVMRuntime::migrate(&mut ctx);

    let erc20 = load_erc20();

    // Test the Create transaction.
    let create_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Create".to_owned(),
            body: cbor::to_value(types::Create {
                value: 0.into(),
                init_code: erc20.clone(),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1000000,
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut ctx, &create_tx).unwrap();
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut check_ctx, &create_tx).unwrap();

    let erc20_addr = ctx.with_tx(0, create_tx, |mut tx_ctx, call| {
        let addr = H160::from_slice(
            &EVM::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("create should succeed"),
        );

        EVM::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        addr
    });

    // Submitting an invalid create transaction should fail.
    let out_of_gas_create = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Create".to_owned(),
            body: cbor::to_value(types::Create {
                value: 0.into(),
                init_code: erc20,
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                1,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 10, // Not enough gas.
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut ctx, &out_of_gas_create).unwrap();
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut check_ctx, &out_of_gas_create).unwrap();

    ctx.with_tx(0, out_of_gas_create.clone(), |mut tx_ctx, call| {
        EVM::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("call transfer should fail");
    });

    // CheckTx should not fail.
    check_ctx.with_tx(0, out_of_gas_create, |mut tx_ctx, call| {
        let rsp = EVM::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed with empty result");

        assert_eq!(
            rsp,
            Vec::<u8>::new(),
            "check tx should return an empty response"
        )
    });

    // Test the Call transaction.
    let name_method: Vec<u8> = Vec::from_hex("06fdde03".to_owned() + &"0".repeat(64 - 8)).unwrap();
    let call_name_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Call".to_owned(),
            body: cbor::to_value(types::Call {
                address: erc20_addr,
                value: 0.into(),
                data: name_method,
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                2,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 25000,
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut ctx, &call_name_tx).unwrap();
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut check_ctx, &call_name_tx).unwrap();

    let erc20_name = ctx.with_tx(0, call_name_tx.clone(), |mut tx_ctx, call| {
        let name = EVM::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call name should succeed");

        EVM::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        name
    });
    assert_eq!(erc20_name.len(), 96);
    assert_eq!(erc20_name[63], 0x04); // Name is 4 bytes long.
    assert_eq!(erc20_name[64..68], vec![0x54, 0x65, 0x73, 0x74]); // "Test".

    // Test the Call transaction with more complicated parameters
    // (transfer 0x1000 coins to 0xc001d00d).
    let transfer_method: Vec<u8> = Vec::from_hex(
        "a9059cbb".to_owned()
            + &"0".repeat(64 - 4)
            + &"1000".to_owned()
            + &"0".repeat(64 - 8)
            + &"c001d00d".to_owned(),
    )
    .unwrap();
    let call_transfer_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Call".to_owned(),
            body: cbor::to_value(types::Call {
                address: erc20_addr,
                value: 0.into(),
                data: transfer_method.clone(),
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                3,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 64000,
                consensus_messages: 0,
            },
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut ctx, &call_transfer_tx).unwrap();
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut check_ctx, &call_transfer_tx).unwrap();

    let transfer_ret = ctx.with_tx(0, call_transfer_tx.clone(), |mut tx_ctx, call| {
        let ret = EVM::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call transfer should succeed");

        EVM::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        ret
    });
    assert_eq!(
        transfer_ret,
        Vec::<u8>::from_hex("0".repeat(64 - 1) + &"1".to_owned()).unwrap()
    ); // OK.

    // Submitting an invalid call transaction should fail.
    let out_of_gas_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Call".to_owned(),
            body: cbor::to_value(types::Call {
                address: erc20_addr,
                value: 0.into(),
                data: transfer_method,
            }),
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::dave::sigspec(),
                4,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 10, // Not enough gas.
                consensus_messages: 0,
            },
        },
    };
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut ctx, &out_of_gas_tx).unwrap();
    <EVMRuntime as Runtime>::Modules::authenticate_tx(&mut check_ctx, &out_of_gas_tx).unwrap();

    ctx.with_tx(0, out_of_gas_tx.clone(), |mut tx_ctx, call| {
        EVM::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("call transfer should fail");
    });

    // CheckTx should not fail.
    check_ctx.with_tx(0, out_of_gas_tx, |mut tx_ctx, call| {
        let rsp = EVM::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed with empty result");

        assert_eq!(
            rsp,
            Vec::<u8>::new(),
            "check tx should return an empty response"
        )
    });
}
