//! Tests for the EVM module.
use std::collections::BTreeMap;

use ethabi::{ParamType, Token};
use sha3::Digest as _;
use uint::hex::FromHex;

use oasis_runtime_sdk::{
    callformat,
    crypto::{self, signature::secp256k1},
    error::Error as _,
    module::{self, InvariantHandler as _, TransactionHandler as _},
    modules::{
        accounts::{self, Module as Accounts, ADDRESS_FEE_ACCUMULATOR, API as _},
        core::{self, Module as Core},
    },
    state::{self, CurrentState, Mode, Options, TransactionResult},
    testing::{keys, mock, mock::CallOptions},
    types::{
        address::{Address, SignatureAddressSpec},
        token::{self, Denomination},
        transaction,
        transaction::Fee,
    },
    Runtime, Version,
};

use crate::{
    derive_caller,
    mock::{decode_reverted, decode_reverted_raw, load_contract_bytecode, EvmSigner, QueryOptions},
    types::{self, H160},
    Config, Genesis, Module as EVMModule,
};

/// Test contract code.
static TEST_CONTRACT_CODE_HEX: &str =
    include_str!("../../../../tests/e2e/contracts/evm_erc20_test_compiled.hex");

pub(crate) struct EVMConfig;

impl Config for EVMConfig {
    type Accounts = Accounts;

    type AdditionalPrecompileSet = ();

    const CHAIN_ID: u64 = 0xa515;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;
}

pub(crate) struct ConfidentialEVMConfig;

impl Config for ConfidentialEVMConfig {
    type Accounts = Accounts;

    type AdditionalPrecompileSet = ();

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
        secp256k1::PublicKey::from_bytes(pub_key.to_encoded_point(true).as_bytes()).unwrap();
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
    let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(true);
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

    Core::<CoreConfig>::init(core::Genesis {
        parameters: core::Parameters {
            max_batch_gas: 10_000_000,
            ..Default::default()
        },
    });

    Accounts::init(accounts::Genesis {
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
    });

    EVMModule::<C>::init(Genesis {
        parameters: Default::default(),
    });

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
    Accounts::authenticate_tx(&ctx, &create_tx).unwrap();

