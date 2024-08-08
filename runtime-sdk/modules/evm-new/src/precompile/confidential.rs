//! Implements the confidential precompiles.
use std::{collections::HashMap, convert::TryInto};

use ethabi::{ParamType, Token};
use hmac::{Hmac, Mac};
use once_cell::sync::Lazy;
use revm::{
    precompile::{
        calc_linear_cost_u32, PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult,
    },
    primitives::Bytes,
};

use oasis_runtime_sdk::{
    core::common::crypto::mrae::deoxysii::{DeoxysII, KEY_SIZE, NONCE_SIZE},
    crypto::signature::{self, SignatureType},
};

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

// TODO: call_random_bytes

pub(super) fn call_curve25519_compute_public(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let cost = CURVE25519_COMPUTE_PUBLIC_COST;
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Input encoding: bytes32 private.
    if input.len() != 32 {
        return Err(PrecompileError::Other("input length must be 32 bytes".into()).into());
    }

    let input = input.as_ref();
    let private = <&[u8; WORD]>::try_from(input).unwrap();
    let secret = x25519_dalek::StaticSecret::from(*private);
    let output = x25519_dalek::PublicKey::from(&secret).as_bytes().to_vec();
    Ok(PrecompileOutput::new(cost, output.into()))
}

pub(super) fn call_x25519_derive(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let cost = calc_linear_cost_u32(input.len(), X25519_KEY_DERIVATION_BASE_COST, 0);
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Input encoding: bytes32 public || bytes32 private.
    let mut public = [0u8; WORD];
    let mut private = [0u8; WORD];
    if input.len() != 2 * WORD {
        return Err(PrecompileError::Other("input length must be 64 bytes".into()).into());
    }
    public.copy_from_slice(&input[0..WORD]);
    private.copy_from_slice(&input[WORD..]);

    let public = x25519_dalek::PublicKey::from(public);
    let private = x25519_dalek::StaticSecret::from(private);

    let mut kdf = Hmac::<sha2::Sha512_256>::new_from_slice(b"MRAE_Box_Deoxys-II-256-128")
        .map_err(|_| PrecompileError::Other("unable to create key derivation function".into()))?;
    kdf.update(private.diffie_hellman(&public).as_bytes());

    let mut derived_key = [0u8; KEY_SIZE];
    let digest = kdf.finalize();
    derived_key.copy_from_slice(&digest.into_bytes()[..KEY_SIZE]);

    let output = derived_key.to_vec();
    Ok(PrecompileOutput::new(cost, output.into()))
}

#[allow(clippy::type_complexity)]
fn decode_deoxysii_call_args(
    input: &[u8],
) -> Result<([u8; KEY_SIZE], [u8; NONCE_SIZE], Vec<u8>, Vec<u8>), PrecompileErrors> {
    let mut call_args = ethabi::decode(
        &[
            ParamType::FixedBytes(32), // key
            ParamType::FixedBytes(32), // nonce
            ParamType::Bytes,          // plain or ciphertext
            ParamType::Bytes,          // associated data
        ],
        input,
    )
    .map_err(|e| PrecompileError::Other(e.to_string()))?;
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

pub(super) fn call_deoxysii_seal(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let cost = calc_linear_cost_u32(input.len(), DEOXYSII_BASE_COST, DEOXYSII_WORD_COST);
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let (key, nonce, text, ad) = decode_deoxysii_call_args(input)?;
    let deoxysii = DeoxysII::new(&key);
    let encrypted = deoxysii.seal(&nonce, text, ad);

    Ok(PrecompileOutput::new(cost, encrypted.into()))
}

pub(super) fn call_deoxysii_open(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let cost = calc_linear_cost_u32(input.len(), DEOXYSII_BASE_COST, DEOXYSII_WORD_COST);
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let (key, nonce, ciphertext, ad) = decode_deoxysii_call_args(input)?;
    let ciphertext = ciphertext.to_vec();
    let deoxysii = DeoxysII::new(&key);

    match deoxysii.open(&nonce, ciphertext, ad) {
        Ok(decrypted) => Ok(PrecompileOutput::new(cost, decrypted.into())),
        Err(_) => Err(PrecompileError::Other("revert".into()).into()), // XXX: How to exit with a revert?
    }
}

pub(super) fn call_keypair_generate(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let mut call_args = ethabi::decode(
        &[
            ParamType::Uint(256), // method
            ParamType::Bytes,     // seed
        ],
        input,
    )
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    let seed = call_args.pop().unwrap().into_bytes().unwrap();
    let method: usize = call_args
        .pop()
        .unwrap()
        .into_uint()
        .unwrap()
        .try_into()
        .map_err(|_| PrecompileError::Other("method identifier out of bounds".into()))?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileError::Other("method identifier out of bounds".into()))?
        .try_into()
        .map_err(|_| PrecompileError::Other("unknown signature type".into()))?;

    let cost = calc_linear_cost_u32(
        input.len(),
        *KEYPAIR_GENERATE_BASE_COST
            .get(&sig_type)
            .ok_or(PrecompileError::Other("unknown signature type".into()))?,
        0,
    );
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let signer = signature::MemorySigner::new_from_seed(sig_type, &seed)
        .map_err(|err| PrecompileError::Other(format!("error creating signer: {err}")))?;
    let public = signer.public_key().as_bytes().to_vec();
    let private = signer.to_bytes();

    let output = ethabi::encode(&[Token::Bytes(public), Token::Bytes(private)]);
    Ok(PrecompileOutput::new(cost, output.into()))
}

