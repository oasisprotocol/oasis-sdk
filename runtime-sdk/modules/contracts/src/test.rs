//! Tests for the contracts module.
use std::{collections::BTreeMap, io::Write};

use oasis_runtime_sdk::{
    context,
    error::Error,
    event::IntoTags,
    module,
    modules::{
        accounts::{self, Module as Accounts, API as _},
        core::{self, Module as Core},
    },
    testing::{keys, mock},
    types::{
        token::{BaseUnits, Denomination},
        transaction,
    },
    BatchContext, Context, Runtime, Version,
};

use crate::{types, types::StoreKind, Config, Genesis};

/// Hello contract code.
static HELLO_CONTRACT_CODE: &[u8] = include_bytes!(
    "../../../../tests/contracts/hello/target/wasm32-unknown-unknown/release/hello.wasm"
);

struct ContractsConfig;

impl Config for ContractsConfig {
    type Accounts = Accounts;
}

type Contracts = crate::Module<ContractsConfig>;

fn upload_hello_contract<C: BatchContext>(ctx: &mut C) -> types::CodeId {
    // Compress contract code.
    let mut code = Vec::with_capacity(HELLO_CONTRACT_CODE.len() << 3);
    let mut encoder = snap::write::FrameEncoder::new(&mut code);
    encoder.write_all(HELLO_CONTRACT_CODE).unwrap();
    drop(encoder); // Make sure data is flushed.

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Upload".to_owned(),
            body: cbor::to_value(types::Upload {
                abi: types::ABI::OasisV1,
                instantiate_policy: types::Policy::Everyone,
                code,
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
                gas: 141_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let code_id = Contracts::tx_upload(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("upload should succeed")
            .id;

        tx_ctx.commit();

        code_id
    })
}

fn deploy_hello_contract<C: BatchContext>(
    ctx: &mut C,
    tokens: Vec<BaseUnits>,
) -> types::InstanceId {
    // Upload the contract.
    upload_hello_contract(ctx);

    // Then instantiate the code.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Instantiate".to_owned(),
            body: cbor::to_value(types::Instantiate {
                code_id: 0.into(),
                upgrades_policy: types::Policy::Address(keys::alice::address()),
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map! {
                    "instantiate" => cbor::cbor_map! {
                        "initial_counter" => cbor::cbor_int!(33)
                    }
                }),
                tokens,
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let instance_id =
            Contracts::tx_instantiate(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("instantiate should succeed")
                .id;

        tx_ctx.commit();

        instance_id
    })
}

#[test]
fn test_hello_contract_call() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Core::<CoreConfig>::init(
        &mut ctx,
        core::Genesis {
            parameters: core::Parameters {
                max_batch_gas: 1_000_000_000,
                ..Default::default()
            },
        },
    );

    Accounts::init(
        &mut ctx,
        accounts::Genesis {
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
        },
    );

    Contracts::init(
        &mut ctx,
        Genesis {
            parameters: Default::default(),
        },
    );

    let instance_id =
        deploy_hello_contract(&mut ctx, vec![BaseUnits::new(1_000, Denomination::NATIVE)]);

    // Check caller account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::alice::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        999_000, // -1_000
        "balance in caller account should be correct"
    );
    assert_eq!(
        bals.balances.len(),
        1,
        "there should only be one denomination"
    );

    // Check contract account balances.
    let bals = Accounts::get_balances(
        ctx.runtime_state(),
        types::Instance::address_for(instance_id),
    )
    .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1_000, // +1_000
        "balance in contract account should be correct"
    );
    assert_eq!(
        bals.balances.len(),
        1,
        "there should only be one denomination"
    );

    // And finally call a method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map! {
                    "say_hello" => cbor::cbor_map!{
                        "who" => cbor::cbor_text!("tester")
                    }
                }),
                tokens: vec![BaseUnits::new(2_000, Denomination::NATIVE)],
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("hello tester (33)")
                }
            }
        );

        // Check caller account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::alice::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            997_000, // -2_000
            "balance in caller account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check contract account balances.
        let bals = Accounts::get_balances(
            tx_ctx.runtime_state(),
            types::Instance::address_for(instance_id),
        )
        .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            3_000, // +2_000
            "balance in contract account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        let (etags, messages) = tx_ctx.commit();
        let tags = etags.into_tags();
        // Make sure no runtime messages got emitted.
        assert!(messages.is_empty(), "no runtime messages should be emitted");
        // Make sure a contract event was emitted and is properly formatted.
        assert_eq!(tags.len(), 2, "two events should have been emitted");
        assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
        assert_eq!(tags[1].key, b"contracts.0\x00\x00\x00\x01"); // contracts.1 (code = 1) event

        let mut events: Vec<types::ContractEvent> =
            cbor::from_slice(&tags[1].value).expect("contract events should be wrapped");
        assert_eq!(events.len(), 1);
        let event = events.pop().unwrap();
        assert_eq!(
            event.id, instance_id,
            "instance id in the event should match"
        );
        assert_eq!(event.data, b"\x65world"); // CBOR-encoded string "world"
    });

    // Second call should increment the counter.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map! {
                    "say_hello" => cbor::cbor_map!{
                        "who" => cbor::cbor_text!("second")
                    }
                }),
                tokens: vec![],
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("hello second (34)")
                }
            }
        );

        // Check caller account balances.
        let bals = Accounts::get_balances(tx_ctx.runtime_state(), keys::alice::address())
            .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            997_000, // No change.
            "balance in caller account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        // Check contract account balances.
        let bals = Accounts::get_balances(
            tx_ctx.runtime_state(),
            types::Instance::address_for(instance_id),
        )
        .expect("get_balances should succeed");
        assert_eq!(
            bals.balances[&Denomination::NATIVE],
            3_000, // No change.
            "balance in contract account should be correct"
        );
        assert_eq!(
            bals.balances.len(),
            1,
            "there should only be one denomination"
        );

        tx_ctx.commit();
    });

    // Test instance query.
    let result = Contracts::query_instance(&mut ctx, types::InstanceQuery { id: instance_id })
        .expect("instance query should succeed");
    assert_eq!(result.id, instance_id);
    assert_eq!(result.code_id, 0.into());
    assert_eq!(result.creator, keys::alice::address());

    // Test code query.
    let result = Contracts::query_code(&mut ctx, types::CodeQuery { id: result.code_id })
        .expect("code query should succeed");
    assert_eq!(result.id, 0.into());
    assert_eq!(result.abi, types::ABI::OasisV1);

    // Test code storage query.
    let result = Contracts::query_code_storage(&mut ctx, types::CodeStorageQuery { id: 0.into() })
        .expect("code storage query should succeed");
    // Stored code is the original code plus some injected gas billing calls.
    assert!(result.code.len() >= HELLO_CONTRACT_CODE.len());

    // Invalid code queries should fail.
    Contracts::query_code(&mut ctx, types::CodeQuery { id: 9999.into() })
        .expect_err("invalid code query should fail");
    Contracts::query_code_storage(&mut ctx, types::CodeStorageQuery { id: 9999.into() })
        .expect_err("invalid code storage query should fail");

    // Test storage query for the counter key.
    let result = Contracts::query_instance_storage(
        &mut ctx,
        types::InstanceStorageQuery {
            id: instance_id,
            key: b"counter".to_vec(),
        },
    )
    .expect("instance storage query should succeed");
    let value = result.value.expect("counter value should be set");
    let value: u64 = cbor::from_slice(&value).expect("counter value should be well-formed");
    // Value is 35 because it was incremented by last call above.
    assert_eq!(value, 35, "counter value should be correct");

    // Test raw public storage query.
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: None,
            offset: None,
        },
    )
    .expect("instance raw public storage query should succeed");
    assert_eq!(
        result.items.len(),
        3,
        "raw storage should contain 3 elements (tester, second, counter)"
    );
    assert_eq!(
        result.items[0].0, b"timestampstester",
        "first item in raw storage should be timestamps[tester]"
    );
    assert_eq!(
        result.items[1].0, b"timestampssecond",
        "second item in raw storage should be timestamps[second]"
    );
    assert_eq!(
        result.items[2].0, b"counter",
        "third item in raw storage should be counter"
    );
    let raw_value: u64 = cbor::from_slice(&result.items[2].1)
        .expect("counter value in raw storage should be well-formed");
    // Value is 35 because it was incremented by last call above.
    assert_eq!(
        raw_value, 35,
        "counter value in raw storage should be correct"
    );

    // Test PublicMap and ConfidentialMap.
    for i in 0..10 {
        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                format: transaction::CallFormat::Plain,
                method: "contracts.Call".to_owned(),
                body: cbor::to_value(types::Call {
                    id: instance_id,
                    // Needs to conform to contract API.
                    data: cbor::to_vec(cbor::cbor_map! {
                        "say_hello" => cbor::cbor_map!{
                            "who" => cbor::cbor_text!(format!("maptest{}", i))
                        }
                    }),
                    tokens: vec![],
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
                    gas: 1_000_000,
                    consensus_messages: 0,
                },
                ..Default::default()
            },
        };
        ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
            let _result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed");
            tx_ctx.commit();
        });
    }
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: None,
            offset: None,
        },
    )
    .expect("instance raw public storage query should succeed");
    assert_eq!(
        result.items.len(),
        13,
        "raw public storage should contain correct number of elements"
    );
    for item in result.items {
        if !item.0.starts_with(b"timestampsmaptest") {
            continue;
        }
        // "testX" -> X
        let idx = std::str::from_utf8(&item.0[17..])
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let val: u64 = cbor::from_slice(&item.1)
            .expect("counter item's value in raw storage should be well-formed");
        assert_eq!(
            val,
            idx + 35,
            "iterated counter value in raw storage should be correct"
        );
    }
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Confidential,
            limit: None,
            offset: None,
        },
    )
    .expect("instance raw confidential storage query should succeed");
    assert_eq!(
        result.items.len(),
        13,
        "raw confidential storage should contain 13 encrypted elements"
    );

    // Test instance raw storage pagination.
    // Insert additional 100 entries so we exceed max_instance_raw_storage_query_items (default: 100)
    for i in 10..110 {
        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                format: transaction::CallFormat::Plain,
                method: "contracts.Call".to_owned(),
                body: cbor::to_value(types::Call {
                    id: instance_id,
                    // Needs to conform to contract API.
                    data: cbor::to_vec(cbor::cbor_map! {
                        "say_hello" => cbor::cbor_map!{
                            "who" => cbor::cbor_text!(format!("test{}", i))
                        }
                    }),
                    tokens: vec![],
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
                    gas: 1_000_000,
                    consensus_messages: 0,
                },
                ..Default::default()
            },
        };
        ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
            let _result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("call should succeed");
            tx_ctx.commit();
        });
    }
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: None,
            offset: None,
        },
    )
    .expect("instance raw storage query should succeed");
    assert_eq!(
        100,
        result.items.len(),
        "raw storage query should be limited by default limit 100"
    );
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: 1000.into(),
            offset: None,
        },
    )
    .expect("instance raw storage query should succeed");
    assert_eq!(
        100,
        result.items.len(),
        "raw storage query should be limited by default limit 100, even if requested limit is higher"
    );
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: 10.into(),
            offset: None,
        },
    )
    .expect("instance raw storage query should succeed");
    assert_eq!(
        10,
        result.items.len(),
        "raw storage should contain 10 elements"
    );
    let result = Contracts::query_instance_raw_storage(
        &mut ctx,
        types::InstanceRawStorageQuery {
            id: instance_id,
            store_kind: StoreKind::Public,
            limit: 10.into(),
            offset: 110.into(),
        },
    )
    .expect("instance raw storage query should succeed");
    assert_eq!(
        result.items.len(),
        3,
        "raw storage should contain the remaining 3 elements"
    );

    // Invalid call should fail.
    let invalid_tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map! {
                    "say_hello" => cbor::cbor_map!{
                        "whooo" => cbor::cbor_text!("second") // Invalid method.
                    }
                }),
                tokens: vec![],
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, invalid_tx.clone(), |mut tx_ctx, call| {
        Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("invalid call should fail");
    });

    ctx.with_child(context::Mode::CheckTx, |mut check_ctx| {
        check_ctx.with_tx(0, 0, invalid_tx, |mut tx_ctx, call| {
            Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect("invalid call should succeed check-tx");
        });
    })
}

