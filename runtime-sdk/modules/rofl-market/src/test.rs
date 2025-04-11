use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    module,
    modules::{
        accounts::{self, API as _},
        core, rofl,
    },
    testing::{keys, mock},
    types::{
        address::Address,
        token::{BaseUnits, Denomination},
    },
    Runtime, Version,
};

use super::{types, ADDRESS_PROVIDER_STAKE_POOL};

type Accounts = accounts::Module;
type Core = core::Module<Config>;

struct Config;

impl core::Config for Config {}

impl rofl::Config for Config {}

impl super::Config for Config {
    type Rofl = rofl::Module<Config>;

    const STAKE_PROVIDER_CREATE: BaseUnits = BaseUnits::new(1_000, Denomination::NATIVE);
}

/// Test runtime.
struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = Core;
    type Accounts = Accounts;

    type Modules = (Core, Accounts, rofl::Module<Config>, super::Module<Config>);

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
                balances: BTreeMap::from([
                    (
                        keys::alice::address(),
                        BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                    ),
                    (
                        keys::charlie::address(),
                        BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                    ),
                ]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 2_000_000)]),
                ..Default::default()
            },
            Default::default(),
            Default::default(),
        )
    }
}

#[test]
fn test_provider_stake_pool_address() {
    // Make sure the provider stake pool address doesn't change.
    assert_eq!(
        ADDRESS_PROVIDER_STAKE_POOL.to_bech32(),
        "oasis1qzta0kk6vy0yrwgllual4ntnjay68lp7vq5fs8jy"
    );
}

