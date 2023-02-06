//! Tests for the EVM module.
use std::collections::BTreeMap;

use sha3::Digest as _;
use uint::hex::FromHex;

use oasis_runtime_sdk::{
    callformat, context,
    crypto::{self, signature::secp256k1},
    error::Error as _,
    module::{self, InvariantHandler as _, TransactionHandler as _},
    modules::{
        accounts::{self, Module as Accounts},
        core::{self, Module as Core},
    },
    testing::{keys, mock},
    types::{address::SignatureAddressSpec, token::Denomination, transaction},
    BatchContext, Context, Runtime, Version,
};

use crate::{
    derive_caller, process_evm_result,
    types::{self, H160},
    Config, Error, Genesis, Module as EVMModule,
};

/// Test contract code.
static TEST_CONTRACT_CODE_HEX: &str =
    include_str!("../../../../tests/e2e/contracts/evm_erc20_test_compiled.hex");

pub(crate) struct EVMConfig;

impl Config for EVMConfig {
    type Accounts = Accounts;

    const CHAIN_ID: u64 = 0xa515;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;
}

pub(crate) struct ConfidentialEVMConfig;

impl Config for ConfidentialEVMConfig {
    type Accounts = Accounts;

    const CHAIN_ID: u64 = 0x5afe;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;

    const CONFIDENTIAL: bool = true;
}

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

