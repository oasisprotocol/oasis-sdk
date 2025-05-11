use std::cmp::{max, Ordering};

use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitSucceed,
};
use num::{BigUint, FromPrimitive, Integer, One, ToPrimitive, Zero};

use crate::precompile::{read_input, PrecompileResult};

/// Minimum gas cost of ModExp contract from eip-2565
/// https://eips.ethereum.org/EIPS/eip-2565
const MIN_GAS_COST: u64 = 200;

pub fn call_bigmodexp(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    let input = handle.input();
    let mut input_offset = 0;

    // Whenever the input is too short, the missing bytes are considered to be zero.
    let mut base_len_buf = [0u8; 32];
    read_input(input, &mut base_len_buf, input_offset);
    input_offset += 32;
    let mut exp_len_buf = [0u8; 32];
    read_input(input, &mut exp_len_buf, input_offset);
    input_offset += 32;
    let mut mod_len_buf = [0u8; 32];
    read_input(input, &mut mod_len_buf, input_offset);
    input_offset += 32;

    // Reasonable assumption: this must fit within the Ethereum EVM's max stack size.
    let max_size_big = BigUint::from_u32(1024).expect("can't create BigUint");

    let base_len_big = BigUint::from_bytes_be(&base_len_buf);
    if base_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large base length".into()),
        });
    }

    let exp_len_big = BigUint::from_bytes_be(&exp_len_buf);
    if exp_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large exponent length".into()),
        });
    }

    let mod_len_big = BigUint::from_bytes_be(&mod_len_buf);
    if mod_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large exponent length".into()),
        });
    }

    // Bounds check handled above.
    let base_len = base_len_big.to_usize().expect("base_len out of bounds");
    let exp_len = exp_len_big.to_usize().expect("exp_len out of bounds");
    let mod_len = mod_len_big.to_usize().expect("mod_len out of bounds");

    // Gas formula allows arbitrary large exp_len when base and modulus are empty, so we need to
    // handle empty base first.
    let r = if base_len == 0 && mod_len == 0 {
        handle.record_cost(MIN_GAS_COST)?;
        BigUint::zero()
    } else {
        // Read the numbers themselves.
        let mut base_buf = vec![0u8; base_len];
        read_input(input, &mut base_buf, input_offset);
        input_offset += base_len;
        let base = BigUint::from_bytes_be(&base_buf);

        let mut exp_buf = vec![0u8; exp_len];
        read_input(input, &mut exp_buf, input_offset);
        input_offset += exp_len;
        let exponent = BigUint::from_bytes_be(&exp_buf);

        let mut mod_buf = vec![0u8; mod_len];
        read_input(input, &mut mod_buf, input_offset); // No need to bump offset as we are done.
        let modulus = BigUint::from_bytes_be(&mod_buf);

        // Calculate gas cost.
        let gas_cost = calculate_gas_cost(
            base_len as u64,
            mod_len as u64,
            &exponent,
            &exp_buf,
            modulus.is_even(),
        );

        handle.record_cost(gas_cost)?;

        if modulus.is_zero() || modulus.is_one() {
            BigUint::zero()
        } else {
            base.modpow(&exponent, &modulus)
        }
    };

    // Write output to given memory, left padded and same length as the modulus.
    let bytes = r.to_bytes_be();

    // Always true except in the case of zero-length modulus, which leads to output of length and
    // value 1.
    match bytes.len().cmp(&mod_len) {
        Ordering::Equal => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: bytes.to_vec(),
        }),
        Ordering::Less => {
            let mut ret = Vec::with_capacity(mod_len);
            ret.extend(core::iter::repeat_n(0, mod_len - bytes.len()));
            ret.extend_from_slice(&bytes[..]);
            Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ret.to_vec(),
            })
        }
        Ordering::Greater => Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("failed".into()),
        }),
    }
}

fn calculate_multiplication_complexity(base_length: u64, mod_length: u64) -> u64 {
    let max_length = max(base_length, mod_length);
    let mut words = max_length / 8;
    if max_length % 8 > 0 {
        words += 1;
    }

    words.saturating_mul(words)
}

fn calculate_iteration_count(exponent: &BigUint, exponent_bytes: &[u8]) -> u64 {
    let mut iteration_count: u64 = 0;
    let exp_length = exponent_bytes.len() as u64;

    if exp_length <= 32 && exponent.is_zero() {
        iteration_count = 0;
    } else if exp_length <= 32 {
        iteration_count = exponent.bits() - 1;
    } else if exp_length > 32 {
        // Retrieve the first 32 bytes of exponent for the adjusted exponent length.
        let exponent_head = BigUint::from_bytes_be(&exponent_bytes[..32]);

        iteration_count = (8 * (exp_length - 32)) + exponent_head.bits() - 1;
    }

    max(iteration_count, 1)
}

/// Calculate ModExp gas cost according to [EIP-2565](https://eips.ethereum.org/EIPS/eip-2565).
fn calculate_gas_cost(
    base_length: u64,
    mod_length: u64,
    exponent: &BigUint,
    exponent_bytes: &[u8],
    mod_is_even: bool,
) -> u64 {
    let multiplication_complexity = calculate_multiplication_complexity(base_length, mod_length);
    let iteration_count = calculate_iteration_count(exponent, exponent_bytes);
    let gas = max(
        MIN_GAS_COST,
        multiplication_complexity.saturating_mul(iteration_count) / 3,
    );
    // The modpow operation is more expensive for even numbers.
    // See https://github.com/rust-num/num-bigint/blob/65f62a8b1484448bfb9789ef4123b50556254905/src/biguint/power.rs#L140-L146.
    gas.saturating_mul(if mod_is_even { 20 } else { 1 })
}

