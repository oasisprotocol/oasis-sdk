use evm::Context;
use oasis_runtime_sdk::{modules::accounts::Module, types::token::Denomination};
pub use primitive_types::H160;

use super::{get_precompiles, PrecompileResult};

struct TestConfig;

impl crate::Config for TestConfig {
    type Accounts = Module;

    const CHAIN_ID: u64 = 0;

    const TOKEN_DENOMINATION: Denomination = Denomination::NATIVE;

    const CONFIDENTIAL: bool = true;
}

pub fn call_contract(address: H160, input: &[u8], target_gas: u64) -> Option<PrecompileResult> {
    let context: Context = Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: From::from(0),
    };
    let map = get_precompiles::<TestConfig>();
    map.get(&address)
        .map(|pf| pf(input, Some(target_gas), &context, false))
}