    let call = create_tx.call.clone();
    let erc20_addr =
        CurrentState::with_transaction_opts(Options::new().with_tx(create_tx.into()), || {
            let addr = H160::from_slice(
                &EVMModule::<C>::tx_create(&ctx, cbor::from_value(call.body).unwrap()).unwrap(),
            );
            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            TransactionResult::Commit(addr)
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
    Accounts::authenticate_tx(&ctx, &call_name_tx).unwrap();

    let call = call_name_tx.call.clone();
    let erc20_name =
        CurrentState::with_transaction_opts(Options::new().with_tx(call_name_tx.into()), || {
            let name: Vec<u8> = cbor::from_value(
                decode_result!(
                    ctx,
                    EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
                )
                .unwrap(),
            )
            .unwrap();

            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            TransactionResult::Commit(name)
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
    let _guard = crypto::signature::context::test_using_chain_context();
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_calls::<ConfidentialEVMConfig>(false);
}

#[test]
fn test_c10l_evm_calls_plain() {
    let _guard = crypto::signature::context::test_using_chain_context();
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_calls::<ConfidentialEVMConfig>(true /* force_plain */);
}

#[test]
fn test_c10l_evm_balance_transfer() {
    let _guard = crypto::signature::context::test_using_chain_context();
    crypto::signature::context::set_chain_context(Default::default(), "test");
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx();

    Core::<CoreConfig>::init(core::Genesis {
        parameters: core::Parameters {
            max_batch_gas: 10_000_000,
            ..Default::default()
        },
    });

    Accounts::init(accounts::Genesis {
        balances: BTreeMap::from([(
            keys::dave::address(),
            BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
        )]),
        total_supplies: BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
        ..Default::default()
    });

    EVMModule::<ConfidentialEVMConfig>::init(Genesis {
        parameters: Default::default(),
    });

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
    Accounts::authenticate_tx(&ctx, &transfer_tx).unwrap();

    let call = transfer_tx.call.clone();
    CurrentState::with_transaction_opts(Options::new().with_tx(transfer_tx.into()), || {
        EVMModule::<ConfidentialEVMConfig>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
            .unwrap();
        EVMModule::<ConfidentialEVMConfig>::check_invariants(&ctx).expect("invariants should hold");
    });

    let recipient_balance = EVMModule::<ConfidentialEVMConfig>::query_balance(
        &ctx,
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
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<ConfidentialEVMConfig>>(true);
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
                    min_gas_price: BTreeMap::from([(token::Denomination::NATIVE, 0)]),
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
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<C>>(true);
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

    EVMRuntime::<C>::migrate(&ctx);

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
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&ctx, &create_tx).unwrap();

    let call = create_tx.call.clone();
    let erc20_addr =
        CurrentState::with_transaction_opts(Options::new().with_tx(create_tx.into()), || {
            let addr = H160::from_slice(
                &EVMModule::<C>::tx_create(&ctx, cbor::from_value(call.body).unwrap()).unwrap(),
            );
            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            TransactionResult::Commit(addr)
        });

    // Make sure the derived address matches the expected value. If this fails it likely indicates
    // a problem with nonce increment semantics between the SDK and EVM.
    assert_eq!(
        erc20_addr,
        "0x3e6a6598a229b84e1411005d55003d88e3b11067"
            .parse()
            .unwrap(),
        "derived address should be correct"
    );

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
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&ctx, &out_of_gas_create).unwrap();

    let call = out_of_gas_create.call.clone();
    CurrentState::with_transaction_opts(
        Options::new().with_tx(out_of_gas_create.clone().into()),
        || {
            assert!(!decode_result!(
                ctx,
                EVMModule::<C>::tx_create(&ctx, cbor::from_value(call.body).unwrap())
            )
            .is_success());
        },
    );

    // CheckTx should not fail.
    let call = out_of_gas_create.call.clone();
    CurrentState::with_transaction_opts(
        Options::new()
            .with_mode(state::Mode::Check)
            .with_tx(out_of_gas_create.clone().into()),
        || {
            let rsp = EVMModule::<C>::tx_create(&ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed with empty result");

            assert_eq!(
                rsp,
                Vec::<u8>::new(),
                "check tx should return an empty response"
            );
        },
    );

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
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&ctx, &call_name_tx).unwrap();

    // Test transaction call in simulate mode.
    let call = call_name_tx.call.clone();
    CurrentState::with_transaction_opts(
        Options::new()
            .with_mode(Mode::Simulate)
            .with_tx(call_name_tx.clone().into()),
        || {
            let erc20_name: Vec<u8> = cbor::from_value(
                decode_result!(
                    ctx,
                    EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
                )
                .unwrap(),
            )
            .unwrap();

            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            assert_eq!(erc20_name.len(), 96);
            assert_eq!(erc20_name[63], 0x04); // Name is 4 bytes long.
            assert_eq!(erc20_name[64..68], vec![0x54, 0x65, 0x73, 0x74]); // "Test".
        },
    );

    let call = call_name_tx.call.clone();
    let erc20_name =
        CurrentState::with_transaction_opts(Options::new().with_tx(call_name_tx.into()), || {
            let name: Vec<u8> = cbor::from_value(
                decode_result!(
                    ctx,
                    EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
                )
                .unwrap(),
            )
            .unwrap();

            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            TransactionResult::Commit(name)
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
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&ctx, &call_transfer_tx).unwrap();

    let call = call_transfer_tx.call.clone();
    let transfer_ret = CurrentState::with_transaction_opts(
        Options::new().with_tx(call_transfer_tx.into()),
        || {
            let ret: Vec<u8> = cbor::from_value(
                decode_result!(
                    ctx,
                    EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
                )
                .unwrap(),
            )
            .unwrap();

            EVMModule::<C>::check_invariants(&ctx).expect("invariants should hold");

            TransactionResult::Commit(ret)
        },
    );
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
    <EVMRuntime<C> as Runtime>::Modules::authenticate_tx(&ctx, &out_of_gas_tx).unwrap();

    let call = out_of_gas_tx.call.clone();
    CurrentState::with_transaction_opts(
        Options::new().with_tx(out_of_gas_tx.clone().into()),
        || {
            assert!(!decode_result!(
                ctx,
                EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
            )
            .is_success());
        },
    );

    // CheckTx should not fail.
    let call = out_of_gas_tx.call.clone();
    CurrentState::with_transaction_opts(
        Options::new()
            .with_mode(state::Mode::Check)
            .with_tx(out_of_gas_tx.clone().into()),
        || {
            let rsp = EVMModule::<C>::tx_call(&ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed with empty result");

            assert_eq!(
                rsp,
                Vec::<u8>::new(),
                "check tx should return an empty response"
            );
        },
    );
}

#[test]
fn test_evm_runtime() {
    do_test_evm_runtime::<EVMConfig>();
}

#[test]
fn test_c10l_evm_runtime() {
    let _guard = crypto::signature::context::test_using_chain_context();
    crypto::signature::context::set_chain_context(Default::default(), "test");
    do_test_evm_runtime::<ConfidentialEVMConfig>();
}

#[test]
fn test_c10l_queries() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<ConfidentialEVMConfig>>(true);
    let mut signer = EvmSigner::new(0, keys::dave::sigspec());

    EVMRuntime::<ConfidentialEVMConfig>::migrate(&ctx);

    static QUERY_CONTRACT_CODE_HEX: &str =
        include_str!("../../../../tests/e2e/contracts/query/query.hex");

    // Create contract.
    let dispatch_result = signer.call(
        &ctx,
        "evm.Create",
        types::Create {
            value: 0.into(),
            init_code: load_contract_bytecode(QUERY_CONTRACT_CODE_HEX),
        },
    );
    let result = dispatch_result.result.unwrap();
    let result: Vec<u8> = cbor::from_value(result).unwrap();
    let contract_address = H160::from_slice(&result);

    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<ConfidentialEVMConfig>>(true);

    // Call the `test` method on the contract via a query.
    let result = signer
        .query_evm(&ctx, contract_address, "test", &[], &[])
        .expect("query should succeed");

    let mut result =
        ethabi::decode(&[ParamType::Address], &result).expect("output should be correct");

    let test = result.pop().unwrap().into_address().unwrap();
    assert_eq!(test, Default::default(), "msg.signer should be zeroized");

    // Test call with confidential envelope.
    let result = signer
        .query_evm_opts(
            &ctx,
            contract_address,
            "test",
            &[],
            &[],
            QueryOptions {
                encrypt: true,
                ..Default::default()
            },
        )
        .expect("query should succeed");

    let mut result =
        ethabi::decode(&[ParamType::Address], &result).expect("output should be correct");

    let test = result.pop().unwrap().into_address().unwrap();
    assert_eq!(test, Default::default(), "msg.signer should be zeroized");
}

#[test]
fn test_fee_refunds() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<EVMConfig>>(true);
    let mut signer = EvmSigner::new(0, keys::dave::sigspec());

    EVMRuntime::<EVMConfig>::migrate(&ctx);

    // Give Dave some tokens.
    Accounts::mint(
        keys::dave::address(),
        &token::BaseUnits(1_000_000_000, Denomination::NATIVE),
    )
    .unwrap();

    // Create contract.
    let dispatch_result = signer.call(
        &ctx,
        "evm.Create",
        types::Create {
            value: 0.into(),
            init_code: load_contract_bytecode(TEST_CONTRACT_CODE_HEX),
        },
    );
    let result = dispatch_result.result.unwrap();
    let result: Vec<u8> = cbor::from_value(result).unwrap();
    let contract_address = H160::from_slice(&result);

    // Call the `name` method on the contract.
    let dispatch_result = signer.call_evm_opts(
        &ctx,
        contract_address,
        "name",
        &[],
        &[],
        CallOptions {
            fee: Fee {
                amount: token::BaseUnits::new(1_000_000, Denomination::NATIVE),
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
        amount: token::BaseUnits,
    }

    let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // One event for fee payment.
    let event = &events[0];
    assert_eq!(event.from, keys::dave::address());
    assert_eq!(event.to, *ADDRESS_FEE_ACCUMULATOR);
    assert_eq!(
        event.amount,
        token::BaseUnits::new(242_700, Denomination::NATIVE)
    );

    #[derive(Debug, Default, cbor::Decode)]
    struct GasUsedEvent {
        amount: u64,
    }

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].amount, 24_270);

    // Call the `transfer` method on the contract with invalid parameters so it reverts.
    let dispatch_result = signer.call_evm_opts(
        &ctx,
        contract_address,
        "transfer",
        &[ParamType::Address, ParamType::Uint(256)],
        &[
            Token::Address(contract_address.into()),
            Token::Uint(u128::MAX.into()), // Too much so it reverts.
        ],
        CallOptions {
            fee: Fee {
                amount: token::BaseUnits::new(1_000_000, Denomination::NATIVE),
                gas: 100_000,
                ..Default::default()
            },
        },
    );
    if let module::CallResult::Failed {
        module,
        code,
        message,
    } = dispatch_result.result
    {
        assert_eq!(module, "evm");
        assert_eq!(code, 8);
        assert_eq!(
            decode_reverted(&message).unwrap(),
            "ERC20: transfer amount exceeds balance"
        );
    } else {
        panic!("call should revert");
    }

    // Make sure two events were emitted and are properly formatted.
    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 2, "two events should have been emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

    let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // One event for fee payment.
    let event = &events[0];
    assert_eq!(event.from, keys::dave::address());
    assert_eq!(event.to, *ADDRESS_FEE_ACCUMULATOR);
    assert_eq!(
        event.amount,
        token::BaseUnits::new(245_850, Denomination::NATIVE) // Note the refund.
    );

    let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].amount, 24_585);
}

#[test]
fn test_return_value_limits() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<EVMConfig>>(true);
    let mut signer = EvmSigner::new(0, keys::dave::sigspec());

