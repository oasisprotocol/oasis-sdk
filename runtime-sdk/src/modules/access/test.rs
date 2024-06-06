//! Tests for the method access control module.
use std::collections::BTreeMap;

use once_cell::unsync::Lazy;

use crate::{
    context::Context,
    crypto::signature::context as signature_context,
    handler,
    module::{self, Module},
    modules::{self, core},
    sdk_derive,
    testing::{keys, mock},
    types::{
        token::{BaseUnits, Denomination},
        transaction,
    },
    Runtime, Version,
};

use super::{
    types::{Authorization, MethodAuthorization},
    Error as AccessError,
};

struct TestConfig;

impl core::Config for TestConfig {}

impl modules::access::Config for TestConfig {
    const METHOD_AUTHORIZATIONS: Lazy<Authorization> = Lazy::new(|| {
        Authorization::with_filtered_methods([(
            "test.FilteredMethod",
            MethodAuthorization::allow_from([keys::alice::address()]),
        )])
    });
}

/// Test runtime.
struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = modules::core::Module<TestConfig>;
    type Accounts = modules::accounts::Module;

    type Modules = (
        modules::core::Module<TestConfig>,
        modules::accounts::Module,
        modules::access::Module<TestConfig>,
        TestModule,
    );

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: 10_000_000,
                    min_gas_price: BTreeMap::from([(Denomination::NATIVE, 0)]),
                    ..Default::default()
                },
            },
            modules::accounts::Genesis {
                balances: BTreeMap::from([
                    (
                        keys::alice::address(),
                        BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                    ),
                    (
                        keys::bob::address(),
                        BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                    ),
                ]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 2_000_000)]),
                ..Default::default()
            },
            (), // Access module has no genesis.
            (), // Test module has no genesis.
        )
    }
}

/// A module with multiple no-op methods; intended for testing routing.
struct TestModule;

#[sdk_derive(Module)]
impl TestModule {
    const NAME: &'static str = "test";
    type Error = core::Error;
    type Event = ();
    type Parameters = ();
    type Genesis = ();

    #[handler(call = "test.FilteredMethod")]
    fn filtered_method<C: Context>(_ctx: &C, fail: bool) -> Result<u64, core::Error> {
        Ok(42)
    }

    #[handler(call = "test.AllowedMethod")]
    fn allowed_method<C: Context>(ctx: &C, _args: ()) -> Result<u64, core::Error> {
        Ok(42)
    }
}

impl module::BlockHandler for TestModule {}
impl module::TransactionHandler for TestModule {}
impl module::InvariantHandler for TestModule {}

fn dispatch_test<C: Context>(
    ctx: &C,
    signer: &mut mock::Signer,
    meth: &str,
    encrypted: bool,
    should_fail: bool,
) {
    let dispatch_result = signer.call_opts(
        ctx,
        meth,
        (),
        mock::CallOptions {
            fee: transaction::Fee {
                amount: BaseUnits::new(1_500, Denomination::NATIVE),
                gas: 1_500,
                ..Default::default()
            },
            encrypted,
            ..Default::default()
        },
    );
    if should_fail {
        let err = core::Error::InvalidArgument(AccessError::NotAuthorized.into());
        assert!(
            matches!(
                dispatch_result.result,
                module::CallResult::Failed { module: _, code: _, message: m } if m == format!("{}", err),
            ),
            "method call should be blocked",
        );
    } else {
        assert!(
            dispatch_result.result.is_success(),
            "method call should succeed but failed with: {:?}",
            dispatch_result.result,
        );
        let unmarshalled: u64 =
            cbor::from_value(dispatch_result.result.unwrap()).expect("result should be decodable");
        assert_eq!(unmarshalled, 42);
    }
}

#[test]
fn test_access_module() {
    let _guard = signature_context::test_using_chain_context();
    signature_context::set_chain_context(Default::default(), "test");
    let mut mock = mock::Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);

    let mut alice = mock::Signer::new(0, keys::alice::sigspec());
    let mut bob = mock::Signer::new(0, keys::bob::sigspec());

    TestRuntime::migrate(&ctx);

    let filtered = "test.FilteredMethod";
    let allowed = "test.AllowedMethod";

    // Test plain calls.

    dispatch_test(&ctx, &mut alice, filtered, false, false);
    dispatch_test(&ctx, &mut alice, allowed, false, false);

    dispatch_test(&ctx, &mut bob, filtered, false, true);
    dispatch_test(&ctx, &mut bob, allowed, false, false);

    // Test encrypted calls.

    dispatch_test(&ctx, &mut alice, filtered, true, false);
    dispatch_test(&ctx, &mut alice, allowed, true, false);

    dispatch_test(&ctx, &mut bob, filtered, true, true);
    dispatch_test(&ctx, &mut bob, allowed, true, false);
}

