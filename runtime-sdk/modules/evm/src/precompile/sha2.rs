//! Implements sha512_256 precompile.
use evm::{
    executor::stack::{PrecompileHandle, PrecompileOutput},
    ExitSucceed,
};

use super::{record_linear_cost, PrecompileResult};

macro_rules! make_hasher {
    ($name:ident, $hasher:ident) => {
        pub(super) fn $name(handle: &mut impl PrecompileHandle) -> PrecompileResult {
            // Costs were computed by benchmarking and comparing to SHA256 and using the SHA256 costs (defined by EVM spec).
            // See benches/criterion_benchmark.rs for the benchmarks.
            record_linear_cost(handle, handle.input().len() as u64, 115, 13)?;
            Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: <sha2::$hasher as sha2::Digest>::digest(handle.input()).to_vec(),
            })
        }
    };
}

make_hasher!(call_sha512_256, Sha512_256);
make_hasher!(call_sha384, Sha384);
make_hasher!(call_sha512, Sha512);

#[cfg(test)]
mod test {
    use super::super::testing::*;

    macro_rules! make_hasher_test {
        ($name:ident, $ix:literal, $hasher:ident) => {
            #[test]
            fn $name() {
                let input_bytes = hex::decode(
                    "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02"
                ).unwrap();
                let ret = call_contract(
                    H160([
                        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, $ix,
                    ]),
                    &input_bytes,
                    3000,
                )
                .unwrap();
                assert_eq!(
                    hex::encode(ret.unwrap().output),
                    hex::encode(<sha2::$hasher as sha2::Digest>::digest(&input_bytes)),
                );
            }
        }
    }

    make_hasher_test!(test_sha512_256, 0x01, Sha512_256);
    make_hasher_test!(test_sha512, 0x02, Sha512);
    make_hasher_test!(test_sha384, 0x04, Sha384);
}
