use std::convert::TryFrom;

use evm::{
    executor::stack::{PrecompileHandle, PrecompileOutput},
    ExitSucceed,
};
use k256::elliptic_curve::scalar::IsHigh;
use ripemd::{Digest as _, Ripemd160};
use sha2::Sha256;
use sha3::Keccak256;

use crate::precompile::{read_input, record_linear_cost, PrecompileResult};

pub fn call_ecrecover(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(handle, handle.input().len() as u64, 3000, 0)?;

    // Make right padding for input.
    let input = handle.input();

    // Input encoded as [hash, r, s, v].
    let mut prehash = [0u8; 32];
    let mut padding = [0u8; 32];
    let mut sig = [0u8; 65];

    read_input(input, &mut prehash, 0);
    read_input(input, &mut padding, 32);
    read_input(input, &mut sig[..64], 64);

    // Check EIP-155
    if padding[31] > 26 {
        sig[64] = padding[31] - 27;
    } else {
        sig[64] = padding[31];
    }

    // Ensure input bytes 32..63 are all zero.
    if padding[..31] != [0; 31] {
        return Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: vec![],
        });
    }

    let recid = match k256::ecdsa::RecoveryId::from_byte(sig[64]) {
        Some(recid) if !recid.is_x_reduced() => recid,
        _ => {
            return Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: vec![],
            })
        }
    };

    let sig = match k256::ecdsa::Signature::try_from(&sig[..64]) {
        Ok(s) => s,
        Err(_) => {
            return Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: vec![],
            })
        }
    };

    // Reject high s to make consistent with our Ethereum transaction signature verification.
    if sig.s().is_high().into() {
        return Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: vec![],
        });
    }

    let output = match k256::ecdsa::VerifyingKey::recover_from_prehash(&prehash, &sig, recid) {
        Ok(recovered_key) => {
            // Convert Ethereum style address
            let p = recovered_key.to_encoded_point(false);
            let mut hasher = Keccak256::new();
            hasher.update(&p.as_bytes()[1..]);
            let mut address = hasher.finalize();
            address[0..12].copy_from_slice(&[0u8; 12]);
            address.to_vec()
        }
        Err(_) => vec![],
    };

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output,
    })
}

pub fn call_sha256(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(handle, handle.input().len() as u64, 60, 12)?;

    let mut hasher = Sha256::new();
    hasher.update(handle.input());
    let digest = hasher.finalize();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: digest.to_vec(),
    })
}

pub fn call_ripemd160(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(handle, handle.input().len() as u64, 600, 120)?;

    let mut hasher = Ripemd160::new();
    hasher.update(handle.input());
    let mut result = [0u8; 32];
    result[12..32].copy_from_slice(&hasher.finalize());

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: result.to_vec(),
    })
}

pub fn call_datacopy(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(handle, handle.input().len() as u64, 15, 3)?;

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: handle.input().to_vec(),
    })
}

#[cfg(test)]
mod test {
    extern crate test;

    use test::Bencher;

    use crate::precompile::testing::*;

    // The following test data is from "go-ethereum/core/vm/contracts_test.go"

    #[test]
    fn test_ecrecover() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "000000000000000000000000ceaccac640adf55b2028469bd36ba501f28b699d"
        );

        // Test with invalid input.
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e0000000000000deadbeef000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap(); // Should be successful, but empty result.
        assert!(ret.unwrap().output.is_empty());
    }

    #[test]
    fn test_sha256() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "811c7003375852fabd0d362e40e68607a12bdabae61a7d068fe5fdd1dbbf2a5d"
        );
    }

    #[test]
    fn test_ripemd160() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "0000000000000000000000009215b8d9882ff46f0dfde6684d78e831467f65e6"
        );
    }

    #[test]
    fn test_datacopy() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(hex::encode(ret.unwrap().output), "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02");
    }

    #[bench]
    fn bench_ecrecover(b: &mut Bencher) {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let input = hex::decode(input).unwrap();

        b.iter(|| {
            call_contract(
                H160([
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
                ]),
                &input,
                3000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }
}
