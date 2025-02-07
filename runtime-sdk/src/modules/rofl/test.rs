use std::collections::BTreeMap;

use crate::{
    crypto, module,
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
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        policy: Default::default(),
        scheme: Default::default(),
        metadata: BTreeMap::from([("foo".into(), "bar".into())]),
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
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

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
            metadata: BTreeMap::from([("foo".into(), "bar".into())]),
            sek: app_cfg.sek.clone(),
            ..Default::default()
        }
    );
    let apps = Module::<Config>::get_apps().unwrap();
    assert_eq!(apps.len(), 2);
    let instances = Module::<Config>::get_instances(app_id).unwrap();
    assert_eq!(instances.len(), 0);

    // Update application. Bob should not be allowed to do it.
    let update = types::Update {
        id: app_id,
        policy: Default::default(),
        admin: Some(keys::bob::address()), // Transfer admin to bob.
        ..Default::default()
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
            sek: app_cfg.sek.clone(),
            ..Default::default()
        }
    );
    let apps = Module::<Config>::get_apps().unwrap();
    assert_eq!(apps.len(), 2);

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

    // Ensure one app is left.
    let apps = Module::<Config>::get_apps().unwrap();
    assert_eq!(apps.len(), 1);
}

#[test]
fn test_create_scheme() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        scheme: types::IdentifierScheme::CreatorNonce,
        ..Default::default()
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
fn test_derive_app_key_id() {
    // Ensure that app key identifier derivation is stable.
    let tcs = [
        (
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf",
            types::KeyKind::EntropyV0,
            types::KeyScope::Global,
            b"test key",
            None,
            "d5c2af4a445f893f3cc395fd7ed3aca53ecc98f4ff93401f03d8e17b9dac563f",
        ),
        (
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf",
            types::KeyKind::X25519,
            types::KeyScope::Global,
            b"test key",
            None,
            "e1ad6dfca49ca3449642a8334c97120bbdeaf778e2c5b44f06b3584d9d77edbb",
        ),
        (
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf",
            types::KeyKind::EntropyV0,
            types::KeyScope::Node,
            b"test key",
            Some(types::Registration {
                node_id: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                ..Default::default()
            }),
            "90725fe1f83e647f2a6ae917e076b5c42cf64944efa79fa4707a39583ff89b26",
        ),
        (
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf",
            types::KeyKind::EntropyV0,
            types::KeyScope::Node,
            b"test key",
            Some(types::Registration {
                node_id: "1111111111111111111111111111111111111111111111111111111111111111".into(),
                ..Default::default()
            }),
            "06374f036728adfbd64a2bb10d55a3d8ea8de122305383ea27064e12caafba71",
        ),
        (
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf",
            types::KeyKind::EntropyV0,
            types::KeyScope::Entity,
            b"test key",
            Some(types::Registration {
                entity_id: Some(
                    "1111111111111111111111111111111111111111111111111111111111111111".into(),
                ),
                ..Default::default()
            }),
            "5e3ec31fb6648b48fa8e721796fb1d01d11fee50303fd382118ddf9e69d6f77b",
        ),
    ];
    for tc in tcs {
        let key_id =
            Module::<Config>::derive_app_key_id(&tc.0.into(), tc.1, tc.2, tc.3, tc.4.clone())
                .unwrap();
        assert_eq!(key_id, tc.5.into(), "{:?}", tc);
    }
}

#[test]
fn test_key_derivation() {
    let _guard = crypto::signature::context::test_using_chain_context();
    crypto::signature::context::set_chain_context(Default::default(), "test");

    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    let create = types::Create {
        scheme: types::IdentifierScheme::CreatorNonce,
        ..Default::default()
    };

    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");
    let app: AppId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();

    let derive = types::DeriveKey {
        app,
        kind: types::KeyKind::EntropyV0,
        scope: types::KeyScope::Global,
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
    state::update_registration(fake_registration.clone()).unwrap();

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
    assert_eq!(&result.key, &[0x33; 32]);

    // Also try X25519 derivation.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        types::DeriveKey {
            kind: types::KeyKind::X25519,
            ..derive.clone()
        },
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let dispatch_result = dispatch_result.result.unwrap();
    let result: types::DeriveKeyResponse = cbor::from_value(dispatch_result).unwrap();
    assert!(!result.key.is_empty());

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

    // Try different scopes.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        types::DeriveKey {
            scope: types::KeyScope::Node,
            ..derive.clone()
        },
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let dispatch_result = dispatch_result.result.unwrap();
    let result: types::DeriveKeyResponse = cbor::from_value(dispatch_result).unwrap();
    assert!(!result.key.is_empty());

    // Entity scope should fail as the registration doesn't have an entity set.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        types::DeriveKey {
            scope: types::KeyScope::Entity,
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

    // Update registration to include an entity.
    state::update_registration(types::Registration {
        entity_id: Some(Default::default()),
        ..fake_registration.clone()
    })
    .unwrap();

    // Entity scope should now work.
    let dispatch_result = signer_alice.call_opts(
        &ctx,
        "rofl.DeriveKey",
        types::DeriveKey {
            scope: types::KeyScope::Entity,
            ..derive.clone()
        },
        CallOptions {
            encrypted: true,
            ..Default::default()
        },
    );
    let dispatch_result = dispatch_result.result.unwrap();
    let result: types::DeriveKeyResponse = cbor::from_value(dispatch_result).unwrap();
    assert!(!result.key.is_empty());
}
