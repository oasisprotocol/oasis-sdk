use std::{
    cmp::{max, min, Ordering},
    collections::BTreeMap,
    convert::TryFrom,
    ops::BitAnd,
};

use evm::{
    executor::stack::{PrecompileFailure, PrecompileFn, PrecompileOutput},
    Context, ExitError, ExitSucceed,
};
use hmac::{Hmac, Mac, NewMac as _};
use k256::{ecdsa::recoverable, EncodedPoint};
use num::{BigUint, FromPrimitive, One, ToPrimitive, Zero};
use oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::{DeoxysII, KEY_SIZE, NONCE_SIZE};
use once_cell::sync::Lazy;
use primitive_types::H160;
use ripemd160::Ripemd160;
use sha2::Sha256;
use sha3::Keccak256;

// Some types matching evm::executor::stack.
type PrecompileResult = Result<PrecompileOutput, PrecompileFailure>;

/// Address of ECDSA public key recovery function.
const PRECOMPILE_ECRECOVER: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
]);
/// Address of of SHA2-256 hash function.
const PRECOMPILE_SHA256: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
]);
/// Address of RIPEMD-160 hash functions.
const PRECOMPILE_RIPEMD160: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
]);
/// Address of identity which defines the output as the input.
const PRECOMPILE_DATACOPY: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
]);
/// Big integer modular exponentiation in EIP-198.
const PRECOMPILE_BIGMODEXP: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
]);
/// Derive symmetric key from public/private key pair for X25519.
const PRECOMPILE_X25519_DERIVE: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
]);
/// Encrypt and authenticate plaintext and authenticate additional data with DeoxysII.
const PRECOMPILE_DEOXYSII_SEAL: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
]);
/// Decrypt and authenticate plaintext and authenticate additional data with DeoxysII.
const PRECOMPILE_DEOXYSII_OPEN: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
]);

/// The base setup cost for encryption and decryption.
const DEOXYSII_BASE_COST: u64 = 50_000;
/// The cost for encryption and decryption per word of input.
const DEOXYSII_WORD_COST: u64 = 100;
/// Length of an EVM word, in bytes.
const WORD: usize = 32;

/// Minimum gas cost of ModExp contract from eip-2565
/// https://eips.ethereum.org/EIPS/eip-2565
const MIN_GAS_COST: u64 = 200;

fn call_ecrecover(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    use sha3::Digest;

    let gas_cost = linear_cost(target_gas, input.len() as u64, 3000, 0)?;

    // Make right padding for input
    let mut padding = [0u8; 128];
    padding[..min(input.len(), 128)].copy_from_slice(&input[..min(input.len(), 128)]);

    let mut msg = [0u8; 32];
    let mut sig = [0u8; 65];

    // input encoded as [hash, v, r, s]
    msg.copy_from_slice(&padding[0..32]);
    sig[0..64].copy_from_slice(&padding[64..]);

    // Check EIP-155
    if padding[63] > 26 {
        sig[64] = padding[63] - 27;
    } else {
        sig[64] = padding[63];
    }

    // Ensure bytes 32..63 are all zero.
    if padding[32..63] != [0; 31] {
        return Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: vec![],
            logs: Default::default(),
        });
    }

    let dsa_sig = match recoverable::Signature::try_from(&sig[..]) {
        Ok(s) => s,
        Err(_) => {
            return Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                cost: gas_cost,
                output: vec![],
                logs: Default::default(),
            });
        }
    };

    // Reject high s to make consistent with our Ethereum transaction signature verification.
    if dsa_sig.s().is_high().into() {
        return Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: vec![],
            logs: Default::default(),
        });
    }

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
        Err(_) => vec![],
    };

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: result.to_vec(),
        logs: Default::default(),
    })
}

fn call_sha256(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    use sha2::Digest;

    let gas_cost = linear_cost(target_gas, input.len() as u64, 60, 12)?;

    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: digest.to_vec(),
        logs: Default::default(),
    })
}

fn call_ripemd160(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    use ripemd160::Digest;

    let gas_cost = linear_cost(target_gas, input.len() as u64, 600, 120)?;

    let mut hasher = Ripemd160::new();
    hasher.update(input);
    let mut result = [0u8; 32];
    result[12..32].copy_from_slice(&hasher.finalize());

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: result.to_vec(),
        logs: Default::default(),
    })
}

