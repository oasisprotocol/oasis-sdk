//! Implements the confidential precompiles.
use std::{collections::HashMap, convert::TryInto};

use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitRevert, ExitSucceed,
};
use hmac::{Hmac, Mac};
use once_cell::sync::Lazy;

use oasis_runtime_sdk::{
    core::common::crypto::mrae::deoxysii::{DeoxysII, KEY_SIZE, NONCE_SIZE},
    crypto::signature::{self, SignatureType},
};

use crate::backend::{EVMBackendExt, RNG_MAX_BYTES};

use super::{record_linear_cost, record_multilinear_cost, PrecompileResult};

/// Length of an EVM word, in bytes.
pub const WORD: usize = 32;

/// The base cost for x25519 key derivation.
const X25519_KEY_DERIVATION_BASE_COST: u64 = 1_100;

/// The cost for converting a Curve25519 secret key to public key.
/// It's one scalar multiplication, so it shouldn't be too expensive.
const CURVE25519_COMPUTE_PUBLIC_COST: u64 = 1_000;

/// The base setup cost for encryption and decryption.
const DEOXYSII_BASE_COST: u64 = 100;
/// The cost for encryption and decryption per word of input.
const DEOXYSII_WORD_COST: u64 = 10;

/// The cost of a key pair generation operation, per method.
static KEYPAIR_GENERATE_BASE_COST: Lazy<HashMap<SignatureType, u64>> = Lazy::new(|| {
    HashMap::from([
        (SignatureType::Ed25519_Oasis, 1_000),
        (SignatureType::Ed25519_Pure, 1_000),
        (SignatureType::Ed25519_PrehashedSha512, 1_000),
        (SignatureType::Secp256k1_Oasis, 1_500),
        (SignatureType::Secp256k1_PrehashedKeccak256, 1_500),
        (SignatureType::Secp256k1_PrehashedSha256, 1_500),
        (SignatureType::Secp256r1_PrehashedSha256, 4_000),
        (SignatureType::Secp384r1_PrehashedSha384, 18_000),
        (SignatureType::Sr25519_Pure, 1_000),
    ])
});

/// The costs of a message signing operation.
static SIGN_MESSAGE_COST: Lazy<HashMap<SignatureType, (u64, u64)>> = Lazy::new(|| {
    HashMap::from([
        (SignatureType::Ed25519_Oasis, (1_500, 8)),
        (SignatureType::Ed25519_Pure, (1_500, 8)),
        (SignatureType::Ed25519_PrehashedSha512, (1_500, 0)),
        (SignatureType::Secp256k1_Oasis, (3_000, 8)),
        (SignatureType::Secp256k1_PrehashedKeccak256, (3_000, 0)),
        (SignatureType::Secp256k1_PrehashedSha256, (3_000, 0)),
        (SignatureType::Secp256r1_PrehashedSha256, (9_000, 0)),
        (SignatureType::Secp384r1_PrehashedSha384, (43_200, 0)),
        (SignatureType::Sr25519_Pure, (1_500, 8)),
    ])
});

/// The costs of a signature verification operation.
static VERIFY_MESSAGE_COST: Lazy<HashMap<SignatureType, (u64, u64)>> = Lazy::new(|| {
    HashMap::from([
        (SignatureType::Ed25519_Oasis, (2_000, 8)),
        (SignatureType::Ed25519_Pure, (2_000, 8)),
        (SignatureType::Ed25519_PrehashedSha512, (2_000, 0)),
        (SignatureType::Secp256k1_Oasis, (3_000, 8)),
        (SignatureType::Secp256k1_PrehashedKeccak256, (3_000, 0)),
        (SignatureType::Secp256k1_PrehashedSha256, (3_000, 0)),
        (SignatureType::Secp256r1_PrehashedSha256, (7_900, 0)),
        (SignatureType::Secp384r1_PrehashedSha384, (37_920, 0)),
        (SignatureType::Sr25519_Pure, (2_000, 8)),
    ])
});