    EVMRuntime::<EVMConfig>::migrate(&ctx);

    // Give Dave some tokens.
    Accounts::mint(
        keys::dave::address(),
        &token::BaseUnits(1_000_000_000, Denomination::NATIVE),
    )
    .unwrap();

    static RETVAL_CONTRACT_CODE_HEX: &str =
        include_str!("../../../../tests/e2e/contracts/retval/retval.hex");

    // Create contract.
    let dispatch_result = signer.call(
        &ctx,
        "evm.Create",
        types::Create {
            value: 0.into(),
            init_code: load_contract_bytecode(RETVAL_CONTRACT_CODE_HEX),
        },
    );
    let result = dispatch_result.result.unwrap();
    let result: Vec<u8> = cbor::from_value(result).unwrap();
    let contract_address = H160::from_slice(&result);

    // Call the `testSuccess` method on the contract.
    let dispatch_result = signer.call_evm_opts(
        &ctx,
        contract_address,
        "testSuccess",
        &[],
        &[],
        CallOptions {
            fee: Fee {
                amount: token::BaseUnits::new(1_000_000, Denomination::NATIVE),
                gas: 100_000,
                ..Default::default()
            },
        },
    );
    let result: Vec<u8> = cbor::from_value(dispatch_result.result.unwrap()).unwrap();
    assert_eq!(result.len(), 1024, "result should be correctly trimmed");
    // Actual payload is ABI-encoded so the raw result starts at offset 64.
    assert_eq!(result[64], 0xFF, "result should be correct");
    assert_eq!(result[1023], 0x42, "result should be correct");

