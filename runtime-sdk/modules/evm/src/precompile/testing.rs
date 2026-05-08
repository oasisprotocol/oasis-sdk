pub use primitive_types::{H160, H256};

use oasis_runtime_sdk::{
    context,
    core::storage::mkvs,
    module::{self, CallResult},
    modules::{
        accounts,
        core::{self},
    },
    storage::MKVSStore,
    testing::{
        keys,
        mock::{CallOptions, Mock},
    },
    types::{
        token::{self, Denomination},
        transaction::Fee,
    },
    CurrentState, Runtime, Version,
};

use crate::{
    mock::{load_contract_bytecode, EvmSigner},
    types::{self},
};

use std::collections::BTreeMap;

pub(crate) struct TestConfig;

impl crate::Config for TestConfig {
    const CHAIN_ID: u64 = 0;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;

    const CONFIDENTIAL: bool = true;
}

/// Test case for precompiled contract tests.
#[cfg(any(test, feature = "test"))]
#[derive(serde::Deserialize)]
pub struct TestCase {
    #[serde(rename = "Input")]
    pub input: String,

    #[serde(rename = "Expected")]
    pub expected: String,

    #[serde(rename = "Name")]
    pub _name: String,

    #[serde(default)]
    #[serde(rename = "Gas")]
    pub gas: u64,

    #[serde(default)]
    #[serde(rename = "NoBenchmark")]
    pub _no_benchmark: bool,
}

/// Reads test cases from the specified file.
///
/// The test cases are from "go-ethereum/core/vm/testdata/precompiles"
/// and from "frontier/frame/evm/precompile/testdata".
///
/// See https://github.com/ethereum/go-ethereum/tree/master/core/vm/testdata/precompiles and
/// https://github.com/paritytech/frontier/tree/master/frame/evm/precompile/testdata.
#[cfg(any(test, feature = "test"))]
pub fn read_test_cases(name: &str) -> Vec<TestCase> {
    let path = format!("src/precompile/testdata/{name}.json");
    let contents = std::fs::read_to_string(path).expect("json file should be readable");

    serde_json::from_str(&contents).expect("json decoding should succeed")
}

type Core = core::Module<TestConfig>;
type Accounts = accounts::Module;
type Evm = crate::Module<TestConfig>;

impl core::Config for TestConfig {}

pub(crate) struct TestRuntime;

impl Runtime for TestRuntime {
    const VERSION: Version = Version::new(0, 0, 0);
    type Core = Core;
    type Accounts = Accounts;
    type Modules = (Core, Accounts, Evm);

    fn genesis_state() -> <Self::Modules as module::MigrationHandler>::Genesis {
        (
            core::Genesis {
                parameters: core::Parameters {
                    max_batch_gas: u64::MAX,
                    max_tx_size: 32 * 1024,
                    max_tx_signers: 1,
                    max_multisig_signers: 8,
                    gas_costs: Default::default(),
                    min_gas_price: BTreeMap::from([(token::Denomination::NATIVE, 0)]),
                    dynamic_min_gas_price: Default::default(),
                },
            },
            accounts::Genesis {
                balances: BTreeMap::from([(
                    keys::dave::address(),
                    BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                )]),
                total_supplies: BTreeMap::from([(Denomination::NATIVE, 1_000_000)]),
                ..Default::default()
            },
            crate::Genesis {
                ..Default::default()
            },
        )
    }
}

#[cfg(any(test, feature = "test"))]
pub fn init_and_deploy_contract<C: context::Context>(
    ctx: &C,
    signer: &mut EvmSigner,
    bytecode: &str,
) -> H160 {
    TestRuntime::migrate(ctx);

    let test_contract = load_contract_bytecode(bytecode);

    // Create contract.
    let dispatch_result = signer.call(
        ctx,
        "evm.Create",
        types::Create {
            value: 0.into(),
            init_code: test_contract,
        },
    );
    let result = dispatch_result.result.unwrap();
    let result: Vec<u8> = cbor::from_value(result).unwrap();
    H160::from_slice(&result)
}

#[doc(hidden)]
pub fn call_contract(address: H160, input: &[u8], gas_limit: u64) -> Result<Vec<u8>, String> {
    let mut mock = Mock::default();
    let ctx = mock.create_ctx_for_runtime::<TestRuntime>(true);
    let mut signer = EvmSigner::new(0, keys::dave::sigspec());

    // Ensure we always start with a clean state.
    let root = mkvs::OverlayTree::new(
        mkvs::Tree::builder()
            .with_root_type(mkvs::RootType::State)
            .build(Box::new(mkvs::sync::NoopReadSyncer)),
    );
    let root = MKVSStore::new(root);

    CurrentState::enter(root, || {
        TestRuntime::migrate(&ctx);

        let dispatch_result = signer.call_opts(
            &ctx,
            "evm.Call",
            types::Call {
                address: address.into(),
                value: 0.into(),
                data: input.to_vec(),
            },
            CallOptions {
                fee: Fee {
                    gas: gas_limit,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        match dispatch_result.result {
            CallResult::Ok(result) => {
                let result: Vec<u8> = cbor::from_value(result).unwrap();
                Ok(result)
            }
            CallResult::Failed {
                module,
                code,
                message,
            } => Err(format!("module: {module} code: {code} message: {message}")),
            CallResult::Aborted(err) => Err(format!("aborted: {err}")),
        }
    })
}