pub(super) fn call_sign(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let mut call_args = ethabi::decode(
        &[
            ParamType::Uint(256), // signature type
            ParamType::Bytes,     // private key
            ParamType::Bytes,     // context or precomputed hash bytes
            ParamType::Bytes,     // message; should be zero-length if precomputed hash given
        ],
        input,
    )
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    let message = call_args.pop().unwrap().into_bytes().unwrap();
    let ctx_or_hash = call_args.pop().unwrap().into_bytes().unwrap();
    let pk = call_args.pop().unwrap().into_bytes().unwrap();
    let method = call_args
        .pop()
        .unwrap()
        .into_uint()
        .unwrap()
        .try_into()
        .map_err(|_| PrecompileError::Other("signature type identifier out of bounds".into()))?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileError::Other("signature type identifier out of bounds".into()))?
        .try_into()
        .map_err(|_| PrecompileError::Other("unknown signature type".into()))?;

    let costs = *SIGN_MESSAGE_COST
        .get(&sig_type)
        .ok_or(PrecompileError::Other("unknown signature type".into()))?;
    let cost = calc_linear_cost_u32(input.len(), costs.0, costs.1);
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let signer = signature::MemorySigner::from_bytes(sig_type, &pk)
        .map_err(|e| PrecompileError::Other(format!("error creating signer: {e}")))?;

    let result = signer.sign_by_type(sig_type, &ctx_or_hash, &message);
    let result =
        result.map_err(|e| PrecompileError::Other(format!("error signing message: {e}")))?;
    let output: Vec<u8> = result.into();

    Ok(PrecompileOutput::new(cost, output.into()))
}

pub(super) fn call_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let mut call_args = ethabi::decode(
        &[
            ParamType::Uint(256), // signature type
            ParamType::Bytes,     // public key
            ParamType::Bytes,     // context or precomputed hash bytes
            ParamType::Bytes,     // message; should be zero-length if precomputed hash given
            ParamType::Bytes,     // signature
        ],
        input,
    )
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    let signature = call_args.pop().unwrap().into_bytes().unwrap();
    let message = call_args.pop().unwrap().into_bytes().unwrap();
    let ctx_or_hash = call_args.pop().unwrap().into_bytes().unwrap();
    let pk = call_args.pop().unwrap().into_bytes().unwrap();
    let method = call_args
        .pop()
        .unwrap()
        .into_uint()
        .unwrap()
        .try_into()
        .map_err(|_| PrecompileError::Other("signature type identifier out of bounds".into()))?;

    let sig_type: SignatureType = <usize as TryInto<u8>>::try_into(method)
        .map_err(|_| PrecompileError::Other("signature type identifier out of bounds".into()))?
        .try_into()
        .map_err(|_| PrecompileError::Other("unknown signature type".into()))?;

    let costs = *VERIFY_MESSAGE_COST
        .get(&sig_type)
        .ok_or(PrecompileError::Other("unknown signature type".into()))?;
    let cost = calc_linear_cost_u32(input.len(), costs.0, costs.1);
    if cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let signature: signature::Signature = signature.into();
    let public_key = signature::PublicKey::from_bytes(sig_type, &pk)
        .map_err(|_| PrecompileError::Other("error reading public key".into()))?;

    let result = public_key.verify_by_type(sig_type, &ctx_or_hash, &message, &signature);

    let output = ethabi::encode(&[Token::Bool(result.is_ok())]);
    Ok(PrecompileOutput::new(cost, output.into()))
}