    // Call the `testRevert` method on the contract.
    let dispatch_result = signer.call_evm_opts(
        &ctx,
        contract_address,
        "testRevert",
        &[],
        &[],
        CallOptions {
            fee: Fee {
                amount: token::BaseUnits::new(1_000_000, Denomination::NATIVE),
                gas: 100_000,
                ..Default::default()
            },
        },
    );
    if let module::CallResult::Failed {
        module,
        code,
        message,
    } = dispatch_result.result
    {
        assert_eq!(module, "evm");
        assert_eq!(code, 8);
        let message = decode_reverted_raw(&message).unwrap();
        // Actual payload is ABI-encoded so the raw result starts at offset 68.
        assert_eq!(message[68], 0xFF, "result should be correct");
        assert_eq!(message[1023], 0x42, "result should be correct");
    } else {
        panic!("call should revert");
    }

    // Make sure that in query context, the return value is not trimmed.
    let ctx = mock.create_ctx_for_runtime::<EVMRuntime<EVMConfig>>(true);

    let result = signer
        .query_evm_opts(
            &ctx,
            contract_address,
            "testSuccess",
            &[],
            &[],
            Default::default(),
        )
        .expect("query should succeed");

    assert_eq!(result.len(), 1120, "result should not be trimmed");
    // Actual payload is ABI-encoded so the raw result starts at offset 64.
    assert_eq!(result[64], 0xFF, "result should be correct");
    assert_eq!(result[1023], 0x42, "result should be correct");
}