fn call_datacopy(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    let gas_cost = linear_cost(target_gas, input.len() as u64, 15, 3)?;

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: input.to_vec(),
        logs: Default::default(),
    })
}

fn call_bigmodexp(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    if input.len() < 96 {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("input must contain at least 96 bytes".into()),
        });
    };

    // reasonable assumption: this must fit within the Ethereum EVM's max stack size
    let max_size_big = BigUint::from_u32(1024).expect("can't create BigUint");

    let mut buf = [0; 32];
    buf.copy_from_slice(&input[0..32]);
    let base_len_big = BigUint::from_bytes_be(&buf);
    if base_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large base length".into()),
        });
    }

    buf.copy_from_slice(&input[32..64]);
    let exp_len_big = BigUint::from_bytes_be(&buf);
    if exp_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large exponent length".into()),
        });
    }

    buf.copy_from_slice(&input[64..96]);
    let mod_len_big = BigUint::from_bytes_be(&buf);
    if mod_len_big > max_size_big {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("unreasonably large exponent length".into()),
        });
    }

    // bounds check handled above
    let base_len = base_len_big.to_usize().expect("base_len out of bounds");
    let exp_len = exp_len_big.to_usize().expect("exp_len out of bounds");
    let mod_len = mod_len_big.to_usize().expect("mod_len out of bounds");

    // input length should be at least 96 + user-specified length of base + exp + mod
    let total_len = base_len + exp_len + mod_len + 96;
    if input.len() < total_len {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("insufficient input size".into()),
        });
    }

    // Gas formula allows arbitrary large exp_len when base and modulus are empty, so we need to handle empty base first.
    let (r, gas_cost) = if base_len == 0 && mod_len == 0 {
        (BigUint::zero(), MIN_GAS_COST)
    } else {
        // read the numbers themselves.
        let base_start = 96; // previous 3 32-byte fields
        let base = BigUint::from_bytes_be(&input[base_start..base_start + base_len]);

        let exp_start = base_start + base_len;
        let exponent = BigUint::from_bytes_be(&input[exp_start..exp_start + exp_len]);

        // do our gas accounting
        // TODO: we could technically avoid reading base first...
        let gas_cost =
            calculate_modexp_gas_cost(base_len as u64, exp_len as u64, mod_len as u64, &exponent)?;

        if let Some(target_gas) = target_gas {
            if target_gas < gas_cost {
                return Err(PrecompileFailure::Error {
                    exit_status: ExitError::OutOfGas,
                });
            }
        }

        let mod_start = exp_start + exp_len;
        let modulus = BigUint::from_bytes_be(&input[mod_start..mod_start + mod_len]);

        if modulus.is_zero() || modulus.is_one() {
            (BigUint::zero(), gas_cost)
        } else {
            (base.modpow(&exponent, &modulus), gas_cost)
        }
    };

    // write output to given memory, left padded and same length as the modulus.
    let bytes = r.to_bytes_be();

    // always true except in the case of zero-length modulus, which leads to
    // output of length and value 1.
    match bytes.len().cmp(&mod_len) {
        Ordering::Equal => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: bytes.to_vec(),
            logs: Default::default(),
        }),
        Ordering::Less => {
            let mut ret = Vec::with_capacity(mod_len);
            ret.extend(core::iter::repeat(0).take(mod_len - bytes.len()));
            ret.extend_from_slice(&bytes[..]);
            Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                cost: gas_cost,
                output: ret.to_vec(),
                logs: Default::default(),
            })
        }
        Ordering::Greater => Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("failed".into()),
        }),
    }
}

fn call_x25519_derive(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    let gas_cost = linear_cost(target_gas, input.len() as u64, 100_000, 0)?;

    // Input encoding: bytes32 public || bytes32 private.
    let mut public = [0u8; WORD];
    let mut private = [0u8; WORD];
    if input.len() != 2 * WORD {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("input length must be 64 bytes".into()),
        });
    }
    public.copy_from_slice(&input[0..WORD]);
    private.copy_from_slice(&input[WORD..]);

    let public = x25519_dalek::PublicKey::from(public);
    let private = x25519_dalek::StaticSecret::from(private);

    let mut kdf = Hmac::<sha2::Sha512Trunc256>::new_from_slice(b"MRAE_Box_Deoxys-II-256-128")
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("unable to create key derivation function".into()),
        })?;
    kdf.update(private.diffie_hellman(&public).as_bytes());

    let mut derived_key = [0u8; KEY_SIZE];
    let digest = kdf.finalize();
    derived_key.copy_from_slice(&digest.into_bytes()[..KEY_SIZE]);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: derived_key.to_vec(),
        logs: Default::default(),
    })
}