pub(super) fn call_random_bytes<B: EVMBackendExt>(
    handle: &mut impl PrecompileHandle,
    backend: &B,
) -> PrecompileResult {
    let (num_bytes_big, pers_str): (solabi::U256, solabi::Bytes<Vec<u8>>) =
        solabi::decode(handle.input()).map_err(|e| PrecompileFailure::Error {
            exit_status: ExitError::Other(e.to_string().into()),
        })?;

    let pers_str = pers_str.as_bytes();
    let num_bytes: u64 = num_bytes_big
        .try_into()
        .unwrap_or(u64::MAX)
        .min(RNG_MAX_BYTES);

    // This operation shouldn't be too cheap to start since it invokes a key manager.
    // Each byte is generated using hashing, so it's neither expensive nor cheap.
    // Thus:
    // * The base gas is 2x the SSTORE gas since storing requires as much effort
    //   as accessing the key manager (which storing does as well).
    // * The word gas is 4x SHA256 gas since the CSPRNG is reasonably expected
    //   to use an efficient cryptographic hash function with some bookkeeping.
    // In any case, it's much cheaper than using a VRF oracle, and even a Solidity DRBG,
    // which has a cost-per-byte upwards of 1000.
    record_multilinear_cost(handle, num_bytes, pers_str.len() as u64, 240, 60, 10_000)?;
    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: backend.random_bytes(num_bytes, pers_str),
    })
}

pub(super) fn call_curve25519_compute_public(
    handle: &mut impl PrecompileHandle,
) -> PrecompileResult {
    handle.record_cost(CURVE25519_COMPUTE_PUBLIC_COST)?;
    let input = handle.input(); // Input encoding: bytes32 private.
    if input.len() != 32 {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("input length must be 32 bytes".into()),
        });
    }
    let private = <&[u8; WORD]>::try_from(input).unwrap();
    let secret = x25519_dalek::StaticSecret::from(*private);
    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: x25519_dalek::PublicKey::from(&secret).as_bytes().to_vec(),
    })
}

pub(super) fn call_x25519_derive(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(
        handle,
        handle.input().len() as u64,
        X25519_KEY_DERIVATION_BASE_COST,
        0,
    )?;

    // Input encoding: bytes32 public || bytes32 private.
    let input = handle.input();
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

    let mut kdf =
        Hmac::<sha2::Sha512_256>::new_from_slice(b"MRAE_Box_Deoxys-II-256-128").map_err(|_| {
            PrecompileFailure::Error {
                exit_status: ExitError::Other("unable to create key derivation function".into()),
            }
        })?;
    kdf.update(private.diffie_hellman(&public).as_bytes());

    let mut derived_key = [0u8; KEY_SIZE];
    let digest = kdf.finalize();
    derived_key.copy_from_slice(&digest.into_bytes()[..KEY_SIZE]);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: derived_key.to_vec(),
    })
}

#[allow(clippy::type_complexity)]
fn decode_deoxysii_call_args(
    input: &[u8],
) -> Result<([u8; KEY_SIZE], [u8; NONCE_SIZE], Vec<u8>, Vec<u8>), PrecompileFailure> {
    let (key, nonce, text, ad): (
        solabi::Bytes<[u8; 32]>, // key
        solabi::Bytes<[u8; 32]>, // nonce
        solabi::Bytes<Vec<u8>>,  // plain or ciphertext
        solabi::Bytes<Vec<u8>>,  // associated data
    ) = solabi::decode(input).map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;

    let key_bytes = key.as_bytes();
    let nonce_bytes = nonce.as_bytes();
    let text = text.to_vec();
    let ad = ad.to_vec();

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_bytes[..NONCE_SIZE]);
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(key_bytes);

    Ok((key, nonce, text, ad))
}

pub(super) fn call_deoxysii_seal(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(
        handle,
        handle.input().len() as u64,
        DEOXYSII_BASE_COST,
        DEOXYSII_WORD_COST,
    )?;

    let (key, nonce, text, ad) = decode_deoxysii_call_args(handle.input())?;
    let deoxysii = DeoxysII::new(&key);
    let encrypted = deoxysii.seal(&nonce, text, ad);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: encrypted,
    })
}