struct CoreConfig;

impl core::Config for CoreConfig {}

/// Contract runtime.
struct ContractRuntime;

impl Runtime for ContractRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core<CoreConfig>;

    type Modules = (Core<CoreConfig>, Accounts, Contracts);

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: 1_000_000_000,
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 1)]),
                    ..Default::default()
                },
            },
            accounts::Genesis {
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
            },
            Genesis {
                parameters: Default::default(),
            },
        )
    }
}

#[test]
fn test_hello_contract_subcalls_overflow() {
    use cbor::cbor_map;

    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);

    // And finally call a method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                data: cbor::to_vec(cbor::cbor_map! { // Needs to conform to contract API.
                    "call_self" => cbor::cbor_map! {
                        "once" => cbor::cbor_bool!(false),
                    },
                }),
                tokens: vec![],
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
                gas: 3_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("call should fail");

        assert_eq!(result.module_name(), "contracts.0");
        assert_eq!(result.code(), 3);
        assert_eq!(
            &result.to_string(),
            "contract error: subcall failed: module=contracts.0 code=3"
        );
    });
}

#[test]
fn test_hello_contract_subcalls() {
    use cbor::cbor_map;

    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);

    // And finally call a method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                data: cbor::to_vec(cbor::cbor_map! { // Needs to conform to contract API.
                    "call_self" => cbor::cbor_map! {
                        "once" => cbor::cbor_bool!(true),
                    },
                }),
                tokens: vec![],
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::alice::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: BaseUnits::new(2_000_000, Denomination::NATIVE),
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx.clone(), |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("hello subcall (33)")
                }
            }
        );
    });

    // Gas estimation should work.
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::CheckTx);
    let args = core::types::EstimateGasQuery {
        caller: None,
        tx,
        propagate_failures: true,
    };
    <ContractRuntime as Runtime>::Core::query_estimate_gas(&mut ctx, args)
        .expect("query_estimate_gas should succeed");
}

