use evm::{executor::stack::PrecompileSet, Context};
use oasis_runtime_sdk::{modules::accounts::Module, types::token::Denomination};
pub use primitive_types::H160;

use super::{PrecompileResult, Precompiles};

struct TestConfig;

impl crate::Config for TestConfig {
    type Accounts = Module;

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
}

pub fn call_contract(address: H160, input: &[u8], target_gas: u64) -> Option<PrecompileResult> {
    let context: Context = Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: From::from(0),
    };
    let precompiles: Precompiles<'_, TestConfig, MockBackend> = Precompiles::new(&MockBackend);
    precompiles.execute(address, input, Some(target_gas), &context, false)
}