#[test]
fn test_method_authorization() {
    let alice = keys::alice::address();
    let bob = keys::bob::address();
    let charlie = keys::charlie::address();

    // An empty authorizer shouldn't let anybody through.
    let empty = MethodAuthorization::allow_from([]);
    assert_eq!(empty.is_authorized(&alice), false);
    assert_eq!(empty.is_authorized(&bob), false);
    assert_eq!(empty.is_authorized(&charlie), false);

    // An authorizer with some addresses should only let those through.
    let for_alice = MethodAuthorization::allow_from([alice]);
    assert_eq!(for_alice.is_authorized(&alice), true);
    assert_eq!(for_alice.is_authorized(&bob), false);
    assert_eq!(for_alice.is_authorized(&charlie), false);

    let for_bob = MethodAuthorization::allow_from([bob]);
    assert_eq!(for_bob.is_authorized(&alice), false);
    assert_eq!(for_bob.is_authorized(&bob), true);
    assert_eq!(for_bob.is_authorized(&charlie), false);
}

#[test]
fn test_authorization() {
    let alice = keys::alice::address();
    let bob = keys::bob::address();
    let charlie = keys::charlie::address();
    let dave = keys::dave::address();

    let authorization = Authorization::with_filtered_methods([
        ("test.Nobody", MethodAuthorization::allow_from([])),
        ("test.Alice", MethodAuthorization::allow_from([alice])),
        ("test.Bob", MethodAuthorization::allow_from([bob])),
        ("test.Both", MethodAuthorization::allow_from([alice, bob])),
        (
            "test.AliceAndCharlie",
            MethodAuthorization::allow_from([alice, charlie]),
        ),
    ]);

    // Alice should be able to access some filtered methods and all unfiltered ones.
    assert_eq!(authorization.is_authorized("test.Nobody", &alice), false);
    assert_eq!(authorization.is_authorized("test.Alice", &alice), true);
    assert_eq!(authorization.is_authorized("test.Bob", &alice), false);
    assert_eq!(authorization.is_authorized("test.Both", &alice), true);
    assert_eq!(
        authorization.is_authorized("test.AliceAndCharlie", &alice),
        true
    );
    assert_eq!(authorization.is_authorized("test.Everybody", &alice), true);

    // Bob should be able to access some filtered methods and all unfiltered ones.
    assert_eq!(authorization.is_authorized("test.Nobody", &bob), false);
    assert_eq!(authorization.is_authorized("test.Alice", &bob), false);
    assert_eq!(authorization.is_authorized("test.Bob", &bob), true);
    assert_eq!(authorization.is_authorized("test.Both", &bob), true);
    assert_eq!(
        authorization.is_authorized("test.AliceAndCharlie", &bob),
        false
    );
    assert_eq!(authorization.is_authorized("test.Everybody", &bob), true);

    // Charlie should be able to access some filtered methods and all unfiltered ones.
    assert_eq!(authorization.is_authorized("test.Nobody", &charlie), false);
    assert_eq!(authorization.is_authorized("test.Alice", &charlie), false);
    assert_eq!(authorization.is_authorized("test.Bob", &charlie), false);
    assert_eq!(authorization.is_authorized("test.Both", &charlie), false);
    assert_eq!(
        authorization.is_authorized("test.AliceAndCharlie", &charlie),
        true
    );
    assert_eq!(
        authorization.is_authorized("test.Everybody", &charlie),
        true
    );

    // Dave is left out of everything, so should only be able to access unfiltered methods.
    assert_eq!(authorization.is_authorized("test.Nobody", &dave), false);
    assert_eq!(authorization.is_authorized("test.Alice", &dave), false);
    assert_eq!(authorization.is_authorized("test.Bob", &dave), false);
    assert_eq!(authorization.is_authorized("test.Both", &dave), false);
    assert_eq!(
        authorization.is_authorized("test.AliceAndCharlie", &dave),
        false
    );
    assert_eq!(authorization.is_authorized("test.Everybody", &dave), true);
}
