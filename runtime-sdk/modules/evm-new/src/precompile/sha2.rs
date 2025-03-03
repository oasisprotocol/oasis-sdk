//! Implements SHA2 precompiles.
use revm::{
    precompile::{calc_linear_cost_u32, PrecompileError, PrecompileOutput, PrecompileResult},
    primitives::Bytes,
};

macro_rules! make_hasher {
    ($name:ident, $hasher:ident) => {
        pub(super) fn $name(input: &Bytes, gas_limit: u64) -> PrecompileResult {
            // Costs were computed by benchmarking and comparing to SHA256
            // and using the SHA256 costs (defined by EVM spec).
            // See benches/criterion_benchmark.rs for the benchmarks.
            let cost = calc_linear_cost_u32(input.len(), 115, 13);
            if cost > gas_limit {
                Err(PrecompileError::OutOfGas.into())
            } else {
                let output = <sha2::$hasher as sha2::Digest>::digest(input).to_vec();
                Ok(PrecompileOutput::new(cost, output.into()))
            }
        }
    };
}

make_hasher!(call_sha512_256, Sha512_256);
make_hasher!(call_sha384, Sha384);
make_hasher!(call_sha512, Sha512);
