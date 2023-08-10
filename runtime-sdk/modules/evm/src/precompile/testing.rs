use evm::{
    executor::stack::{PrecompileHandle, PrecompileSet},
    Context, ExitError, ExitReason, Transfer,
};
pub use primitive_types::{H160, H256};

use oasis_runtime_sdk::{
    modules::{accounts::Module, core::Error},
    subcall,
    types::token::Denomination,
};

use super::{PrecompileResult, Precompiles};

struct TestConfig;

impl crate::Config for TestConfig {
    type Accounts = Module;

    type AdditionalPrecompileSet = ();

    const CHAIN_ID: u64 = 0;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;

    const CONFIDENTIAL: bool = true;
}

struct MockBackend;
impl crate::backend::EVMBackendExt for MockBackend {
    fn random_bytes(&self, num_bytes: u64, pers: &[u8]) -> Vec<u8> {
        pers.iter()
            .copied()
            .chain((pers.len()..(num_bytes as usize)).map(|i| i as u8))
            .collect()
    }

    fn subcall<V: subcall::Validator + 'static>(
        &self,
        _info: subcall::SubcallInfo,
        _validator: V,
    ) -> Result<subcall::SubcallResult, Error> {
        unimplemented!()
    }
}

struct MockPrecompileHandle<'a> {
    address: H160,
    input: &'a [u8],
    context: &'a Context,
    gas_limit: u64,
    gas_cost: u64,
}

impl<'a> PrecompileHandle for MockPrecompileHandle<'a> {
    fn call(
        &mut self,
        _to: H160,
        _transfer: Option<Transfer>,
        _input: Vec<u8>,
        _gas_limit: Option<u64>,
        _is_static: bool,
        _context: &Context,
    ) -> (ExitReason, Vec<u8>) {
        unimplemented!()
    }

    fn record_cost(&mut self, cost: u64) -> Result<(), ExitError> {
        if self.remaining_gas() < cost {
            return Err(ExitError::OutOfGas);
        }
        self.gas_cost = self.gas_cost.saturating_add(cost);

        Ok(())
    }

    fn remaining_gas(&self) -> u64 {
        self.gas_limit.saturating_sub(self.gas_cost)
    }

    fn log(&mut self, _address: H160, _topics: Vec<H256>, _data: Vec<u8>) -> Result<(), ExitError> {
        Ok(())
    }

    fn code_address(&self) -> H160 {
        self.address
    }

    fn input(&self) -> &[u8] {
        self.input
    }

    fn context(&self) -> &Context {
        self.context
    }

    fn is_static(&self) -> bool {
        false
    }

    fn gas_limit(&self) -> Option<u64> {
        Some(self.gas_limit)
    }
}

#[doc(hidden)]
pub fn call_contract(address: H160, input: &[u8], gas_limit: u64) -> Option<PrecompileResult> {
    call_contract_with_gas_report(address, input, gas_limit).map(|(result, _)| result)
}

#[doc(hidden)]
pub fn call_contract_with_gas_report(
    address: H160,
    input: &[u8],
    gas_limit: u64,
) -> Option<(PrecompileResult, u64)> {
    let context: Context = Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: From::from(0),
    };
    let precompiles: Precompiles<'_, TestConfig, MockBackend> = Precompiles::new(&MockBackend);
    let mut handle = MockPrecompileHandle {
        address,
        input,
        context: &context,
        gas_limit,
        gas_cost: 0,
    };
    precompiles
        .execute(&mut handle)
        .map(|result| (result, handle.gas_cost))
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
