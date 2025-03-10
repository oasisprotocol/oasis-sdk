use oasis_runtime_sdk::types::{
    address::SignatureAddressSpec,
    transaction::{AddressSpec, AuthInfo, CallerAddress},
};

use crate::{types::H160, Error};

pub fn from_sigspec(spec: &SignatureAddressSpec) -> Result<H160, Error> {
    match spec {
        SignatureAddressSpec::Secp256k1Eth(pk) => Ok(H160::from_slice(&pk.to_eth_address())),
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
