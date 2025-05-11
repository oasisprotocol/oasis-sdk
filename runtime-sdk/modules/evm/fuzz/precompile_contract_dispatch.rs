use honggfuzz::fuzz;

use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    Context, ExitError, ExitReason, ExitSucceed, Transfer,
};
use primitive_types::{H160, H256};

use oasis_runtime_sdk_evm::{
    self, // proc-macros need this for name resolution.
    precompile::contract::StaticContract,
};
use oasis_runtime_sdk_macros::{evm_contract_address, evm_method, sdk_derive};

// We only need this to provide call data for the dispatcher to munch on.
struct MockPrecompileHandle<'a> {
    input: &'a [u8],
}

impl PrecompileHandle for MockPrecompileHandle<'_> {
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

    fn record_cost(&mut self, _cost: u64) -> Result<(), ExitError> {
        unimplemented!()
    }

    fn remaining_gas(&self) -> u64 {
        unimplemented!()
    }

    fn log(&mut self, _address: H160, _topics: Vec<H256>, _data: Vec<u8>) -> Result<(), ExitError> {
        unimplemented!()
    }

    fn code_address(&self) -> H160 {
        unimplemented!()
    }

    fn input(&self) -> &[u8] {
        self.input
    }

    fn context(&self) -> &Context {
        unimplemented!()
    }

    fn is_static(&self) -> bool {
        unimplemented!()
    }

    fn gas_limit(&self) -> Option<u64> {
        unimplemented!()
    }

    fn record_external_cost(
        &mut self,
        _ref_time: Option<u64>,
        _proof_size: Option<u64>,
        _storage_growth: Option<u64>,
    ) -> Result<(), ExitError> {
        unimplemented!()
    }

    fn refund_external_cost(&mut self, _ref_time: Option<u64>, _proof_size: Option<u64>) {
        unimplemented!()
    }

    fn used_gas(&self) -> u64 {
        unimplemented!()
    }
}

#[derive(Default)]
struct Contract {}

#[sdk_derive(EvmContract)]
impl Contract {
    #[evm_contract_address]
    fn address() -> H160 {
        H160::zero()
    }

    // selector: 222b1407
    #[evm_method(signature = "direct()")]
    fn direct(
        _handle: &mut impl PrecompileHandle,
        _input_offset: usize,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: vec![],
        })
    }

    // selector d52bce59
    #[evm_method(signature = "arg(address,uint256)", convert)]
    fn arg(
        _handle: &mut impl PrecompileHandle,
        _address: H160,
        _value: u128,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: vec![],
        })
    }
}

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            let mut precompile_handle = MockPrecompileHandle { input: data };

            <Contract as StaticContract>::dispatch_call(&mut precompile_handle);
        });
    }
}