fn do_test_evm_calls<C: Config>(force_plain: bool) {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let client_keypair =
        oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::generate_key_pair();

    macro_rules! encode_data {
        ($data:expr) => {
            if C::CONFIDENTIAL && !force_plain {
                cbor::to_vec(
                    callformat::encode_call(
                        &ctx,
                        transaction::Call {
                            format: transaction::CallFormat::EncryptedX25519DeoxysII,
                            method: "".into(),
                            body: cbor::Value::from($data),
                            ..Default::default()
                        },
                        &client_keypair,
                    )
                    .unwrap(),
                )
            } else {
                $data
            }
        };
    }

    macro_rules! decode_result {
        ($tx_ctx:ident, $result:expr$(,)?) => {
            match $result {
                Ok(evm_result) => {
                    if C::CONFIDENTIAL && !force_plain {
                        let call_result: transaction::CallResult =
                            cbor::from_slice(&evm_result).unwrap();
                        callformat::decode_result(
                            &$tx_ctx,
                            transaction::CallFormat::EncryptedX25519DeoxysII,
                            call_result,
                            &client_keypair,
                        )
                        .expect("bad decode")
                    } else {
                        module::CallResult::Ok(cbor::Value::from(evm_result))
                    }
                }
                Err(e) => e.into_call_result(),
            }
        };
    }

    Core::<CoreConfig>::init(
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

    EVMModule::<C>::init(
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
                init_code: encode_data!(erc20),
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
                gas: 1000000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    Accounts::authenticate_tx(&mut ctx, &create_tx).unwrap();

    let erc20_addr = ctx.with_tx(0, 0, create_tx, |mut tx_ctx, call| {
        let addr = H160::from_slice(
            &EVMModule::<C>::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap()).unwrap(),
        );
        EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");
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
                data: encode_data!(name_method),
            }),
            ..Default::default()
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
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    Accounts::authenticate_tx(&mut ctx, &call_name_tx).unwrap();

    let erc20_name = ctx.with_tx(0, 0, call_name_tx, |mut tx_ctx, call| {
        let name: Vec<u8> = cbor::from_value(
            decode_result!(
                tx_ctx,
                EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            )
            .unwrap(),
        )
        .unwrap();

        EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");

        tx_ctx.commit();

        name
    });
    assert_eq!(erc20_name.len(), 96);
    assert_eq!(erc20_name[63], 0x04); // Name is 4 bytes long.
    assert_eq!(erc20_name[64..68], vec![0x54, 0x65, 0x73, 0x74]); // "Test".
}

#[test]
fn test_evm_calls() {
    do_test_evm_calls::<EVMConfig>(false);
}

#[test]
fn test_c10l_evm_calls_enc() {
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_calls::<ConfidentialEVMConfig>(false);
}

#[test]
fn test_c10l_evm_calls_plain() {
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_calls::<ConfidentialEVMConfig>(true /* force_plain */);
}

#[test]
fn test_c10l_evm_balance_transfer() {
    crypto::signature::context::set_chain_context(Default::default(), "test");
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::<CoreConfig>::init(
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
            balances: BTreeMap::from([(
                keys::dave::address(),
                BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
            )]),
            total_supplies: BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
            ..Default::default()
        },
    );

    EVMModule::<ConfidentialEVMConfig>::init(
        &mut ctx,
        Genesis {
            parameters: Default::default(),
        },
    );

    let recipient = ethabi::Address::repeat_byte(42);
    let transfer_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Call".to_owned(),
            body: cbor::to_value(types::Call {
                address: recipient.into(),
                value: 12345u64.into(),
                data: vec![],
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
                gas: 1000000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    Accounts::authenticate_tx(&mut ctx, &transfer_tx).unwrap();

    ctx.with_tx(0, 0, transfer_tx, |mut tx_ctx, call| {
        EVMModule::<ConfidentialEVMConfig>::tx_call(
            &mut tx_ctx,
            cbor::from_value(call.body).unwrap(),
        )
        .unwrap();
        EVMModule::<ConfidentialEVMConfig>::check_invariants(&mut tx_ctx)
            .expect("invariants should hold");
        tx_ctx.commit();
    });

    let recipient_balance = EVMModule::<ConfidentialEVMConfig>::query_balance(
        &mut ctx,
        types::BalanceQuery {
            address: recipient.into(),
        },
    )
    .unwrap();
    assert_eq!(recipient_balance, 12345u64.into());
}

#[test]
fn test_c10l_enc_call_identity_decoded() {
    // Calls sent using the Oasis encrypted envelope format (not inner-enveloped)
    // should not be decoded:
    let mut mock = mock::Mock::default();
    let ctx =
        mock.create_ctx_for_runtime::<EVMRuntime<ConfidentialEVMConfig>>(context::Mode::ExecuteTx);
    let data = vec![1, 2, 3, 4, 5];
    let (decoded_data, metadata) = EVMModule::<ConfidentialEVMConfig>::decode_call_data(
        &ctx,
        data.clone(),
        transaction::CallFormat::EncryptedX25519DeoxysII,
        0,
        true,
    )
    .expect("decode failed")
    .expect("km is unreachable");
    assert_eq!(data, decoded_data);
    assert!(matches!(metadata, callformat::Metadata::Empty));
}

struct CoreConfig;

impl core::Config for CoreConfig {}

/// EVM test runtime.
struct EVMRuntime<C>(C);

impl<C: Config> Runtime for EVMRuntime<C> {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core<CoreConfig>;

    type Modules = (Core<CoreConfig>, Accounts, EVMModule<C>);

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

fn do_test_evm_runtime<C: Config>() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<EVMRuntime<C>>(context::Mode::ExecuteTx);
    let client_keypair =
        oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::generate_key_pair();

    // This is a macro to avoid mucking with borrow scopes.
    macro_rules! encode_data {
        ($data:expr) => {
            if C::CONFIDENTIAL {
                cbor::to_vec(
                    callformat::encode_call(
                        &ctx,
                        transaction::Call {
                            format: transaction::CallFormat::EncryptedX25519DeoxysII,
                            method: "".into(),
                            body: cbor::Value::from($data),
                            ..Default::default()
                        },
                        &client_keypair,
                    )
                    .unwrap(),
                )
            } else {
                $data
            }
        };
    }

    macro_rules! decode_result {
        ($tx_ctx:ident, $result:expr$(,)?) => {
            match $result {
                Ok(evm_result) => {
                    if C::CONFIDENTIAL {
                        let call_result: transaction::CallResult =
                            cbor::from_slice(&evm_result).unwrap();
                        callformat::decode_result(
                            &$tx_ctx,
                            transaction::CallFormat::EncryptedX25519DeoxysII,
                            call_result,
                            &client_keypair,
                        )
                        .expect("bad decode")
                    } else {
                        module::CallResult::Ok(cbor::Value::from(evm_result))
                    }
                }
                Err(e) => e.into_call_result(),
            }
        };
    }

    EVMRuntime::<C>::migrate(&mut ctx);

    let erc20 = load_erc20();

    // Test the Create transaction.
    let create_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "evm.Create".to_owned(),
            body: cbor::to_value(types::Create {
                value: 0.into(),
                init_code: encode_data!(erc20.clone()),
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
                gas: 1000000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&mut ctx, &create_tx).unwrap();

    let erc20_addr = ctx.with_tx(0, 0, create_tx, |mut tx_ctx, call| {
        let addr = H160::from_slice(
            &EVMModule::<C>::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap()).unwrap(),
        );
        EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");
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
                init_code: encode_data!(erc20),
            }),
            ..Default::default()
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
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&mut ctx, &out_of_gas_create).unwrap();

    ctx.with_tx(0, 0, out_of_gas_create.clone(), |mut tx_ctx, call| {
        assert!(!decode_result!(
            tx_ctx,
            EVMModule::<C>::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
        )
        .is_success());
    });

    // CheckTx should not fail.
    ctx.with_child(context::Mode::CheckTx, |mut check_ctx| {
        check_ctx.with_tx(0, 0, out_of_gas_create, |mut tx_ctx, call| {
            let rsp = EVMModule::<C>::tx_create(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed with empty result");

            assert_eq!(
                rsp,
                Vec::<u8>::new(),
                "check tx should return an empty response"
            );
        });
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
                data: encode_data!(name_method),
            }),
            ..Default::default()
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
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&mut ctx, &call_name_tx).unwrap();

    // Test transaction call in simulate mode.
    ctx.with_child(context::Mode::SimulateTx, |mut sim_ctx| {
        let erc20_name = sim_ctx.with_tx(0, 0, call_name_tx.clone(), |mut tx_ctx, call| {
            let name: Vec<u8> = cbor::from_value(
                decode_result!(
                    tx_ctx,
                    EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                )
                .unwrap(),
            )
            .unwrap();

            EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");

            tx_ctx.commit();

            name
        });
        assert_eq!(erc20_name.len(), 96);
        assert_eq!(erc20_name[63], 0x04); // Name is 4 bytes long.
        assert_eq!(erc20_name[64..68], vec![0x54, 0x65, 0x73, 0x74]); // "Test".
    });

    let erc20_name = ctx.with_tx(0, 0, call_name_tx.clone(), |mut tx_ctx, call| {
        let name: Vec<u8> = cbor::from_value(
            decode_result!(
                tx_ctx,
                EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            )
            .unwrap(),
        )
        .unwrap();

        EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");

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
                data: encode_data!(transfer_method.clone()),
            }),
            ..Default::default()
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
            ..Default::default()
        },
    };
    // Run authentication handler to simulate nonce increments.
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&mut ctx, &call_transfer_tx).unwrap();

    let transfer_ret = ctx.with_tx(0, 0, call_transfer_tx.clone(), |mut tx_ctx, call| {
        let ret: Vec<u8> = cbor::from_value(
            decode_result!(
                tx_ctx,
                EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            )
            .unwrap(),
        )
        .unwrap();

        EVMModule::<C>::check_invariants(&mut tx_ctx).expect("invariants should hold");

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
                data: encode_data!(transfer_method),
            }),
            ..Default::default()
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
            ..Default::default()
        },
    };
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&mut ctx, &out_of_gas_tx).unwrap();

    ctx.with_tx(0, 0, out_of_gas_tx.clone(), |mut tx_ctx, call| {
        assert!(!decode_result!(
            tx_ctx,
            EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
        )
        .is_success());
    });

    // CheckTx should not fail.
    ctx.with_child(context::Mode::CheckTx, |mut check_ctx| {
        check_ctx.with_tx(0, 0, out_of_gas_tx, |mut tx_ctx, call| {
            let rsp = EVMModule::<C>::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed with empty result");

            assert_eq!(
                rsp,
                Vec::<u8>::new(),
                "check tx should return an empty response"
            )
        });
    });
}