#[test]
fn test_hello_contract_query() {
    let mut mock = mock::Mock::default();

    // Replace default values so we can check them in query results.
    mock.runtime_header.round = 11;
    mock.runtime_header.timestamp = 1629117379;
    mock.epoch = 42;

    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);

    // Call the query_block_info method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                data: cbor::to_vec(cbor::cbor_text!("query_block_info")), // Needs to conform to contract API.
                tokens: vec![],
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("round: 11 epoch: 42 timestamp: 1629117379")
                }
            }
        );
    });

    // Call the query_accounts method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                data: cbor::to_vec(cbor::cbor_text!("query_accounts")), // Needs to conform to contract API.
                tokens: vec![BaseUnits::new(2_000, Denomination::NATIVE)], // For the query below.
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("my native balance is: 2000")
                }
            }
        );
    });

    // Call the query_address method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Call".to_owned(),
            body: cbor::to_value(types::Call {
                id: instance_id,
                data: cbor::to_vec(cbor::cbor_text!("query_address")), // Needs to conform to contract API.
                tokens: vec![],
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
                gas: 1_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_call(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("call should succeed");

        let result: cbor::Value =
            cbor::from_slice(&result.0).expect("result should be correctly formatted");
        assert_eq!(
            result,
            cbor::cbor_map! {
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("my address is: oasis1qq08mjlkztsgpgrar082rzzxwjaplxmgjs5ftugn")
                }
            }
        );
    });
}

