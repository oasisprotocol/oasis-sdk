use tiny_keccak::{Hasher, Keccak};

use oasis_runtime_sdk::{
    crypto::signature::{secp256k1, PublicKey},
    types::{
        address::Address,
        transaction::{AddressSpec, AuthInfo},
    },
};

use crate::types::H160;

pub fn from_bytes(b: &[u8]) -> H160 {
    // Caller address is derived by doing Keccak-256 on the
    // secp256k1 public key and taking the last 20 bytes
    // of the result.
    let mut k = Keccak::v256();
    let mut out = [0u8; 32];
    k.update(b);
    k.finalize(&mut out);
    H160::from_slice(&out[32 - 20..])
}

pub fn from_secp256k1_public_key(public_key: &secp256k1::PublicKey) -> H160 {
    from_bytes(&public_key.to_uncompressed_untagged_bytes())
}

pub fn from_non_secp256k1_address(address: &Address) -> H160 {
    from_bytes(&address.as_ref()[1..])
}

pub fn from_public_key(pk: &PublicKey) -> H160 {
    match pk {
        PublicKey::Secp256k1(pk) => from_secp256k1_public_key(pk),
        pk => from_non_secp256k1_address(&Address::from_pk(pk)),
    }
}

pub fn from_tx_auth_info(ai: &AuthInfo) -> H160 {
    match &ai.signer_info[0].address_spec {
        AddressSpec::Signature(PublicKey::Secp256k1(pk)) => from_secp256k1_public_key(pk),
        address_spec => from_non_secp256k1_address(&address_spec.address()),
    }
}