pub(super) fn call_deoxysii_open(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    record_linear_cost(
        handle,
        handle.input().len() as u64,
        DEOXYSII_BASE_COST,
        DEOXYSII_WORD_COST,
    )?;

    let (key, nonce, ciphertext, ad) = decode_deoxysii_call_args(handle.input())?;
    let ciphertext = ciphertext.to_vec();
    let deoxysii = DeoxysII::new(&key);

    match deoxysii.open(&nonce, ciphertext, ad) {
        Ok(decrypted) => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: decrypted,
        }),
        Err(_) => Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: vec![],
        }),
    }
}

pub(super) fn call_keypair_generate(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    let (method, seed): (
        solabi::U256,           // method
        solabi::Bytes<Vec<u8>>, // seed
    ) = solabi::decode(handle.input()).map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;

    let method: usize = method.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("method identifier out of bounds".into()),
    })?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("method identifier out of bounds".into()),
        })?
        .try_into()
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("unknown signature type".into()),
        })?;

    record_linear_cost(
        handle,
        handle.input().len() as u64,
        *KEYPAIR_GENERATE_BASE_COST
            .get(&sig_type)
            .ok_or(PrecompileFailure::Error {
                exit_status: ExitError::Other("unknown signature type".into()),
            })?,
        0,
    )?;

    let signer =
        signature::MemorySigner::new_from_seed(sig_type, seed.as_bytes()).map_err(|err| {
            PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("error creating signer: {err}").into()),
            }
        })?;
    let public = signer.public_key().as_bytes().to_vec();
    let private = signer.to_bytes();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: solabi::encode(&(solabi::Bytes(public), solabi::Bytes(private))),
    })
}

pub(super) fn call_sign(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    #[allow(clippy::type_complexity)]
    let (typ, pk, ctx, msg): (
        solabi::U256,           // signature type
        solabi::Bytes<Vec<u8>>, // private key
        solabi::Bytes<Vec<u8>>, // context or precomputed hash bytes
        solabi::Bytes<Vec<u8>>, // message; should be zero-length if precomputed hash given
    ) = solabi::decode(handle.input()).map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;

    let message = msg.as_bytes();
    let ctx_or_hash = ctx.as_bytes();
    let pk = pk.as_bytes();
    let method: usize = typ.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("signature type identifier out of bounds".into()),
    })?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("signature type identifier out of bounds".into()),
        })?
        .try_into()
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("unknown signature type".into()),
        })?;

    let costs = *SIGN_MESSAGE_COST
        .get(&sig_type)
        .ok_or(PrecompileFailure::Error {
            exit_status: ExitError::Other("unknown signature type".into()),
        })?;
    record_linear_cost(handle, handle.input().len() as u64, costs.0, costs.1)?;

    let signer = signature::MemorySigner::from_bytes(sig_type, pk).map_err(|e| {
        PrecompileFailure::Error {
            exit_status: ExitError::Other(format!("error creating signer: {e}").into()),
        }
    })?;

    let result = signer.sign_by_type(sig_type, ctx_or_hash, message);
    let result = result.map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(format!("error signing message: {e}").into()),
    })?;

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: result.into(),
    })
}

pub(super) fn call_verify(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    #[allow(clippy::type_complexity)]
    let (typ, pk, ctx, msg, sig): (
        solabi::U256,           // signature type
        solabi::Bytes<Vec<u8>>, // public key
        solabi::Bytes<Vec<u8>>, // context or precomputed hash bytes
        solabi::Bytes<Vec<u8>>, // message; should be zero-length if precomputed hash given
        solabi::Bytes<Vec<u8>>, // signature
    ) = solabi::decode(handle.input()).map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;

    let signature = sig.to_vec();
    let message = msg.as_bytes();
    let ctx_or_hash = ctx.as_bytes();
    let pk = pk.as_bytes();
    let method: usize = typ.try_into().map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("signature type identifier out of bounds".into()),
    })?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("signature type identifier out of bounds".into()),
        })?
        .try_into()
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("unknown signature type".into()),
        })?;

    let costs = *VERIFY_MESSAGE_COST
        .get(&sig_type)
        .ok_or(PrecompileFailure::Error {
            exit_status: ExitError::Other("unknown signature type".into()),
        })?;
    record_linear_cost(handle, handle.input().len() as u64, costs.0, costs.1)?;

    let signature: signature::Signature = signature.into();
    let public_key =
        signature::PublicKey::from_bytes(sig_type, pk).map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("error reading public key".into()),
        })?;

    let result = public_key.verify_by_type(sig_type, ctx_or_hash, message, &signature);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: solabi::encode(&(result.is_ok(),)),
    })
}

