use ethabi::ParamType;
use evm::{
    executor::stack::{PrecompileFailure, PrecompileOutput},
    Context, ExitError, ExitRevert, ExitSucceed,
};
use hmac::{Hmac, Mac, NewMac as _};
use oasis_runtime_sdk::core::common::crypto::mrae::deoxysii::{DeoxysII, KEY_SIZE, NONCE_SIZE};

use crate::backend::EVMBackendExt;

use super::{linear_cost, multilinear_cost, PrecompileResult};

/// The base setup cost for encryption and decryption.
const DEOXYSII_BASE_COST: u64 = 50_000;
/// The cost for encryption and decryption per word of input.
const DEOXYSII_WORD_COST: u64 = 100;
/// Length of an EVM word, in bytes.
const WORD: usize = 32;

pub(super) fn call_random_bytes<B: EVMBackendExt>(
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
    _is_static: bool,
    backend: &B,
) -> PrecompileResult {
    let mut call_args =
        ethabi::decode(&[ParamType::Uint(256), ParamType::Bytes], input).map_err(|e| {
            PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }
        })?;
    let pers_str = call_args.pop().unwrap().into_bytes().unwrap();
    let num_words_big = call_args.pop().unwrap().into_uint().unwrap();
    let num_words = num_words_big.try_into().unwrap_or(u64::max_value());
    // This operation shouldn't be too cheap to start since it invokes a key manager.
    // Each byte is generated using hashing, so it's neither expensive nor cheap.
    // Thus:
    // * The base gas is 2x the SSTORE gas since storing requires as much effort
    //   as accessing the key manager (which storing does as well).
    // * The word gas is 4x SHA256 gas since the CSPRNG is reasonably expected
    //   to use an efficient cryptographic hash function with some bookkeeping.
    // In any case, it's much cheaper than using a VRF oracle, and even a Solidity DRBG,
    // which has a cost-per-byte upwards of 1000.
    let gas_cost = multilinear_cost(
        target_gas,
        num_words,
        pers_str.len() as u64,
        240,
        60,
        10_000,
    )?;
    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        cost: gas_cost,
        output: backend.random_bytes(num_words, &pers_str),
        logs: Default::default(),
    })
}

pub(super) fn call_x25519_derive(
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
) -> Result<([u8; KEY_SIZE], [u8; NONCE_SIZE], Vec<u8>, Vec<u8>), PrecompileFailure> {
    let mut call_args = ethabi::decode(
        &[
            ParamType::FixedBytes(32), // key
            ParamType::FixedBytes(32), // nonce
            ParamType::Bytes,          // plain or ciphertext
            ParamType::Bytes,          // associated data
        ],
        input,
    )
    .map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;
    let ad = call_args.pop().unwrap().into_bytes().unwrap();
    let text = call_args.pop().unwrap().into_bytes().unwrap();
    let nonce_bytes = call_args.pop().unwrap().into_fixed_bytes().unwrap();
    let key_bytes = call_args.pop().unwrap().into_fixed_bytes().unwrap();

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_bytes[..NONCE_SIZE]);
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&key_bytes);

    Ok((key, nonce, text, ad))
}

pub(super) fn call_deoxysii_seal(
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

pub(super) fn call_deoxysii_open(
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
        Err(_) => Err(PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: vec![],
            cost: gas_cost,
        }),
    }
}

#[cfg(test)]
mod test {
    extern crate test;

    use ethabi::Token;
    use rand::rngs::OsRng;
    use test::Bencher;

    use super::super::test::*;

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
            &ethabi::encode(&[
                Token::FixedBytes(key.to_vec()),
                Token::FixedBytes(nonce.to_vec()),
                Token::Bytes(plaintext.to_vec()),
                Token::Bytes(ad.to_vec()),
            ]),
            10_000_000,
        )
        .expect("call should return something")
        .expect("call should succeed");
        assert_ne!(plaintext.as_slice(), ret_ct.output);

        let ret_pt = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
            ]),
            &ethabi::encode(&[
                Token::FixedBytes(key.to_vec()),
                Token::FixedBytes(nonce.to_vec()),
                Token::Bytes(ret_ct.output),
                Token::Bytes(ad.to_vec()),
            ]),
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
            &ethabi::encode(&[Token::Uint(2.into()), Token::Bytes(vec![0xbe, 0xef])]),
            10_560,
        )
        .unwrap();
        assert_eq!(hex::encode(ret.unwrap().output), "beef02030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f");
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
                &ethabi::encode(&[
                    Token::FixedBytes(key.to_vec()),
                    Token::FixedBytes(nonce.to_vec()),
                    Token::Bytes(plaintext.to_vec()),
                    Token::Bytes(ad.to_vec()),
                ]),
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
                &ethabi::encode(&[
                    Token::FixedBytes(key.to_vec()),
                    Token::FixedBytes(nonce.to_vec()),
                    Token::Bytes(plaintext.to_vec()),
                    Token::Bytes(ad.to_vec()),
                ]),
                10_000_000,
            )
            .expect("call should return something")
            .expect("call should succeed");
        });
    }
}
