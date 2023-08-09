//! Implements sha512_256 precompile.
use evm::{
    executor::stack::{PrecompileHandle, PrecompileOutput},
    ExitSucceed,
};
use ripemd160::Digest as _;
use sha2::{Sha512, Sha512Trunc256};

use super::{record_linear_cost, PrecompileResult};

pub(super) fn call_sha512_256(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    // Costs were computed by benchmarking and comparing to SHA256 and using the SHA256 costs (defined by EVM spec).
    // See benches/criterion_benchmark.rs for the benchmarks.
    record_linear_cost(handle, handle.input().len() as u64, 115, 13)?;

    let mut hasher = Sha512Trunc256::new();
    hasher.update(handle.input());
    let digest = hasher.finalize();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: digest.to_vec(),
    })
}

pub(super) fn call_sha512(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    // Same costs as SHA512_256.
    record_linear_cost(handle, handle.input().len() as u64, 115, 13)?;

    let mut hasher = Sha512::new();
    hasher.update(handle.input());
    let digest = hasher.finalize();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: digest.to_vec(),
    })
}

#[cfg(test)]
mod test {
    use super::super::testing::*;

    #[test]
    fn test_sha512_256() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x01,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "41f7883fc8df1d31b1b1f7c0379f7b5a990d457347d997fdd76a2f4bb5812342"
        );
    }

    #[test]
    fn test_sha512() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x02,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "2b80ea6632ad6148483537e6afe59b835bd989b4deb1f0e556e6c7cf30f979dbfc2dbd226e1e646fb202b82180faa3bcba6282573a99895956f7005845dd3a6a"
        );
    }
}