#[cfg(test)]
mod test {
    use crate::precompile::testing::*;

    use super::*;

    // The following test data is from "go-ethereum/core/vm/contracts_test.go"

    #[test]
    fn test_modexp() {
        let input = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            03fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc\
            2efffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        )
        .unwrap();

        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &input,
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn test_out_of_gas() {
        let input = "000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000200db34d0e438249c0ed685c949cc28776a05094e1c48691dc3f2dca5fc3356d2a0663bd376e4712839917eb9a19c670407e2c377a2de385a3ff3b52104f7f1f4e0c7bf7717fb913896693dc5edbb65b760ef1b00e42e9d8f9af17352385e1cd742c9b006c0f669995cb0bb21d28c0aced2892267637b6470d8cee0ab27fc5d42658f6e88240c31d6774aa60a7ebd25cd48b56d0da11209f1928e61005c6eb709f3e8e0aaf8d9b10f7d7e296d772264dc76897ccdddadc91efa91c1903b7232a9e4c3b941917b99a3bc0c26497dedc897c25750af60237aa67934a26a2bc491db3dcc677491944bc1f51d3e5d76b8d846a62db03dedd61ff508f91a56d71028125035c3a44cbb041497c83bf3e4ae2a9613a401cc721c547a2afa3b16a2969933d3626ed6d8a7428648f74122fd3f2a02a20758f7f693892c8fd798b39abac01d18506c45e71432639e9f9505719ee822f62ccbf47f6850f096ff77b5afaf4be7d772025791717dbe5abf9b3f40cff7d7aab6f67e38f62faf510747276e20a42127e7500c444f9ed92baf65ade9e836845e39c4316d9dce5f8e2c8083e2c0acbb95296e05e51aab13b6b8f53f06c9c4276e12b0671133218cc3ea907da3bd9a367096d9202128d14846cc2e20d56fc8473ecb07cecbfb8086919f3971926e7045b853d85a69d026195c70f9f7a823536e2a8f4b3e12e94d9b53a934353451094b81010001df3143a0057457d75e8c708b6337a6f5a4fd1a06727acf9fb93e2993c62f3378b37d56c85e7b1e00f0145ebf8e4095bd723166293c60b6ac1252291ef65823c9e040ddad14969b3b340a4ef714db093a587c37766d68b8d6b5016e741587e7e6bf7e763b44f0247e64bae30f994d248bfd20541a333e5b225ef6a61199e301738b1e688f70ec1d7fb892c183c95dc543c3e12adf8a5e8b9ca9d04f9445cced3ab256f29e998e69efaa633a7b60e1db5a867924ccab0a171d9d6e1098dfa15acde9553de599eaa56490c8f411e4985111f3d40bddfc5e301edb01547b01a886550a61158f7e2033c59707789bf7c854181d0c2e2a42a93cf09209747d7082e147eb8544de25c3eb14f2e35559ea0c0f5877f2f3fc92132c0ae9da4e45b2f6c866a224ea6d1f28c05320e287750fbc647368d41116e528014cc1852e5531d53e4af938374daba6cee4baa821ed07117253bb3601ddd00d59a3d7fb2ef1f5a2fbba7c429f0cf9a5b3462410fd833a69118f8be9c559b1000cc608fd877fb43f8e65c2d1302622b944462579056874b387208d90623fcdaf93920ca7a9e4ba64ea208758222ad868501cc2c345e2d3a5ea2a17e5069248138c8a79c0251185d29ee73e5afab5354769142d2bf0cb6712727aa6bf84a6245fcdae66e4938d84d1b9dd09a884818622080ff5f98942fb20acd7e0c916c2d5ea7ce6f7e173315384518f";
        match call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap()
        {
            Ok(_) => {
                panic!("Test not expected to pass");
            }
            Err(e) => {
                assert_eq!(
                    e,
                    PrecompileFailure::Error {
                        exit_status: ExitError::OutOfGas
                    }
                );
            }
        }
    }

    #[test]
    fn test_zero_exp_with_33_length() {
        let input = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ];

        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &input,
            100_000,
        )
        .unwrap()
        .expect("precompile call should succeed");

        assert_eq!(ret.output.len(), 1);

        let result = BigUint::from_bytes_be(&ret.output[..]);
        let expected = BigUint::parse_bytes(b"0", 10).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_long_exp_gas_cost_matches_specs() {
        let input = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 96, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            16, 0, 0, 0, 255, 255, 255, 2, 0, 0, 179, 0, 0, 2, 0, 0, 122, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 255, 251, 0, 0, 0, 0, 4, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 96, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 255, 255, 255, 2, 0, 0, 179, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255,
            255, 255, 255, 249,
        ];

        let (_, gas_used) = call_contract_with_gas_report(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &input,
            1_000_000,
        )
        .expect("precompile call should succeed");

        // Compare against reference value from geth.
        assert_eq!(gas_used, 7104 * 20, "used gas should match reference value");
    }

    #[test]
    fn test_vectors() {
        let address = H160([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
        ]);

        for case in read_test_cases("modexp_eip2565").iter() {
            let (ret, gas_used) = call_contract_with_gas_report(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                case.gas,
            )
            .unwrap();

            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
            assert_eq!(gas_used, case.gas);
        }
    }
}