#[test]
fn test_hello_contract_upgrade() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);
    let code_2 = upload_hello_contract(&mut ctx);

    // Call the upgrade method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Upgrade".to_owned(),
            body: cbor::to_value(types::Upgrade {
                id: instance_id,
                code_id: code_2,
                data: cbor::to_vec(cbor::cbor_text!("upgrade_proceed")), // Needs to conform to contract API.
                tokens: vec![],
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
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        Contracts::tx_upgrade(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("upgrade should succeed");

        tx_ctx.commit();
    });
}

#[test]
fn test_hello_contract_upgrade_fail_policy() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);
    let code_2 = upload_hello_contract(&mut ctx);

    // Make Bob call the upgrade method which should fail as he is not authorized.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Upgrade".to_owned(),
            body: cbor::to_value(types::Upgrade {
                id: instance_id,
                code_id: code_2,
                data: cbor::to_vec(cbor::cbor_text!("upgrade_proceed")), // Needs to conform to contract API.
                tokens: vec![],
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::bob::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_upgrade(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("upgrade should fail");

        assert_eq!(result.module_name(), "contracts");
        assert_eq!(result.code(), 13);
        assert_eq!(&result.to_string(), "forbidden by policy");
    });
}

#[test]
fn test_hello_contract_upgrade_fail_pre() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);
    let code_2 = upload_hello_contract(&mut ctx);

    // Call the upgrade handler with a request that should cause a failure in pre-upgrade.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Upgrade".to_owned(),
            body: cbor::to_value(types::Upgrade {
                id: instance_id,
                code_id: code_2,
                data: cbor::to_vec(cbor::cbor_text!("upgrade_fail_pre")), // Needs to conform to contract API.
                tokens: vec![],
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
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_upgrade(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("upgrade should fail");

        assert_eq!(result.module_name(), "contracts.0");
        assert_eq!(result.code(), 4);
        assert_eq!(
            &result.to_string(),
            "contract error: upgrade not allowed (pre)"
        );
    });
}