#[cfg(test)]
mod test {
    extern crate test;

    use rand::rngs::OsRng;
    use test::Bencher;

    use oasis_runtime_sdk::crypto::signature::{self, SignatureType};

    use crate::precompile::{testing::*, PrecompileResult};

    #[test]
    fn test_x25519_derive() {
        let mut rng = OsRng {};
        let static_secret = x25519_dalek::StaticSecret::random_from_rng(rng);
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
            1_000,
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
        let static_secret = x25519_dalek::StaticSecret::random_from_rng(rng);
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

    #[bench]
    fn bench_curve25519_compute_public(b: &mut Bencher) {
        let mut rng = OsRng {};
        let static_secret = x25519_dalek::StaticSecret::random_from_rng(rng);

        let mut blob = [0u8; 32];
        blob[..32].copy_from_slice(&static_secret.to_bytes());

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08,
                ]),
                &blob,
                1_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[test]
    fn test_deoxysii() {
        let key = b"this must be the excelentest key";
        let nonce = b"complete noncence, and too long.";
        let plaintext = b"0123456789";
        let ad = b"additional data";
        let ret_ct = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
            ]),
            &solabi::encode(&(
                solabi::Bytes(key.to_vec()),
                solabi::Bytes(nonce.to_vec()),
                solabi::Bytes(plaintext.to_vec()),
                solabi::Bytes(ad.to_vec()),
            )),
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");
        assert_ne!(plaintext.as_slice(), ret_ct.output);

        let ret_pt = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
            ]),
            &solabi::encode(&(
                solabi::Bytes(key.to_vec()),
                solabi::Bytes(nonce.to_vec()),
                solabi::Bytes(ret_ct.output.to_vec()),
                solabi::Bytes(ad.to_vec()),
            )),
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");
        assert_eq!(plaintext.as_slice(), ret_pt.output);
    }

    #[test]
    fn test_random_bytes() {
        let ret = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ]),
            &solabi::encode(&(solabi::U256::new(4_u128), solabi::Bytes(vec![0xbe, 0xef]))),
            10_560,
        )
        .unwrap();
        assert_eq!(hex::encode(ret.unwrap().output), "beef0203");
    }

    #[bench]
    fn bench_deoxysii_short(b: &mut Bencher) {
        let key = b"this must be the excelentest key";
        let nonce = b"complete noncence, and too long.";
        let plaintext = b"01234567890123456789";
        let ad = b"additional data";
        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
                ]),
                &solabi::encode(&(
                    solabi::Bytes(key.to_vec()),
                    solabi::Bytes(nonce.to_vec()),
                    solabi::Bytes(plaintext.to_vec()),
                    solabi::Bytes(ad.to_vec()),
                )),
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[bench]
    fn bench_deoxysii_long(b: &mut Bencher) {
        let key = b"this must be the excelentest key";
        let nonce = b"complete noncence, and too long.";
        let plaintext = b"0123456789".repeat(200);
        let ad = b"additional data";
        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
                ]),
                &solabi::encode(&(
                    solabi::Bytes(key.to_vec()),
                    solabi::Bytes(nonce.to_vec()),
                    solabi::Bytes(plaintext.to_vec()),
                    solabi::Bytes(ad.to_vec()),
                )),
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[test]
    fn test_curve25519_compute_public() {
        let params =
            hex::decode(b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20")
                .unwrap();
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08,
            ]),
            &params,
            10_000_000,
        )
        .expect("call should return something")
        .expect_err("call should fail as it has an extra byte of input");

        let params =
            hex::decode(b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .unwrap();
        let output = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08,
            ]),
            &params,
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed")
        .output;

        assert_eq!(
            hex::encode(output),
            "8f40c5adb68f25624ae5b214ea767a6ec94d829d3d7b5e1ad1ba6f3e2138285f"
        );
    }

    #[test]
    fn test_keypair_generate() {
        // Invalid method.
        let params = solabi::encode(&(
            solabi::U256::new(50_u128),
            solabi::Bytes(b"01234567890123456789012345678901".to_vec()),
        ));
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &params,
            10_000_000,
        )
        .expect("call should return something")
        .expect_err("call should fail");

        // Working test.
        let params = solabi::encode(&(
            solabi::U256::new(SignatureType::Ed25519_Oasis.as_int().into()),
            solabi::Bytes(b"01234567890123456789012345678901".to_vec()),
        ));
        let output1 = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &params,
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed")
        .output;

        // Again, should be repeatable.
        let params = solabi::encode(&(
            solabi::U256::new(SignatureType::Ed25519_Oasis.as_int().into()),
            solabi::Bytes(b"01234567890123456789012345678901".to_vec()),
        ));
        let output2 = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
            ]),
            &params,
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed")
        .output;

        assert_eq!(output1, output2);
    }

    fn bench_keypair_generate(b: &mut Bencher, signature_type: SignatureType) {
        let seed = b"01234567".repeat(if signature_type.is_secp384r1_variant() {
            6
        } else {
            4
        });
        let params = solabi::encode(&(
            solabi::U256::new(signature_type.as_int().into()),
            solabi::Bytes(seed),
        ));
        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
                ]),
                &params,
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed")
        });
    }

    #[bench]
    fn bench_keypair_generate_ed25519(b: &mut Bencher) {
        bench_keypair_generate(b, SignatureType::Ed25519_Oasis);
    }

    #[bench]
    fn bench_keypair_generate_secp256k1(b: &mut Bencher) {
        bench_keypair_generate(b, SignatureType::Secp256k1_Oasis);
    }

    #[bench]
    fn bench_keypair_generate_secp256r1(b: &mut Bencher) {
        bench_keypair_generate(b, SignatureType::Secp256r1_PrehashedSha256);
    }

    #[bench]
    fn bench_keypair_generate_secp384r1(b: &mut Bencher) {
        bench_keypair_generate(b, SignatureType::Secp384r1_PrehashedSha384);
    }

    #[bench]
    fn bench_keypair_generate_sr25519(b: &mut Bencher) {
        bench_keypair_generate(b, SignatureType::Sr25519_Pure);
    }

    #[test]
    fn test_basic_roundtrip() {
        let seed = b"01234567890123456789012345678901";
        let context = b"test context";
        let message = b"test message";

        for method in 0u8..=6u8 {
            let sig_type: SignatureType = method.try_into().unwrap();
            if sig_type.is_prehashed() {
                // Tested in test_basic_roundtrip_prehashed below.
                continue;
            }

            // Generate key pair from a fixed seed.
            let params = solabi::encode(&(
                solabi::U256::new(method.into()),
                solabi::Bytes(seed.to_vec()),
            ));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;

            let (public_key, private_key): (solabi::Bytes<Vec<u8>>, solabi::Bytes<Vec<u8>>) =
                solabi::decode(&output).expect("decode should succeed");
            let public_key = public_key.as_bytes();
            let private_key = private_key.as_bytes();

            // Sign message.
            let params = solabi::encode(&(
                solabi::U256::new(method.into()),
                solabi::Bytes(private_key),
                solabi::Bytes(context.to_vec()),
                solabi::Bytes(message.to_vec()),
            ));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;

            let signature = output.to_vec();

            // Verify signature.
            let params = solabi::encode(&(
                solabi::U256::new(method.into()),
                solabi::Bytes(public_key),
                solabi::Bytes(context.to_vec()),
                solabi::Bytes(message.to_vec()),
                solabi::Bytes(signature),
            ));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;
            let status: bool = solabi::decode(&output).expect("decode should succeed");
            assert!(status);
        }
    }

    #[test]
    fn test_basic_roundtrip_prehashed() {
        use sha2::Digest as _;

        let message = b"test message";

        let sig_types: &[(SignatureType, Box<dyn Fn(&[u8]) -> Vec<u8>>)] = &[
            (
                SignatureType::Ed25519_PrehashedSha512,
                Box::new(|message| sha2::Sha512::digest(message).to_vec()),
            ),
            (
                SignatureType::Secp256k1_PrehashedKeccak256,
                Box::new(|message| sha3::Keccak256::digest(message).to_vec()),
            ),
            (
                SignatureType::Secp256k1_PrehashedSha256,
                Box::new(|message| sha2::Sha256::digest(message).to_vec()),
            ),
            (
                SignatureType::Secp256r1_PrehashedSha256,
                Box::new(|message| sha2::Sha256::digest(message).to_vec()),
            ),
            (
                SignatureType::Secp384r1_PrehashedSha384,
                Box::new(|message| sha2::Sha384::digest(message).to_vec()),
            ),
        ];

        for (sig_type, hasher) in sig_types {
            let method: u8 = sig_type.as_int();

            let seed = b"01234567".repeat(if sig_type.is_secp384r1_variant() {
                6
            } else {
                4
            });

            // Generate key pair from a fixed seed.
            let params = solabi::encode(&(solabi::U256::new(method.into()), solabi::Bytes(seed)));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;

            let (public_key, private_key): (solabi::Bytes<Vec<u8>>, solabi::Bytes<Vec<u8>>) =
                solabi::decode(&output).expect("decode should succeed");
            let public_key = public_key.as_bytes();
            let private_key = private_key.as_bytes();

            // Sign message.
            let params = solabi::encode(&(
                solabi::U256::new(method.into()),
                solabi::Bytes(private_key),
                solabi::Bytes(hasher(message)),
                solabi::Bytes(Vec::new()),
            ));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;

            let signature = output.to_vec();

            // Verify signature.
            let params = solabi::encode(&(
                solabi::U256::new(method.into()),
                solabi::Bytes(public_key),
                solabi::Bytes(hasher(message)),
                solabi::Bytes(Vec::new()),
                solabi::Bytes(signature),
            ));
            let output = call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
                ]),
                &params,
                10_000_000,
            )
            .unwrap()
            .unwrap()
            .output;
            let status: bool = solabi::decode(&output).expect("decode should succeed");
            assert!(status);
        }
    }

    #[test]
    fn test_signing_params() {
        fn push_all_and_test(
            method: Option<u8>,
            pk: Option<&[u8]>,
            context: Option<&[u8]>,
            message: Option<&[u8]>,
        ) -> Option<PrecompileResult> {
            let def_pk = signature::MemorySigner::new_from_seed(
                SignatureType::Ed25519_Oasis,
                b"01234567890123456789012345678901",
            )
            .unwrap()
            .to_bytes();
            let def_ctx = b"default context";
            let def_msg = b"default message";

            let ctx_method = if context.map(|o| o.len()).unwrap_or(1) == 0 {
                SignatureType::Ed25519_Pure.as_int()
            } else {
                SignatureType::Ed25519_Oasis.as_int()
            };

            let params = solabi::encode(&(
                solabi::U256::new(method.unwrap_or(ctx_method).into()),
                solabi::Bytes(pk.map(|o| o.to_vec()).unwrap_or(def_pk)),
                solabi::Bytes(context.unwrap_or(def_ctx).to_vec()),
                solabi::Bytes(message.unwrap_or(def_msg).to_vec()),
            ));
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06,
                ]),
                &params,
                10_000_000,
            )
        }

        // Bogus method.
        push_all_and_test(Some(55), None, None, None)
            .expect("call should return something")
            .expect_err("call should fail");

        // All ok, with context.
        push_all_and_test(None, None, None, None)
            .expect("call should return something")
            .expect("call should succeed");

        // All ok, raw.
        push_all_and_test(None, None, Some(b""), None)
            .expect("call should return something")
            .expect("call should succeed");
    }

    fn bench_signer(
        b: &mut Bencher,
        signature_type: SignatureType,
        context_long: bool,
        message_long: bool,
    ) {
        let seed = b"01234567".repeat(if signature_type.is_secp384r1_variant() {
            6
        } else {
            4
        });
        let signer = signature::MemorySigner::new_from_seed(signature_type, &seed).unwrap();

        let message = b"0123456789".repeat(if message_long { 200 } else { 1 });
        let (context, message) = if signature_type.is_prehashed() {
            (
                if signature_type.is_secp384r1_variant() {
                    <sha2::Sha384 as sha2::digest::Digest>::digest(&message).to_vec()
                } else {
                    <sha2::Sha256 as sha2::digest::Digest>::digest(&message).to_vec()
                },
                vec![],
            )
        } else {
            (
                b"0123456789".repeat(if context_long { 200 } else { 1 }),
                message,
            )
        };

        let params = solabi::encode(&(
            solabi::U256::new(signature_type.as_int().into()),
            solabi::Bytes(signer.to_bytes()),
            solabi::Bytes(context),
            solabi::Bytes(message),
        ));

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06,
                ]),
                &params,
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[bench]
    fn bench_sign_ed25519_shortctx_shortmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Ed25519_Oasis, false, false);
    }

    #[bench]
    fn bench_sign_ed25519_shortctx_longmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Ed25519_Oasis, false, true);
    }

    #[bench]
    fn bench_sign_ed25519_longctx_shortmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Ed25519_Oasis, true, false);
    }

    #[bench]
    fn bench_sign_ed25519_longctx_longmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Ed25519_Oasis, true, true);
    }

    #[bench]
    fn bench_sign_secp256k1_short(b: &mut Bencher) {
        bench_signer(b, SignatureType::Secp256k1_Oasis, false, false);
    }

    #[bench]
    fn bench_sign_secp256k1_long(b: &mut Bencher) {
        bench_signer(b, SignatureType::Secp256k1_Oasis, false, true);
    }

    #[bench]
    fn bench_sign_secp256k1_prehashed_sha256(b: &mut Bencher) {
        bench_signer(b, SignatureType::Secp256k1_PrehashedSha256, false, false);
    }

    #[bench]
    fn bench_sign_secp256r1_prehashed_sha256(b: &mut Bencher) {
        bench_signer(b, SignatureType::Secp256r1_PrehashedSha256, false, false);
    }

    #[bench]
    fn bench_sign_secp384r1_prehashed_sha384(b: &mut Bencher) {
        bench_signer(b, SignatureType::Secp384r1_PrehashedSha384, false, false);
    }

    #[bench]
    fn bench_sign_sr25519_shortctx_shortmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Sr25519_Pure, false, false);
    }

    #[bench]
    fn bench_sign_sr25519_shortctx_longmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Sr25519_Pure, false, true);
    }

    #[bench]
    fn bench_sign_sr25519_longctx_shortmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Sr25519_Pure, true, false);
    }

    #[bench]
    fn bench_sign_sr25519_longctx_longmsg(b: &mut Bencher) {
        bench_signer(b, SignatureType::Sr25519_Pure, true, true);
    }

    #[test]
    fn test_verification_params() {
        fn push_all_and_test(
            method: Option<u8>,
            pk: Option<&[u8]>,
            signature: Option<&[u8]>,
            context: Option<&[u8]>,
            message: Option<&[u8]>,
        ) -> Option<PrecompileResult> {
            let def_pk = signature::MemorySigner::new_from_seed(
                SignatureType::Ed25519_Oasis,
                b"01234567890123456789012345678901",
            )
            .unwrap()
            .public_key()
            .as_bytes()
            .to_vec();
            let def_sig: signature::Signature = hex::decode("6377cc65a95c5cbc2e9bb59a7a8bc6b9ab70517c49eeefa359302750347b585865b7d7dd0e46b43f81b20bd45b727286cbca50725f09c0793352c7d383e8ed08").unwrap().into();
            let def_ctx = b"default context";
            let def_msg = b"default message";

            let params = solabi::encode(&(
                solabi::U256::new(
                    method
                        .unwrap_or(SignatureType::Ed25519_Oasis.as_int())
                        .into(),
                ),
                solabi::Bytes(pk.map(|o| o.to_vec()).unwrap_or(def_pk)),
                solabi::Bytes(context.unwrap_or(def_ctx).to_vec()),
                solabi::Bytes(message.unwrap_or(def_msg).to_vec()),
                solabi::Bytes(signature.unwrap_or(def_sig.as_ref()).to_vec()),
            ));
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
                ]),
                &params,
                10_000_000,
            )
        }

        // Bogus method.
        push_all_and_test(Some(55), None, None, None, None)
            .expect("call should return something")
            .expect_err("call should fail");

        // Invalid public key.
        let zeroes: Vec<u8> = vec![0; 32];
        let mut output = push_all_and_test(None, Some(&zeroes), None, None, None)
            .expect("call should return something")
            .expect("call should succeed")
            .output;
        // Verification should have failed.
        let status: bool = solabi::decode(&output).expect("decode should succeed");
        assert!(!status);

        // Invalid signature.
        let long_zeroes: Vec<u8> = vec![0; 64];
        output = push_all_and_test(None, None, Some(&long_zeroes), None, None)
            .expect("call should return something")
            .expect("call should succeed")
            .output;
        // Verification should have failed.
        let status: bool = solabi::decode(&output).expect("decode should succeed");
        assert!(!status);

        // All ok.
        output = push_all_and_test(None, None, None, None, None)
            .expect("call should return something")
            .expect("call should succeed")
            .output;
        let status: bool = solabi::decode(&output).expect("decode should succeed");
        assert!(status);
    }

    fn bench_verification(
        b: &mut Bencher,
        signature_type: SignatureType,
        context_long: bool,
        message_long: bool,
    ) {
        let seed = b"01234567".repeat(if signature_type.is_secp384r1_variant() {
            6
        } else {
            4
        });
        let signer = signature::MemorySigner::new_from_seed(signature_type, &seed).unwrap();

        let message = b"0123456789".repeat(if message_long { 200 } else { 1 });
        let (context, message) = if signature_type.is_prehashed() {
            (
                if signature_type.is_secp384r1_variant() {
                    <sha2::Sha384 as sha2::digest::Digest>::digest(&message).to_vec()
                } else {
                    <sha2::Sha256 as sha2::digest::Digest>::digest(&message).to_vec()
                },
                vec![],
            )
        } else {
            (
                b"0123456789".repeat(if context_long { 200 } else { 1 }),
                message,
            )
        };
        let signature = signer.sign(&context, &message).unwrap();

        let params = solabi::encode(&(
            solabi::U256::new(signature_type.as_int().into()),
            solabi::Bytes(signer.public_key().as_bytes().to_vec()),
            solabi::Bytes(context),
            solabi::Bytes(message),
            solabi::Bytes(signature.as_ref().to_vec()),
        ));

        b.iter(|| {
            call_contract(
                H160([
                    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
                ]),
                &params,
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }

    #[bench]
    fn bench_verify_ed25519_shortctx_shortmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Ed25519_Oasis, false, false);
    }

    #[bench]
    fn bench_verify_ed25519_shortctx_longmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Ed25519_Oasis, false, true);
    }

    #[bench]
    fn bench_verify_ed25519_longctx_shortmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Ed25519_Oasis, true, false);
    }

    #[bench]
    fn bench_verify_ed25519_longctx_longmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Ed25519_Oasis, true, true);
    }

    #[bench]
    fn bench_verify_secp256k1_short(b: &mut Bencher) {
        bench_verification(b, SignatureType::Secp256k1_Oasis, false, false);
    }

    #[bench]
    fn bench_verify_secp256k1_long(b: &mut Bencher) {
        bench_verification(b, SignatureType::Secp256k1_Oasis, false, true);
    }

    #[bench]
    fn bench_verify_secp256k1_prehashed_sha256(b: &mut Bencher) {
        bench_verification(b, SignatureType::Secp256k1_PrehashedSha256, false, false);
    }

    #[bench]
    fn bench_verify_secp256r1_prehashed_sha256(b: &mut Bencher) {
        bench_verification(b, SignatureType::Secp256r1_PrehashedSha256, false, false);
    }

    #[bench]
    fn bench_verify_secp384r1_prehashed_sha384(b: &mut Bencher) {
        bench_verification(b, SignatureType::Secp384r1_PrehashedSha384, false, false);
    }

    #[bench]
    fn bench_verify_sr25519_shortctx_shortmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Sr25519_Pure, false, false);
    }

    #[bench]
    fn bench_verify_sr25519_shortctx_longmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Sr25519_Pure, false, true);
    }

    #[bench]
    fn bench_verify_sr25519_longctx_shortmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Sr25519_Pure, true, false);
    }

    #[bench]
    fn bench_verify_sr25519_longctx_longmsg(b: &mut Bencher) {
        bench_verification(b, SignatureType::Sr25519_Pure, true, true);
    }
}
