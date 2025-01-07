use std::collections::BTreeMap;

use crate::{
    module,
    modules::{
        accounts::{self, API as _},
        core,
    },
    testing::{
        keys,
        mock::{self, CallOptions},
    },
    types::{
        address::Address,
        token::{BaseUnits, Denomination},
    },
    Runtime, Version,
};

use super::{app_id::AppId, state, types, Genesis, Module, ADDRESS_APP_STAKE_POOL, API as _};

type Accounts = accounts::Module;
type Core = core::Module<Config>;

struct Config;

impl core::Config for Config {}

impl super::Config for Config {
    const STAKE_APP_CREATE: BaseUnits = BaseUnits::new(1_000, Denomination::NATIVE);
}

/// Test runtime.
struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core;
    type Accounts = Accounts;

    type Modules = (Core, Accounts, Module<Config>);

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: 10_000_000,
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 0)]),
                    ..Default::default()
                },
            },
            accounts::Genesis {
                balances: BTreeMap::from([(
                    keys::alice::address(),
                    BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                )]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                ..Default::default()
            },
            Genesis::default(),
        )
    }
}

#[test]
fn test_app_stake_pool_address() {
    // Make sure the application stake pool address doesn't change.
    assert_eq!(
        ADDRESS_APP_STAKE_POOL.to_bech32(),
        "oasis1qza6sddnalgzexk3ct30gqfvntgth5m4hsyywmff"
    );
}

#[test]
fn test_management_ops() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        policy: Default::default(),
        scheme: Default::default(),
    };

    // Bob attempts to create a new ROFL application, but he doesn't have enough to stake.
    let mut signer_bob = mock::Signer::new(0, keys::bob::sigspec());
    let dispatch_result = signer_bob.call(&ctx, "rofl.Create", create.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");

    // Alice should be able to create a new ROFL application.
    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure the correct application ID has been created.
    let app_id: AppId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();
    assert_eq!(
        app_id.to_bech32(),
        "rofl1qpa9ydy3qmka3yrqzx0pxuvyfexf9mlh75hker5j"
    );

    // Make sure correct events were emitted.
    let tags = &dispatch_result.tags;
    assert_eq!(tags.len(), 3, "three event kinds should have been emitted");
    assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
    assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event
    assert_eq!(tags[2].key, b"rofl\x00\x00\x00\x01"); // rofl.AppCreated (code = 1) event

    // Ensure stake has been escrowed.
    #[derive(Debug, Default, cbor::Decode)]
    struct TransferEvent {
        from: Address,
        to: Address,
        amount: BaseUnits,
    }

    let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
    assert_eq!(events.len(), 1); // Just the escrow event as fee is zero.
    let event = &events[0];
    assert_eq!(event.from, keys::alice::address());
    assert_eq!(event.to, *ADDRESS_APP_STAKE_POOL);
    assert_eq!(event.amount, BaseUnits::new(1_000, Denomination::NATIVE));

    // Simulate round advancing as application ID is generated from it.
    mock.runtime_header.round += 1;
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);

    // Creating another application should get a different application ID.
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create.clone());
    let app_id: AppId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();
    assert_eq!(
        app_id.to_bech32(),
        "rofl1qzxz79xj0jxq07jtd2aysj0yxkxvldcg5vq24pj5"
    );

    // Ensure balances are correct.
    let balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 998_000); // Two applications require 2_000 stake in escrow.
    let balance = Accounts::get_balance(*ADDRESS_APP_STAKE_POOL, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 2_000); // Two applications require 2_000 stake in escrow.

    // Ensure queries return the right things.
    let app_cfg = Module::<Config>::get_app(app_id).unwrap();
    assert_eq!(
        app_cfg,
        types::AppConfig {
            id: app_id,
            policy: create.policy,
            admin: Some(keys::alice::address()),
            stake: BaseUnits::new(1_000, Denomination::NATIVE),
        }
    );
    let instances = Module::<Config>::get_instances(app_id).unwrap();
    assert_eq!(instances.len(), 0);

    // Update application. Bob should not be allowed to do it.
    let update = types::Update {
        id: app_id,
        policy: Default::default(),
        admin: Some(keys::bob::address()), // Transfer admin to bob.
    };

    let dispatch_result = signer_bob.call(&ctx, "rofl.Update", update.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 11); // Forbidden.

    // Update application. Alice should be allowed to transfer to Bob.
    let dispatch_result = signer_alice.call(&ctx, "rofl.Update", update.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Alice should no longer be allowed to update.
    let dispatch_result = signer_alice.call(&ctx, "rofl.Update", update.clone());
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 11); // Forbidden.

    // Ensure queries return the right things.
    let app_cfg = Module::<Config>::get_app(app_id).unwrap();
    assert_eq!(
        app_cfg,
        types::AppConfig {
            id: app_id,
            policy: update.policy,
            admin: Some(keys::bob::address()),
            stake: BaseUnits::new(1_000, Denomination::NATIVE),
        }
    );

    // Remove application. Alice should not be allowed to do it.
    let remove = types::Remove { id: app_id };

    let dispatch_result = signer_alice.call(&ctx, "rofl.Remove", remove.clone());
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 11); // Forbidden.

    // Remove application. Bob should be allowed to do it.
    let dispatch_result = signer_bob.call(&ctx, "rofl.Remove", remove.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure balances are correct.
    let balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 998_000); // One application requires 1_000 stake in escrow and 1_000 of stake was returned.
    let balance = Accounts::get_balance(keys::bob::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 1_000); // Returned stake for one application.
    let balance = Accounts::get_balance(*ADDRESS_APP_STAKE_POOL, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 1_000); // One application remains.
}

#[test]
fn test_create_scheme() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        policy: Default::default(),
        scheme: types::IdentifierScheme::CreatorNonce,
    };

    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure the correct application ID has been created.
    let app_id: AppId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();
    assert_eq!(
        app_id.to_bech32(),
        "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf"
    );
}

#[test]
fn test_key_derivation() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        policy: Default::default(),
        scheme: types::IdentifierScheme::CreatorNonce,
    };

    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");
    let app: AppId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();

    let derive = types::DeriveKey {
        app,
        kind: types::KeyKind::EntropyV0,
        generation: 0,
        key_id: b"my test key".into(),
    };

    // First try with plain calls which should fail.
    let dispatch_result = signer_alice.call(&ctx, "rofl.DeriveKey", derive.clone());
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 13); // Must use non-plain call format.

    // Use encrypted calls.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        derive.clone(),
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 11); // Forbidden (not an authorized key for the given app).

    // Manually create an application with alice being an authorized key.
    let fake_registration = types::Registration {
        app,
        extra_keys: vec![keys::alice::pk()],
        ..Default::default()
    };
    state::update_registration(fake_registration).unwrap();

    // The call should succeed now.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        derive.clone(),
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let dispatch_result = dispatch_result.result.unwrap();

    // In mock mode all the keys are deterministic.
    let result: types::DeriveKeyResponse = cbor::from_value(dispatch_result).unwrap();
    assert_eq!(result.key.as_ref(), &[0x33; 32]);

    // Ensure key identifier length limit is respected.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        types::DeriveKey {
            key_id: vec![0x01; 256],
            ..derive.clone()
        },
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let (err_module, err_code) = dispatch_result.result.unwrap_failed();
    assert_eq!(&err_module, "rofl");
    assert_eq!(err_code, 1);
}