#[test]
fn test_hello_contract_upgrade_fail_post() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);
    let code_2 = upload_hello_contract(&mut ctx);

    // Call the upgrade handler with a request that should cause a failure in post-upgrade.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.Upgrade".to_owned(),
            body: cbor::to_value(types::Upgrade {
                id: instance_id,
                code_id: code_2,
                data: cbor::to_vec(cbor::cbor_text!("upgrade_fail_post")), // Needs to conform to contract API.
                tokens: vec![],
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
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result = Contracts::tx_upgrade(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect_err("upgrade should fail");

        assert_eq!(result.module_name(), "contracts.1"); // Note the new code id.
        assert_eq!(result.code(), 5);
        assert_eq!(
            &result.to_string(),
            "contract error: upgrade not allowed (post)"
        );
    });
}

#[test]
fn test_hello_contract_change_upgrade_policy() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);

    // Call the upgrade method.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.ChangeUpgradePolicy".to_owned(),
            body: cbor::to_value(types::ChangeUpgradePolicy {
                id: instance_id,
                upgrades_policy: types::Policy::Nobody,
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
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        Contracts::tx_change_upgrade_policy(&mut tx_ctx, cbor::from_value(call.body).unwrap())
            .expect("upgrade should succeed");

        tx_ctx.commit();
    });
}

#[test]
fn test_hello_contract_change_upgrade_policy_fail() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx_for_runtime::<ContractRuntime>(context::Mode::ExecuteTx);

    ContractRuntime::migrate(&mut ctx);

    let instance_id = deploy_hello_contract(&mut ctx, vec![]);

    // Make Bob call the change upgrade policy method which should fail as he is not authorized.
    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "contracts.ChangeUpgradePolicy".to_owned(),
            body: cbor::to_value(types::ChangeUpgradePolicy {
                id: instance_id,
                upgrades_policy: types::Policy::Nobody,
            }),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new_sigspec(
                keys::bob::sigspec(),
                0,
            )],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 2_000_000,
                consensus_messages: 0,
            },
            ..Default::default()
        },
    };
    ctx.with_tx(0, 0, tx, |mut tx_ctx, call| {
        let result =
            Contracts::tx_change_upgrade_policy(&mut tx_ctx, cbor::from_value(call.body).unwrap())
                .expect_err("change upgrade policy should fail");

        assert_eq!(result.module_name(), "contracts");
        assert_eq!(result.code(), 13);
        assert_eq!(&result.to_string(), "forbidden by policy");
    });
}
