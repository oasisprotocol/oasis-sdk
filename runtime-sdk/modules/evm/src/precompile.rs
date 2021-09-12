use core::cmp::min;
use core::convert::TryFrom;

use evm::{executor::PrecompileOutput, ExitSucceed, ExitError, Context};
use k256::{
    ecdsa::recoverable,
    EncodedPoint
};
use sha3::Keccak256;
use sha2::Sha256;
use ripemd160::Ripemd160;
use primitive_types::{H160};

const PRECOMPILE_ECRECOVER: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
const PRECOMPILE_SHA256: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
const PRECOMPILE_RIPEMD160: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
const PRECOMPILE_DATACOPY: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);

fn call_ecrecover(
    input: &[u8],
    target_gas: u64
) -> Result<PrecompileOutput, ExitError> {
    use sha3::Digest;

    let gas_cost = linear_cost(Some(target_gas), input.len() as u64, 3000, 0)?;

    // Make right padding for input
    let mut padding = [0u8; 128];
    padding[..min(input.len(), 128)].copy_from_slice(&input[..min(input.len(), 128)]);

    let mut msg = [0u8; 32];
    let mut sig = [0u8; 65];

    // input encoded as [hash, v, r, s)
    msg[0..32].copy_from_slice(&input[0..32]);
    sig[0..32].copy_from_slice(&input[64..96]);
    sig[32..64].copy_from_slice(&input[96..128]);

    // Check EIP-155
    if input[63] > 26 {
        sig[64] = input[63] - 27;
    } else {
        sig[64] = input[63];
    }

    let dsa_sig = recoverable::Signature::try_from(&sig[..]).unwrap();
    let result = match dsa_sig.recover_verify_key_from_digest_bytes(&msg.into()) {
        Ok(recovered_key) => {
            // Convert Ethereum style address
            let p = EncodedPoint::from(&recovered_key).decompress().unwrap();
            let mut hasher = Keccak256::new();
            hasher.update(&p.as_bytes()[1..]);
            let mut address = hasher.finalize();
            address[0..12].copy_from_slice(&[0u8; 12]);
            address.to_vec()
        }
        Err(_) => [0u8; 0].to_vec(),
    };

    Ok(
        PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: result.to_vec(),
            logs: Default::default(),
        }
    )
}

fn call_sha256(
    input: &[u8],
    target_gas: u64
) -> Result<PrecompileOutput, ExitError> {
    use sha2::Digest;

    let gas_cost = linear_cost(Some(target_gas), input.len() as u64, 60, 12)?;

    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();

    Ok(
        PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: digest.to_vec(),
            logs: Default::default(),
        }
    )
}

fn call_ripemd160(
    input: &[u8],
    target_gas: u64
) -> Result<PrecompileOutput, ExitError> {
    use ripemd160::Digest;

    let gas_cost = linear_cost(Some(target_gas), input.len() as u64, 600, 120)?;

    let mut hasher = Ripemd160::new();
    hasher.update(input);
    let mut result = [0u8; 32];
    result[12..32].copy_from_slice(&hasher.finalize());

    Ok(
        PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: result.to_vec(),
            logs: Default::default(),
        }
    )
}

fn call_datacopy(
    input: &[u8],
    target_gas: u64
) -> Result<PrecompileOutput, ExitError> {
    let gas_cost = linear_cost(Some(target_gas), input.len() as u64, 15, 3)?;

    Ok(
        PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: input.to_vec(),
            logs: Default::default(),
        }
    )
}

pub fn precompiled_contract(
    _address: H160,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context
) -> Option<Result<PrecompileOutput, ExitError>> {

    if _address == PRECOMPILE_ECRECOVER {
        return Some(call_ecrecover(_input, _target_gas.unwrap()));
    }
    if _address == PRECOMPILE_SHA256 {
        return Some(call_sha256(_input, _target_gas.unwrap()));
    }
    if _address == PRECOMPILE_RIPEMD160 {
        return Some(call_ripemd160(_input, _target_gas.unwrap()));
    }
    if _address == PRECOMPILE_DATACOPY {
        return Some(call_datacopy(_input, _target_gas.unwrap()));
    }
    None
}

/// Linear gas cost
fn linear_cost(
    target_gas: Option<u64>,
    len: u64,
    base: u64,
    word: u64,
) -> Result<u64, ExitError> {
    let cost = base
        .checked_add(
            word.checked_mul(len.saturating_add(31) / 32)
                .ok_or(ExitError::OutOfGas)?,
        )
        .ok_or(ExitError::OutOfGas)?;

    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(ExitError::OutOfGas);
        }
    }

    Ok(cost)
}

#[cfg(test)]
mod test {
    use super::*;

   // The following test data is from "go-ethereum/core/vm/contracts_test.go"
    #[test]
    fn test_ecrecover() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_ecrecover(&hex::decode(input).unwrap(), 3000);
        assert_eq!(hex::encode(ret.unwrap().output), "000000000000000000000000ceaccac640adf55b2028469bd36ba501f28b699d");
    }

    #[test]
    fn test_sha256() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_sha256(&hex::decode(input).unwrap(), 3000);
        assert_eq!(hex::encode(ret.unwrap().output), "811c7003375852fabd0d362e40e68607a12bdabae61a7d068fe5fdd1dbbf2a5d");
    }

    #[test]
    fn test_ripemd160() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_ripemd160(&hex::decode(input).unwrap(), 3000);
        assert_eq!(hex::encode(ret.unwrap().output), "0000000000000000000000009215b8d9882ff46f0dfde6684d78e831467f65e6");
    }

    #[test]
    fn test_datacopy() {
        let input = "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02";
        let ret = call_datacopy(&hex::decode(input).unwrap(), 3000);
        assert_eq!(hex::encode(ret.unwrap().output), "38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02");
    }
}
