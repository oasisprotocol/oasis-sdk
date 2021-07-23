//! Tests for the contracts module.
use oasis_runtime_sdk::{
    testing::{keys, mock},
    types::transaction,
    BatchContext, Context,
};

use crate::{types, Genesis, Module as Contracts};

/// Hello contract code.
static HELLO_CONTRACT: &[u8] = include_bytes!("../../../../tests/contracts/hello/hello.wasm");

#[test]
fn test_hello_contract_call() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Contracts::init(
        &mut ctx,
        Genesis {
            parameters: Default::default(),
        },
    );

    // First upload the contract code.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "contracts.Upload".to_owned(),
            body: cbor::to_value(types::Upload {
                abi: types::ABI::OasisV1,
                instantiate_policy: types::Policy::Everyone,
                code: HELLO_CONTRACT.to_vec(),
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
    ctx.with_tx(tx, |mut tx_ctx, call| {
        Contracts::tx_upload(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("upload should succeed");

        tx_ctx.commit();
    });

    // Then instantiate the code.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "contracts.Instantiate".to_owned(),
            body: cbor::to_value(types::Instantiate {
                code_id: 0.into(),
                calls_policy: types::Policy::Everyone,
                upgrades_policy: types::Policy::Nobody,
                data: cbor::to_vec(cbor::cbor_text!("instantiate")), // Needs to conform to contract API.
                tokens: vec![],
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
    ctx.with_tx(tx, |mut tx_ctx, call| {
        Contracts::tx_instantiate(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("instantiate should succeed");

        tx_ctx.commit();
    });

    // And finally call a method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: 0.into(),
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map! {
                    "say_hello" => cbor::cbor_map!{
                        "who" => cbor::cbor_text!("tester")
                    }
                }),
                tokens: vec![],
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
    ctx.with_tx(tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("hello tester")
                }
            }
        );

        tx_ctx.commit();
    });
}