#[allow(clippy::type_complexity)]
fn decode_deoxysii_call_args(
    input: &[u8],
) -> Result<([u8; KEY_SIZE], [u8; NONCE_SIZE], &[u8], &[u8]), PrecompileFailure> {
    // Number of fixed words in the input (key, nonce word, two lengths; see
    // comments in the precompiles).
    const SLOTS: usize = 4;

    if input.len() < SLOTS * WORD {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("input length must be at least 128 bytes".into()),
        });
    }

    let mut words = input.array_chunks::<WORD>().take(SLOTS);
    let key = words.next().unwrap();
    let nonce_word = words.next().unwrap();
    let text_len = words.next().unwrap();
    let ad_len = words.next().unwrap();

    // Only the initial NONCE_SIZE bytes of the nonce field are used - bytes at
    // lower addresses in the input.
    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_word[..NONCE_SIZE]);

    let text_len_big = BigUint::from_bytes_be(text_len);
    let text_len = text_len_big
        .to_usize()
        .ok_or_else(|| PrecompileFailure::Error {
            exit_status: ExitError::Other("text length out of bounds".into()),
        })?;
    let text_size = text_len.saturating_add(31) & (!0x1f); // Round up to 32 bytes.

    let ad_len_big = BigUint::from_bytes_be(ad_len);
    let ad_len = ad_len_big
        .to_usize()
        .ok_or_else(|| PrecompileFailure::Error {
            exit_status: ExitError::Other("additional data length out of bounds".into()),
        })?;
    let ad_size = ad_len.saturating_add(31) & (!0x1f); // Round up to 32 bytes.
    if input.len() != SLOTS * WORD + ad_size + text_size {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("input too short".into()),
        });
    }

    let text = &input[(SLOTS * WORD)..(SLOTS * WORD + text_len)];
    let ad = &input[(SLOTS * WORD + text_size)..(SLOTS * WORD + text_size + ad_len)];

    Ok((*key, nonce, text, ad))
}

fn call_deoxysii_seal(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    let gas_cost = linear_cost(
        target_gas,
        input.len() as u64,
        DEOXYSII_BASE_COST,
        DEOXYSII_WORD_COST,
    )?;

    // Input encoding: bytes32 key || bytes32 nonce || uint plaintext_len || uint ad_len || plaintext || ad.
    let (key, nonce, text, ad) = decode_deoxysii_call_args(input)?;

    let deoxysii = DeoxysII::new(&key);
    let encrypted = deoxysii.seal(&nonce, text, ad);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: encrypted,
        logs: Default::default(),
    })
}

fn call_deoxysii_open(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
) -> PrecompileResult {
    let gas_cost = linear_cost(
        target_gas,
        input.len() as u64,
        DEOXYSII_BASE_COST,
        DEOXYSII_WORD_COST,
    )?;

    // Input encoding: bytes32 key || bytes32 nonce || uint ciphertext_len || uint ad_len || ciphertext || ad.
    let (key, nonce, ciphertext, ad) = decode_deoxysii_call_args(input)?;
    let ciphertext = ciphertext.to_vec();

    let deoxysii = DeoxysII::new(&key);
    match deoxysii.open(&nonce, ciphertext, ad) {
        Ok(decrypted) => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gas_cost,
            output: decrypted,
            logs: Default::default(),
        }),
        Err(_) => Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("decryption error".into()),
        }),
    }
}

/// A set of precompiles.
pub static PRECOMPILED_CONTRACT: Lazy<BTreeMap<H160, PrecompileFn>> = Lazy::new(|| {
    BTreeMap::from([
        (PRECOMPILE_ECRECOVER, call_ecrecover as PrecompileFn),
        (PRECOMPILE_SHA256, call_sha256),
        (PRECOMPILE_RIPEMD160, call_ripemd160),
        (PRECOMPILE_DATACOPY, call_datacopy),
        (PRECOMPILE_BIGMODEXP, call_bigmodexp),
        (PRECOMPILE_X25519_DERIVE, call_x25519_derive),
        (PRECOMPILE_DEOXYSII_SEAL, call_deoxysii_seal),
        (PRECOMPILE_DEOXYSII_OPEN, call_deoxysii_open),
    ])
});

