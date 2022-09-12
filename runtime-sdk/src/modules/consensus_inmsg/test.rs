use std::collections::BTreeMap;

use crate::{
    context::{Context, Mode},
    core::consensus::roothash::IncomingMessage,
    crypto::signature,
    error::SerializableError,
    event::IntoTags,
    module::{InMsgHandler, InMsgResult, MigrationHandler, Module},
    modules,
    runtime::Runtime,
    testing::{keys, mock},
    types::{token, transaction},
    Version,
};

struct Config;

impl modules::core::Config for Config {}

impl super::Config for Config {
    type Accounts = modules::accounts::Module;
    type Consensus = modules::consensus::Module;
}

type Core = modules::core::Module<Config>;

type Accounts = modules::accounts::Module;

type InMsgTx = super::InMsgTx<Config>;

struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core;

    type Modules = (Core, Accounts, modules::consensus::Module);

    fn genesis_state() -> <Self::Modules as MigrationHandler>::Genesis {
        Default::default()
    }
}

#[test]
fn test_process_in_msg_no_gas() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::set_params(
        ctx.runtime_state(),
        modules::core::Parameters {
            max_batch_gas: 1_000_000,
            max_inmsg_gas: 0,
            ..Default::default()
        },
    );

    let in_msg = IncomingMessage {
        id: 42,
        caller: keys::alice::address().into(),
        tag: 1000,
        fee: 1000u128.into(),
        tokens: 2000u128.into(),
        data: vec![],
    };
    let decision = InMsgTx::process_in_msg(&mut ctx, &in_msg);
    assert!(
        matches!(decision, InMsgResult::Stop),
        "should stop due to max_inmsg_gas being zero"
    );
}

#[test]
fn test_process_in_msg_no_tx() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::set_params(
        ctx.runtime_state(),
        modules::core::Parameters {
            max_batch_gas: 1_000_000,
            max_inmsg_gas: 500_000,
            ..Default::default()
        },
    );

    let in_msg = IncomingMessage {
        id: 42,
        caller: keys::alice::address().into(),
        tag: 1000,
        fee: 1000u128.into(),
        tokens: 2000u128.into(),
        data: vec![],
    };
    let decision = InMsgTx::process_in_msg(&mut ctx, &in_msg);

    assert!(
        matches!(decision, InMsgResult::Skip),
        "should skip as message does not contain a tx"
    );

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "2 events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_inmsg\x00\x00\x00\x01"); // consensus_inmsg.Processed (code = 1) event

    let expected_mint = modules::accounts::Event::Mint {
        owner: keys::alice::address(),
        amount: token::BaseUnits::new(3000, "TEST".parse().unwrap()), // Default consensus denomination is TEST.
    };
    assert_eq!(tags[0].value, cbor::to_vec(vec![expected_mint]));

    let expected_processed = super::Event::Processed {
        id: 42,
        tag: 1000,
        error: None,
    };
    assert_eq!(tags[1].value, cbor::to_vec(vec![expected_processed]));
}

#[test]
fn test_process_in_msg_tx_malformed() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::set_params(
        ctx.runtime_state(),
        modules::core::Parameters {
            max_batch_gas: 1_000_000,
            max_inmsg_gas: 500_000,
            ..Default::default()
        },
    );

    let in_msg = IncomingMessage {
        id: 42,
        caller: keys::alice::address().into(),
        tag: 1000,
        fee: 1000u128.into(),
        tokens: 2000u128.into(),
        data: b"not a valid transaction".to_vec(),
    };
    let decision = InMsgTx::process_in_msg(&mut ctx, &in_msg);

    assert!(
        matches!(decision, InMsgResult::Skip),
        "should skip as tx is malformed"
    );

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "2 events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_inmsg\x00\x00\x00\x01"); // consensus_inmsg.Processed (code = 1) event

    let expected_mint = modules::accounts::Event::Mint {
        owner: keys::alice::address(),
        amount: token::BaseUnits::new(3000, "TEST".parse().unwrap()), // Default consensus denomination is TEST.
    };
    assert_eq!(tags[0].value, cbor::to_vec(vec![expected_mint]));

    let expected_processed = super::Event::Processed {
        id: 42,
        tag: 1000,
        error: Some(SerializableError {
            module: "core".to_owned(),
            code: 1,
            message: "malformed transaction: decoding failed".to_owned(),
        }),
    };
    assert_eq!(tags[1].value, cbor::to_vec(vec![expected_processed]));
}

