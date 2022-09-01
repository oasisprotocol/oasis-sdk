use evm::Context;
pub use primitive_types::H160;

use super::{PrecompileResult, PRECOMPILED_CONTRACT};

pub fn call_contract(address: H160, input: &[u8], target_gas: u64) -> Option<PrecompileResult> {
    let context: Context = Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: From::from(0),
    };
    PRECOMPILED_CONTRACT
        .get(&address)
        .map(|pf| pf(input, Some(target_gas), &context, false))
}
