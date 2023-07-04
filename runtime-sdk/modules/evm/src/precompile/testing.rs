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
    precompiles.execute(&mut handle)
}