#[test]
fn test_process_in_msg_tx() {
    let _guard = signature::context::test_using_chain_context();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<TestRuntime>(Mode::ExecuteTx);

    Core::set_params(
        ctx.runtime_state(),
        modules::core::Parameters {
            max_batch_gas: 1_000_000,
            max_inmsg_gas: 500_000,
            max_tx_size: 32 * 1024,
            max_tx_signers: 1,
            min_gas_price: BTreeMap::from([("TEST".parse().unwrap(), 0)]),
            ..Default::default()
        },
    );

    signature::context::set_chain_context(Default::default(), "test");

    let tx = transaction::Transaction {
        version: transaction::LATEST_TRANSACTION_VERSION,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(modules::accounts::types::Transfer {
                to: keys::bob::address(),
                amount: token::BaseUnits::new(1_000, "TEST".parse().unwrap()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(10, "TEST".parse().unwrap()),
                gas: 1000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    let tx = cbor::to_vec(tx);
    let signature = keys::alice::signer()
        .context_sign(
            &signature::context::get_chain_context_for(transaction::SIGNATURE_CONTEXT_BASE),
            &tx,
        )
        .unwrap();
    let utx =
        transaction::UnverifiedTransaction(tx, vec![transaction::AuthProof::Signature(signature)]);

    let in_msg = IncomingMessage {
        id: 42,
        caller: keys::alice::address().into(),
        tag: 1000,
        fee: 1000u128.into(),
        tokens: 2000u128.into(),
        data: cbor::to_vec(utx),
    };
    let decision = InMsgTx::process_in_msg(&mut ctx, &in_msg);

    assert!(
        matches!(decision, InMsgResult::Execute(..)),
        "should execute tx"
    );

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "2 events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_inmsg\x00\x00\x00\x01"); // consensus_inmsg.Processed (code = 1) event

    let expected_mint = modules::accounts::Event::Mint {
        owner: keys::alice::address(),
        amount: token::BaseUnits::new(3000, "TEST".parse().unwrap()), // Default consensus denomination is TEST.
    };
    assert_eq!(tags[0].value, cbor::to_vec(vec![expected_mint]));

    let expected_processed = super::Event::Processed {
        id: 42,
        tag: 1000,
        error: None,
    };
    assert_eq!(tags[1].value, cbor::to_vec(vec![expected_processed]));
}

#[test]
fn test_process_in_msg_tx_fail_checks() {
    let _guard = signature::context::test_using_chain_context();
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<TestRuntime>(Mode::ExecuteTx);

    Core::set_params(
        ctx.runtime_state(),
        modules::core::Parameters {
            max_batch_gas: 1_000_000,
            max_inmsg_gas: 500_000,
            max_tx_size: 32 * 1024,
            max_tx_signers: 1,
            min_gas_price: BTreeMap::from([("TEST".parse().unwrap(), 0)]),
            ..Default::default()
        },
    );

    signature::context::set_chain_context(Default::default(), "test");

    let tx = transaction::Transaction {
        version: transaction::LATEST_TRANSACTION_VERSION,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "accounts.Transfer".to_owned(),
            body: cbor::to_value(modules::accounts::types::Transfer {
                to: keys::bob::address(),
                amount: token::BaseUnits::new(1_000, "TEST".parse().unwrap()),
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                // Set a fee that we don't have the funds to pay.
                amount: token::BaseUnits::new(10_000, "TEST".parse().unwrap()),
                gas: 1000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    let tx = cbor::to_vec(tx);
    let signature = keys::alice::signer()
        .context_sign(
            &signature::context::get_chain_context_for(transaction::SIGNATURE_CONTEXT_BASE),
            &tx,
        )
        .unwrap();
    let utx =
        transaction::UnverifiedTransaction(tx, vec![transaction::AuthProof::Signature(signature)]);

    let in_msg = IncomingMessage {
        id: 42,
        caller: keys::alice::address().into(),
        tag: 1000,
        fee: 1000u128.into(),
        tokens: 2000u128.into(),
        data: cbor::to_vec(utx),
    };
    let decision = InMsgTx::process_in_msg(&mut ctx, &in_msg);

    assert!(matches!(decision, InMsgResult::Skip), "should skip tx");

    let (etags, _) = ctx.commit();
    let tags = etags.into_tags();
    assert_eq!(tags.len(), 2, "2 events should be emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x03"); // accounts.Mint (code = 3) event
    assert_eq!(tags[1].key, b"consensus_inmsg\x00\x00\x00\x01"); // consensus_inmsg.Processed (code = 1) event

    let expected_mint = modules::accounts::Event::Mint {
        owner: keys::alice::address(),
        amount: token::BaseUnits::new(3000, "TEST".parse().unwrap()), // Default consensus denomination is TEST.
    };
    assert_eq!(tags[0].value, cbor::to_vec(vec![expected_mint]));

    let expected_processed = super::Event::Processed {
        id: 42,
        tag: 1000,
        error: Some(SerializableError {
            module: "core".to_owned(),
            code: 5,
            message: "check failed: insufficient balance to pay fees".to_owned(),
        }),
    };
    assert_eq!(tags[1].value, cbor::to_vec(vec![expected_processed]));
}