/// Linear gas cost
fn linear_cost(
    target_gas: Option<u64>,
    len: u64,
    base: u64,
    word: u64,
) -> Result<u64, PrecompileFailure> {
    let cost = base
        .checked_add(word.checked_mul(len.saturating_add(31) / 32).ok_or(
            PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            },
        )?)
        .ok_or(PrecompileFailure::Error {
            exit_status: ExitError::OutOfGas,
        })?;

    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            });
        }
    }

    Ok(cost)
}

fn calculate_multiplication_complexity(
    base_length: u64,
    mod_length: u64,
) -> Result<u64, PrecompileFailure> {
    let max_length = max(base_length, mod_length);
    let mut words = max_length / 8;
    if max_length % 8 > 0 {
        words += 1;
    }

    // prevent overflow
    words.checked_mul(words).ok_or(PrecompileFailure::Error {
        exit_status: ExitError::OutOfGas,
    })
}

fn calculate_iteration_count(exp_length: u64, exponent: &BigUint) -> u64 {
    let mut iteration_count: u64 = 0;

    if exp_length <= 32 && exponent.is_zero() {
        iteration_count = 0;
    } else if exp_length <= 32 {
        iteration_count = exponent.bits() - 1;
    } else if exp_length > 32 {
        // construct BigUint to represent (2^256) - 1
        let bytes: [u8; 32] = [0xFF; 32];
        let max_256_bit_uint = BigUint::from_bytes_be(&bytes);

        iteration_count =
            (8 * (exp_length - 32)) + ((exponent.bitand(max_256_bit_uint)).bits() - 1);
    }

    max(iteration_count, 1)
}

/// Calculate ModExp gas cost according to EIP 2565:
/// https://eips.ethereum.org/EIPS/eip-2565
fn calculate_modexp_gas_cost(
    base_length: u64,
    exp_length: u64,
    mod_length: u64,
    exponent: &BigUint,
) -> Result<u64, PrecompileFailure> {
    let multiplication_complexity = calculate_multiplication_complexity(base_length, mod_length)?;
    let iteration_count = calculate_iteration_count(exp_length, exponent);
    let gas = max(
        MIN_GAS_COST,
        multiplication_complexity
            .checked_mul(iteration_count)
            .ok_or(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            })?
            / 3,
    );

    Ok(gas)
}

#[cfg(test)]
mod test {
    extern crate test;

    use rand::rngs::OsRng;
    use test::Bencher;

    use super::*;
    // The following test data is from "go-ethereum/core/vm/contracts_test.go"

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