#[test]
fn test_evm_runtime() {
    do_test_evm_runtime::<EVMConfig>();
}

#[test]
fn test_c10l_evm_runtime() {
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_runtime::<ConfidentialEVMConfig>();
}

#[test]
fn test_revert_reason_decoding() {
    let long_reason = vec![0x61; 1050];
    let long_reason_hex = hex::encode(&long_reason);
    let long_reason_str = String::from_utf8(long_reason).unwrap();
    let long_reason_hex = &[
        "08c379a0\
        0000000000000000000000000000000000000000000000000000000000000020\
        000000000000000000000000000000000000000000000000000000000000041a",
        &long_reason_hex,
    ]
    .concat();

    let tcs = vec![
        // Valid values.
        (
            "08c379a0\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000018\
            4461692f696e73756666696369656e742d62616c616e63650000000000000000",
            "Dai/insufficient-balance",
        ),
        (
            "08c379a0\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000047\
            6d7946756e6374696f6e206f6e6c79206163636570747320617267756d656e74\
            7320776869636820617265206772656174686572207468616e206f7220657175\
            616c20746f203500000000000000000000000000000000000000000000000000",
            "myFunction only accepts arguments which are greather than or equal to 5",
        ),
        // Valid value, empty reason.
        (
            "08c379a0\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000000",
            "",
        ),
        // Valid value, reason too long and should be truncated.
        (long_reason_hex, &long_reason_str[..1024]),
        // No revert reason.
        ("", "no revert reason"),
        // Malformed output, incorrect selector and bad length.
        (
            "BADBADBADBADBADBAD",
            "invalid reason prefix: 'utututututut'",
        ),
        // Malformed output, bad selector.
        (
            "BAAAAAAD\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000018\
            4461692f696e73756666696369656e742d62616c616e63650000000000000000",
            "invalid reason prefix: 'uqqqrQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABhEYWkvaW5zdWZmaWNpZW50LWJhbGFuY2UAAAAAAAAAAA=='",
        ),
        // Malformed output, corrupted length.
        (
            "08c379a0\
            0000000000000000000000000000000000000000000000000000000000000020\
            00000000000000000000000000000000000000000000000000000000FFFFFFFF\
            4461692f696e73756666696369656e742d62616c616e63650000000000000000",
            "invalid reason length: 'CMN5oAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP////9EYWkvaW5zdWZmaWNpZW50LWJhbGFuY2UAAAAAAAAAAA=='",
        ),
    ];

    for tc in tcs {
        let raw = hex::decode(tc.0).unwrap();
        let err = process_evm_result(evm::ExitReason::Revert(evm::ExitRevert::Reverted), raw)
            .unwrap_err();
        match err {
            Error::Reverted(reason) => {
                assert_eq!(&reason, tc.1, "revert reason should be decoded correctly");
            }
            _ => panic!("expected Error::Reverted(_) variant"),
        }
    }
}