#[test]
fn test_provider_management() {
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    // Create a provider.
    let create = types::ProviderCreate {
        nodes: vec![],
        scheduler_app: Default::default(),
        payment_address: types::PaymentAddress::Native(keys::alice::address()),
        offers: vec![types::Offer {
            capacity: 1,
            metadata: BTreeMap::from([("foo".to_string(), "bar".to_string())]),
            resources: types::Resources {
                tee: types::TeeType::TDX,
                memory: 512,
                cpus: 1,
                storage: 1024,
                gpu: None,
            },
            ..Default::default()
        }],
        metadata: Default::default(),
    };

    // Bob attempts to create a new provider, but he doesn't have enough to stake.
    let mut signer_bob = mock::Signer::new(0, keys::bob::sigspec());
    let dispatch_result = signer_bob.call(&ctx, "roflmarket.ProviderCreate", create.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "accounts");
    assert_eq!(code, 2); // Insufficient balance.

    // Alice should be able to create a new provider.
    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderCreate", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Re-creating a provider should be rejected.
    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderCreate", create.clone());
    assert!(
        !dispatch_result.result.is_success(),
        "re-create call should fail"
    );
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 2); // Provider already exists.

    // Query the provider.
    let provider: types::Provider = signer_alice
        .query(
            &ctx,
            "roflmarket.Provider",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(provider.nodes, create.nodes);
    assert_eq!(provider.scheduler_app, create.scheduler_app);
    assert_eq!(provider.payment_address, create.payment_address);
    assert_eq!(provider.metadata, create.metadata);

    // Query all providers.
    let providers: Vec<types::Provider> = signer_alice
        .query(&ctx, "roflmarket.Providers", ())
        .unwrap();
    assert_eq!(providers.len(), 1);

    // Query offer.
    let offer: types::Offer = signer_alice
        .query(
            &ctx,
            "roflmarket.Offer",
            types::OfferQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(offer, create.offers[0]);

    // Query offers.
    let offers: Vec<types::Offer> = signer_alice
        .query(
            &ctx,
            "roflmarket.Offers",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0], create.offers[0]);

    // Update provider.
    let update = types::ProviderUpdate {
        provider: keys::alice::address(),
        nodes: vec![keys::bob::pk_ed25519().into()],
        scheduler_app: create.scheduler_app,
        payment_address: create.payment_address,
        metadata: create.metadata,
    };

    // Bob attempts to update the provider, but he doesn't have permission.
    let dispatch_result = signer_bob.call(&ctx, "roflmarket.ProviderUpdate", update.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Alice can update the provider.
    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderUpdate", update.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query the provider again.
    let provider: types::Provider = signer_alice
        .query(
            &ctx,
            "roflmarket.Provider",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(provider.nodes, update.nodes);

    // Query all providers.
    let providers: Vec<types::Provider> = signer_alice
        .query(&ctx, "roflmarket.Providers", ())
        .unwrap();
    assert_eq!(providers.len(), 1);

    // Update an offer.
    let update = types::ProviderUpdateOffers {
        provider: keys::alice::address(),
        update: vec![types::Offer {
            id: 0.into(),
            capacity: 2, // Bump capacity to 2.
            metadata: BTreeMap::from([("foo".to_string(), "bar".to_string())]),
            resources: types::Resources {
                tee: types::TeeType::TDX,
                memory: 512,
                cpus: 1,
                storage: 1024,
                gpu: None,
            },
            ..Default::default()
        }],
        ..Default::default()
    };

    // Bob attempts to update an offer, but he doesn't have permission.
    let dispatch_result = signer_bob.call(&ctx, "roflmarket.ProviderUpdateOffers", update.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Alice can update an offer.
    let dispatch_result =
        signer_alice.call(&ctx, "roflmarket.ProviderUpdateOffers", update.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query offer.
    let offer: types::Offer = signer_alice
        .query(
            &ctx,
            "roflmarket.Offer",
            types::OfferQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(offer, update.update[0]);

    // Remove provider.
    let remove = types::ProviderRemove {
        provider: keys::alice::address(),
    };

    // Bob attempts to remove the provider, but he doesn't have permission.
    let dispatch_result = signer_bob.call(&ctx, "roflmarket.ProviderRemove", remove.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Alice can remove the provider.
    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderRemove", remove.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query the provider again.
    signer_alice
        .query::<_, _, types::Provider>(
            &ctx,
            "roflmarket.Provider",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap_err();

    // Query all providers.
    let providers: Vec<types::Provider> = signer_alice
        .query(&ctx, "roflmarket.Providers", ())
        .unwrap();
    assert!(providers.is_empty(), "there should be no providers");
}

#[test]
fn test_instance_management() {
    let mut mock = mock::Mock::default();
    mock.epoch = 42;
    mock.runtime_header.timestamp = 1741778021;
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    // Summary of keys used for this test:
    //
    // alice: provider
    // bob: provider's node hosting the scheduler app
    // charlie: instance/app deployer
    // dave: provider's scheduler app
    // erin: random account
    //

    // Create the scheduler app.
    let create = rofl::types::Create {
        scheme: rofl::types::IdentifierScheme::CreatorNonce,
        ..Default::default()
    };

    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create);
    assert!(dispatch_result.result.is_success(), "call should succeed");
    let scheduler_app: rofl::app_id::AppId =
        cbor::from_value(dispatch_result.result.unwrap()).unwrap();

    // Create a provider.
    let create = types::ProviderCreate {
        nodes: vec![keys::bob::pk_ed25519().into()], // Bob seems like a nice node.
        scheduler_app,
        payment_address: types::PaymentAddress::Native(keys::alice::address()),
        offers: vec![types::Offer {
            payment: types::Payment::Native {
                denomination: Denomination::NATIVE,
                terms: BTreeMap::from([
                    (types::Term::Month, 10_000),
                    (types::Term::Year, 10_000_000), // Too expensive.
                ]),
            },
            resources: types::Resources {
                tee: types::TeeType::TDX,
                memory: 512,
                cpus: 1,
                storage: 1024,
                gpu: None,
            },
            capacity: 1,
            ..Default::default()
        }],
        metadata: Default::default(),
    };

    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderCreate", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Create a mock instance of the scheduler app where dave is an authorized key.
    let fake_registration = rofl::types::Registration {
        app: scheduler_app,
        extra_keys: vec![keys::dave::pk()],
        ..Default::default()
    };
    rofl::state::update_registration(fake_registration.clone()).unwrap();

    // Create an instance.
    let create = types::InstanceCreate {
        provider: keys::alice::address(),
        offer: 0.into(),
        admin: None, // Caller.
        deployment: Some(Default::default()),
        term: types::Term::Hour, // Hourly term does not exist for this offer.
        term_count: 1,
    };

    // Attempt to create an instance with a non-existent term which should fail.
    let mut signer_charlie = mock::Signer::new(0, keys::charlie::sigspec());
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCreate", create.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 10); // Payment failed.

    // Attempt to create an instance for a term that is too expensive for us should fail.
    let create = types::InstanceCreate {
        term: types::Term::Year, // The expensive one.
        ..create.clone()
    };
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCreate", create.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "accounts");
    assert_eq!(code, 2); // Insufficient balance.

    // Finally create an instance we can afford.
    let create = types::InstanceCreate {
        term: types::Term::Month,
        ..create.clone()
    };
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCreate", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");
    let result: types::InstanceId = cbor::from_value(dispatch_result.result.unwrap()).unwrap();
    assert_eq!(result, 0.into());

    // Query the instances.
    let instances: Vec<types::Instance> = signer_alice
        .query(
            &ctx,
            "roflmarket.Instances",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].provider, keys::alice::address());
    assert_eq!(instances[0].id, 0.into());
    assert_eq!(instances[0].offer, 0.into());
    assert_eq!(instances[0].status, types::InstanceStatus::Created);
    assert_eq!(instances[0].admin, keys::charlie::address());
    assert_eq!(instances[0].node_id, None);
    assert!(instances[0].metadata.is_empty());
    assert_eq!(instances[0].deployment, create.deployment);
    assert_eq!(instances[0].created_at, 1741778021);
    assert_eq!(instances[0].updated_at, 1741778021);
    assert_eq!(instances[0].paid_from, 1741778021);
    assert_eq!(instances[0].paid_until, 1741778021 + 30 * 24 * 60 * 60); // One month term.
    assert_eq!(
        instances[0].payment_address,
        [
            190, 103, 35, 233, 66, 149, 162, 148, 91, 11, 252, 184, 132, 97, 247, 193, 82, 63, 150,
            76
        ]
    );
    let charlie_address: Vec<u8> = keys::charlie::address().into();
    assert_eq!(instances[0].refund_data, charlie_address);

    // Ensure instance payment address has the correct amount of funds.
    let payment_address = Address::from_eth(&instances[0].payment_address);
    let balance = Accounts::get_balance(payment_address, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 10_000);

    // Query the provider metadata.
    let provider: types::Provider = signer_alice
        .query(
            &ctx,
            "roflmarket.Provider",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(provider.instances_count, 1);

    // Accept instance.
    let accept = types::InstanceAccept {
        provider: keys::alice::address(),
        ids: vec![0.into()],
        metadata: Default::default(),
    };

    // Only the scheduler app from the correct node should be allowed to accept.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceAccept", accept.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Scheduler app (dave), but incorrect node should still be forbidden.
    let mut signer_dave = mock::Signer::new(0, keys::dave::sigspec());
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceAccept", accept.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Update scheduler app's mock registration such that it is endorsed by the right node.
    let fake_registration = rofl::types::Registration {
        node_id: keys::bob::pk_ed25519().into(), // Bob is a nice approved node.
        ..fake_registration.clone()
    };
    rofl::state::update_registration(fake_registration.clone()).unwrap();

    // Scheduler app from the correct node should be allowed to accept.
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceAccept", accept.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query instance.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.status, types::InstanceStatus::Accepted);
    assert_eq!(instance.node_id, Some(keys::bob::pk_ed25519().into()));

    // Query offer.
    let offer: types::Offer = signer_alice
        .query(
            &ctx,
            "roflmarket.Offer",
            types::OfferQuery {
                provider: keys::alice::address(),
                id: instance.offer,
            },
        )
        .unwrap();
    assert_eq!(offer.capacity, 0, "offer capacity should be updated");

    // Update instance metadata.
    let update = types::InstanceUpdate {
        provider: keys::alice::address(),
        updates: vec![types::Update {
            id: 0.into(),
            deployment: Some(None.into()),
            metadata: Some(BTreeMap::from([("foo".to_string(), "bar".to_string())])),
            // Other fields are unchanged.
            ..Default::default()
        }],
    };

    // Only the scheduler app from the correct node should be allowed to update metadata.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceUpdate", update.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Query instance.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.metadata.get("foo"), None);

    // Scheduler app from the correct node should be allowed to update metadata.
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceUpdate", update.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query instance.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.metadata.get("foo"), Some(&"bar".to_string()));
    assert_eq!(instance.deployment, None);

    // Top-up instance.
    let topup = types::InstanceTopUp {
        provider: keys::alice::address(),
        id: 0.into(),
        term: types::Term::Year, // The expensive one.
        term_count: 2,
    };

    // Attempting to top-up for a term that one doesn't have the funds for should fail.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceTopUp", topup.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "accounts");
    assert_eq!(code, 2); // Insufficient balance.

    // Top-up for a sane term.
    let topup = types::InstanceTopUp {
        term: types::Term::Month,
        ..topup.clone()
    };
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceTopUp", topup.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure payment address now has additional funds.
    let balance = Accounts::get_balance(payment_address, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 30_000); // 10_000 from before and 2*10_000 for the top-up.

    // Ensure paid_until timestamp has been correctly updated.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.paid_from, 1741778021); // Nothing has been claimed yet.
    assert_eq!(instance.paid_until, 1741778021 + 3 * 30 * 24 * 60 * 60); // Three months in total.

    // Advance current time so the provider can claim some funds.
    mock.runtime_header.timestamp += 24 * 60 * 60; // One day.
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    // Claim payment.
    let claim = types::InstanceClaimPayment {
        provider: keys::alice::address(),
        instances: vec![0.into()],
    };

    // Only the scheduler app from the correct node should be allowed to claim payment.
    let dispatch_result =
        signer_charlie.call(&ctx, "roflmarket.InstanceClaimPayment", claim.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    let prev_balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();

    // Scheduler app from the correct node should be allowed to claim payment.
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceClaimPayment", claim.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure payment address now has less funds.
    let balance = Accounts::get_balance(payment_address, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 29_667); // 30_000 - 1/90 * 30_000.

    // Ensure paid_from timestamp has been correctly updated.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.paid_from, 1741778021 + 24 * 60 * 60); // One day has been claimed.
    assert_eq!(instance.paid_until, 1741778021 + 3 * 30 * 24 * 60 * 60); // Nothing has changed.

    // Ensure provider's payment address now has more funds.
    let new_balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert!(new_balance > prev_balance);
    assert_eq!(new_balance - prev_balance, 333); // 30_000 - 29_667.

    // Execute instance commands.
    let exec = types::InstanceExecuteCmds {
        provider: keys::alice::address(),
        id: 0.into(),
        cmds: vec![
            b"do the thing. now!".to_vec(),
            b"and then one more.".to_vec(),
            b"and the last one.".to_vec(),
        ],
    };

    // Only the instance admin should be allowed to execute commands.
    let mut signer_erin = mock::Signer::new(0, keys::erin::sigspec());
    let dispatch_result = signer_erin.call(&ctx, "roflmarket.InstanceExecuteCmds", exec.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Instance admin should be allowed to execute commands.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceExecuteCmds", exec.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure command count has been correctly updated.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.cmd_count, 3);

    // Query instance commands.
    let cmds: Vec<types::QueuedCommand> = signer_alice
        .query(
            &ctx,
            "roflmarket.InstanceCommands",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0].id, 0.into());
    assert_eq!(&cmds[0].cmd, b"do the thing. now!");
    assert_eq!(cmds[1].id, 1.into());
    assert_eq!(&cmds[1].cmd, b"and then one more.");
    assert_eq!(cmds[2].id, 2.into());
    assert_eq!(&cmds[2].cmd, b"and the last one.");

    // Complete instance commands.
    let complete = types::InstanceUpdate {
        provider: keys::alice::address(),
        updates: vec![types::Update {
            id: 0.into(),
            last_completed_cmd: Some(0.into()), // Complete the first command.
            // Leave the rest unchanged.
            ..Default::default()
        }],
    };

    // Only the scheduler app from the correct node should be allowed to complete commands.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceUpdate", complete.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Scheduler app from the correct node should be allowed to complete commands.
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceUpdate", complete.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure command count has been correctly updated.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.cmd_count, 2);

    // Query instance commands.
    let cmds: Vec<types::QueuedCommand> = signer_alice
        .query(
            &ctx,
            "roflmarket.InstanceCommands",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].id, 1.into());
    assert_eq!(&cmds[0].cmd, b"and then one more.");
    assert_eq!(cmds[1].id, 2.into());
    assert_eq!(&cmds[1].cmd, b"and the last one.");

    // Cancel instance. Since it happened outside the acceptance window, the provider may claim the
    // entire amount.
    let cancel = types::InstanceCancel {
        provider: keys::alice::address(),
        id: 0.into(),
    };

    // Only the instance admin should be allowed to cancel the instance.
    let dispatch_result = signer_erin.call(&ctx, "roflmarket.InstanceCancel", cancel.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Instance admin should be allowed to cancel the instance.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCancel", cancel.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Ensure provider's payment address now has more funds.
    let new_balance = Accounts::get_balance(keys::alice::address(), Denomination::NATIVE).unwrap();
    assert!(new_balance > prev_balance);
    assert_eq!(new_balance - prev_balance, 30_000); // The entire amount.

    // Ensure instance has been correctly updated.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(instance.status, types::InstanceStatus::Cancelled);
    assert_eq!(instance.paid_from, 1741778021 + 3 * 30 * 24 * 60 * 60); // Everything has been claimed.
    assert_eq!(instance.paid_until, 1741778021 + 3 * 30 * 24 * 60 * 60);

    // Attempt to remove provider with instances. It should fail.
    let remove = types::ProviderRemove {
        provider: keys::alice::address(),
    };

    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderRemove", remove.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 5); // Provider has instances.

    // Remove instance.
    let remove = types::InstanceRemove {
        provider: keys::alice::address(),
        id: 0.into(),
    };

    // Only the scheduler app from the correct node should be allowed to remove instances.
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceRemove", remove.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 4); // Forbidden.

    // Scheduler app from the correct node should be allowed to remove instances.
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceRemove", remove.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query the instances.
    let instances: Vec<types::Instance> = signer_alice
        .query(
            &ctx,
            "roflmarket.Instances",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(instances.len(), 0, "instance should be removed");

    // Query instance commands.
    let cmds: Vec<types::QueuedCommand> = signer_alice
        .query(
            &ctx,
            "roflmarket.InstanceCommands",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(cmds.len(), 0, "instance commands should be removed");

    // Query offer.
    let offer: types::Offer = signer_alice
        .query(
            &ctx,
            "roflmarket.Offer",
            types::OfferQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();
    assert_eq!(offer.capacity, 1, "offer capacity should be updated");

    // Query the provider metadata.
    let provider: types::Provider = signer_alice
        .query(
            &ctx,
            "roflmarket.Provider",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(
        provider.instances_count, 0,
        "instance count should be decremented"
    );

    // Remove provider.
    let remove = types::ProviderRemove {
        provider: keys::alice::address(),
    };

    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderRemove", remove.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Query all providers.
    let providers: Vec<types::Provider> = signer_alice
        .query(&ctx, "roflmarket.Providers", ())
        .unwrap();
    assert!(providers.is_empty(), "there should be no providers");

    // Query offers.
    let offers: Vec<types::Offer> = signer_alice
        .query(
            &ctx,
            "roflmarket.Offers",
            types::ProviderQuery {
                provider: keys::alice::address(),
            },
        )
        .unwrap();
    assert_eq!(offers.len(), 0, "there should be no offers");
}

#[test]
fn test_instance_accept_timeout() {
    let mut mock = mock::Mock::default();
    mock.epoch = 42;
    mock.runtime_header.timestamp = 1741778021;
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    TestRuntime::migrate(&ctx);

    // Summary of keys used for this test:
    //
    // alice: provider
    // bob: provider's node hosting the scheduler app
    // charlie: instance/app deployer
    // dave: provider's scheduler app
    //

    // Create the scheduler app.
    let create = rofl::types::Create {
        scheme: rofl::types::IdentifierScheme::CreatorNonce,
        ..Default::default()
    };

    let mut signer_alice = mock::Signer::new(0, keys::alice::sigspec());
    let dispatch_result = signer_alice.call(&ctx, "rofl.Create", create);
    assert!(dispatch_result.result.is_success(), "call should succeed");
    let scheduler_app: rofl::app_id::AppId =
        cbor::from_value(dispatch_result.result.unwrap()).unwrap();

    // Create a provider.
    let create = types::ProviderCreate {
        nodes: vec![keys::bob::pk_ed25519().into()], // Bob seems like a nice node.
        scheduler_app,
        payment_address: types::PaymentAddress::Native(keys::alice::address()),
        offers: vec![types::Offer {
            payment: types::Payment::Native {
                denomination: Denomination::NATIVE,
                terms: BTreeMap::from([(types::Term::Month, 10_000)]),
            },
            resources: types::Resources {
                tee: types::TeeType::TDX,
                memory: 512,
                cpus: 1,
                storage: 1024,
                gpu: None,
            },
            capacity: 1,
            ..Default::default()
        }],
        metadata: Default::default(),
    };

    let dispatch_result = signer_alice.call(&ctx, "roflmarket.ProviderCreate", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Create a mock instance of the scheduler app where dave is an authorized key.
    let fake_registration = rofl::types::Registration {
        app: scheduler_app,
        node_id: keys::bob::pk_ed25519().into(), // Bob is a nice approved node.
        extra_keys: vec![keys::dave::pk()],
        ..Default::default()
    };
    rofl::state::update_registration(fake_registration).unwrap();

    // Create an instance.
    let create = types::InstanceCreate {
        provider: keys::alice::address(),
        offer: 0.into(),
        admin: None, // Caller.
        deployment: None,
        term: types::Term::Month,
        term_count: 1,
    };

    let mut signer_charlie = mock::Signer::new(0, keys::charlie::sigspec());
    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCreate", create.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Simulate the provider not accepting an instance so that the accept timeout passes.
    mock.runtime_header.timestamp += 1800; // Default accept timeout is 300 sec.
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    // Make sure the provider cannot claim anything in case the instance has not been accepted.
    let claim = types::InstanceClaimPayment {
        provider: keys::alice::address(),
        instances: vec![0.into()],
    };

    let mut signer_dave = mock::Signer::new(0, keys::dave::sigspec());
    let dispatch_result = signer_dave.call(&ctx, "roflmarket.InstanceClaimPayment", claim.clone());
    assert!(!dispatch_result.result.is_success(), "call should fail");
    let (module, code) = dispatch_result.result.unwrap_failed();
    assert_eq!(module, "roflmarket");
    assert_eq!(code, 1); // Invalid argument.

    // Query instance.
    let instance: types::Instance = signer_alice
        .query(
            &ctx,
            "roflmarket.Instance",
            types::InstanceQuery {
                provider: keys::alice::address(),
                id: 0.into(),
            },
        )
        .unwrap();

    // Instance should hold its prepayment.
    let payment_address = Address::from_eth(&instance.payment_address);
    let balance = Accounts::get_balance(payment_address, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 10_000);

    // The deployer has enough of waiting so a cancellation is issued.
    let cancel = types::InstanceCancel {
        provider: keys::alice::address(),
        id: 0.into(),
    };

    let dispatch_result = signer_charlie.call(&ctx, "roflmarket.InstanceCancel", cancel.clone());
    assert!(dispatch_result.result.is_success(), "call should succeed");

    // Instance should be fully refunded.
    let payment_address = Address::from_eth(&instance.payment_address);
    let balance = Accounts::get_balance(payment_address, Denomination::NATIVE).unwrap();
    assert_eq!(balance, 0);
    let balance = Accounts::get_balance(keys::charlie::address(), Denomination::NATIVE).unwrap();
    assert_eq!(balance, 1_000_000);
}