    #[test]
    fn test_bigmodexp() {
        let input = "00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002003fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2efffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f";
        let ret = call_contract(
            H160([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &hex::decode(input).unwrap(),
            3000,
        )
        .unwrap();
        assert_eq!(
            hex::encode(ret.unwrap().output),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn test_bigmodexp_outofgas() {
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
    fn test_x25519_derive() {
        let mut rng = OsRng {};
        let static_secret = x25519_dalek::StaticSecret::new(&mut rng);
        let public = x25519_dalek::PublicKey::from(&static_secret);

        let mut blob = [0u8; 64];
        blob[..32].copy_from_slice(public.as_bytes());
        blob[32..].copy_from_slice(&static_secret.to_bytes());

        // Normal try.
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
            ]),
            &blob,
            1_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");

        // Not enough gas.
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
            ]),
            &blob,
            10_000,
        )
        .expect("call should return something")
        .expect_err("call should fail");

        // Test with known values.
        blob[..32].copy_from_slice(
            &<[u8; 32] as hex::FromHex>::from_hex(
                "3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576",
            )
            .unwrap(),
        );
        blob[32..].copy_from_slice(
            &<[u8; 32] as hex::FromHex>::from_hex(
                "c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750",
            )
            .unwrap(),
        );
        let output = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
            ]),
            &blob,
            1_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed")
        .output;
        assert_eq!(
            hex::encode(&output),
            "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586"
        );
    }

    #[bench]
    fn bench_x25519_derive(b: &mut Bencher) {
        let mut rng = OsRng {};
        let static_secret = x25519_dalek::StaticSecret::new(&mut rng);
        let public = x25519_dalek::PublicKey::from(&static_secret);

        let mut blob = [0u8; 64];
        blob[..32].copy_from_slice(public.as_bytes());
        blob[32..].copy_from_slice(&static_secret.to_bytes());

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
                ]),
                &blob,
                1_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    fn get_usize_bytes(u: usize) -> [u8; 32] {
        let short = u.to_be_bytes();
        let mut long = [0u8; 32];
        long[(32 - short.len())..].copy_from_slice(&short);
        long
    }

    #[test]
    fn test_deoxysii() {
        let mut key = [0u8; 32];
        key.copy_from_slice(b"this must be the excelentest key");
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(b"complete noncence, and too long.");
        let plaintext = b"plaintext";
        let plaintext_len = get_usize_bytes(plaintext.len());
        let ad = b"additional data";
        let ad_len = get_usize_bytes(ad.len());

        // Compose the input blob and try calling with partial fragments.
        let mut plain_input: Vec<u8> = Vec::new();
        plain_input.extend_from_slice(&key);
        plain_input.extend_from_slice(&nonce);
        plain_input.extend_from_slice(&plaintext_len);
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &plain_input,
            1_000_000,
        )
        .expect("call should return something")
        .expect_err("call should fail");

        plain_input.extend_from_slice(&ad_len);
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &plain_input,
            1_000_000,
        )
        .expect("call should return something")
        .expect_err("call should fail");

        plain_input.extend_from_slice(plaintext);
        plain_input.resize((plain_input.len() + 31) & (!31), 0);
        plain_input.extend_from_slice(ad);
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &plain_input,
            1_000_000,
        )
        .expect("call should return something")
        .expect_err("call should fail");

        plain_input.resize((plain_input.len() + 31) & (!31), 0);

        // Get ciphertext.
        let result = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &plain_input,
            1_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");
        let ciphertext = result.output;
        let ciphertext_len = get_usize_bytes(ciphertext.len());

        // Compose input blob for decryption.
        let mut cipher_input: Vec<u8> = Vec::new();
        cipher_input.extend_from_slice(&key);
        cipher_input.extend_from_slice(&nonce);
        cipher_input.extend_from_slice(&ciphertext_len);
        cipher_input.extend_from_slice(&ad_len);
        cipher_input.extend_from_slice(&ciphertext);
        cipher_input.resize((cipher_input.len() + 31) & (!31), 0);
        cipher_input.extend_from_slice(ad);
        cipher_input.resize((cipher_input.len() + 31) & (!31), 0);

        // Try decrypting and compare.
        let result = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
            ]),
            &cipher_input,
            1_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");
        assert_eq!(result.output, plaintext);
    }

    #[bench]
    fn bench_deoxysii_short(b: &mut Bencher) {
        let mut key = [0u8; 32];
        key.copy_from_slice(b"this must be the excelentest key");
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(b"complete noncence, and too long.");
        let plaintext = b"01234567890123456789";
        let plaintext_len = get_usize_bytes(plaintext.len());
        let ad = b"additional data";
        let ad_len = get_usize_bytes(ad.len());

        let mut plain_input: Vec<u8> = Vec::new();
        plain_input.extend_from_slice(&key);
        plain_input.extend_from_slice(&nonce);
        plain_input.extend_from_slice(&plaintext_len);
        plain_input.extend_from_slice(&ad_len);
        plain_input.extend_from_slice(plaintext);
        plain_input.resize((plain_input.len() + 31) & (!31), 0);
        plain_input.extend_from_slice(ad);
        plain_input.resize((plain_input.len() + 31) & (!31), 0);

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
                ]),
                &plain_input,
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[bench]
    fn bench_deoxysii_long(b: &mut Bencher) {
        let mut key = [0u8; 32];
        key.copy_from_slice(b"this must be the excelentest key");
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(b"complete noncence, and too long.");
        let plaintext = b"0123456789".repeat(200);
        let plaintext_len = get_usize_bytes(plaintext.len());
        let ad = b"additional data";
        let ad_len = get_usize_bytes(ad.len());

        let mut plain_input: Vec<u8> = Vec::new();
        plain_input.extend_from_slice(&key);
        plain_input.extend_from_slice(&nonce);
        plain_input.extend_from_slice(&plaintext_len);
        plain_input.extend_from_slice(&ad_len);
        plain_input.extend_from_slice(&plaintext);
        plain_input.resize((plain_input.len() + 31) & (!31), 0);
        plain_input.extend_from_slice(ad);
        plain_input.resize((plain_input.len() + 31) & (!31), 0);

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
                ]),
                &plain_input,
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }
}
