/*use ethabi::{ParamType, Token};
use revm::{
    precompile::{PrecompileError, PrecompileOutput, PrecompileResult},
    primitives::{Bytes, Env},
};

const GAS_USED_COST: u64 = 10;
const PAD_GAS_COST: u64 = 10;

pub(super) fn call_gas_used(_input: &Bytes, gas_limit: u64, _env: &Env) -> PrecompileResult {
    let cost = GAS_USED_COST;
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // TODO
    let used_gas = cost; // handle.used_gas(); // XXX

    let output = ethabi::encode(&[Token::Uint(used_gas.into())]);
    Ok(PrecompileOutput::new(cost, output.into()))
}

pub(super) fn call_pad_gas(input: &Bytes, gas_limit: u64, _env: &Env) -> PrecompileResult {
    let cost = PAD_GAS_COST;
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Decode args.
    let mut call_args = ethabi::decode(&[ParamType::Uint(128)], input)
        .map_err(|e| PrecompileError::Other(e.to_string()))?;
    let gas_amount_big = call_args.pop().unwrap().into_uint().unwrap();
    let gas_amount = gas_amount_big.try_into().unwrap_or(u64::MAX);

    // Obtain total used gas so far.
    let used_gas = cost; // handle.used_gas(); // XXX

    // Fail if more gas that the desired padding was already used.
    if gas_amount < used_gas {
        return Err(PrecompileError::Other(
            "gas pad amount less than already used gas".to_string(),
        )
        .into());
    }

    // Record the remainder so that the gas use is padded to the desired amount.
    // TODO
    //handle.record_cost(gas_amount - used_gas)?; // XXX

    Ok(PrecompileOutput::new(cost, Bytes::new()))
}
*/
