use sha3::Digest as _;

use oasis_runtime_sdk::{
    crypto::signature::secp256k1,
    types::{
        address::SignatureAddressSpec,
        transaction::{AddressSpec, AuthInfo, CallerAddress},
    },
};

use crate::{types::H160, Error};

pub fn from_bytes(b: &[u8]) -> H160 {
    H160::from_slice(&sha3::Keccak256::digest(b)[32 - 20..])
}

pub fn from_secp256k1_public_key(public_key: &secp256k1::PublicKey) -> H160 {
    from_bytes(&public_key.to_uncompressed_untagged_bytes())
}

pub fn from_sigspec(spec: &SignatureAddressSpec) -> Result<H160, Error> {
    match spec {
        SignatureAddressSpec::Secp256k1Eth(pk) => Ok(from_secp256k1_public_key(pk)),
        _ => Err(Error::InvalidSignerType),
    }
}

pub fn from_tx_auth_info(ai: &AuthInfo) -> Result<H160, Error> {
    match &ai.signer_info[0].address_spec {
        AddressSpec::Signature(spec) => from_sigspec(spec),
        AddressSpec::Internal(CallerAddress::EthAddress(address)) => Ok(address.into()),
        _ => Err(Error::InvalidSignerType),
    }
}
