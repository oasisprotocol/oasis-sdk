//! Account address type.
use std::{convert::TryFrom, fmt};

use bech32::{self, FromBase32, ToBase32, Variant};
use thiserror::Error;

use oasis_core_runtime::{
    common::{crypto::hash::Hash, namespace::Namespace},
    consensus::address::Address as ConsensusAddress,
};

use crate::crypto::{multisig, signature::PublicKey};

const ADDRESS_VERSION_SIZE: usize = 1;
const ADDRESS_DATA_SIZE: usize = 20;
const ADDRESS_SIZE: usize = ADDRESS_VERSION_SIZE + ADDRESS_DATA_SIZE;

const ADDRESS_V0_VERSION: u8 = 0;
/// V0 Ed25519 addres context (shared with consensus layer).
const ADDRESS_V0_ED25519_CONTEXT: &[u8] = b"oasis-core/address: staking";
/// V0 Secp256k1 address context.
const ADDRESS_V0_SECP256K1_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: secp256k1";
/// V0 Sr25519 address context.
const ADDRESS_V0_SR25519_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: sr25519";

const ADDRESS_V0_MODULE_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: module";

// V0 runtime address.
const ADDRESS_RUNTIME_V0_CONTEXT: &[u8] = b"oasis-core/address: runtime";
const ADDRESS_RUNTIME_V0_VERSION: u8 = 0;

/// V0 multisig address context.
const ADDRESS_V0_MULTISIG_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: multisig";

const ADDRESS_BECH32_HRP: &str = "oasis";

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("malformed address")]
    MalformedAddress,
}

/// An account address.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address([u8; ADDRESS_SIZE]);

impl Address {
    /// Size of an address in bytes.
    pub const SIZE: usize = ADDRESS_SIZE;

    /// Creates a new address from a context, version and data.
    pub fn new(ctx: &'static [u8], version: u8, data: &[u8]) -> Self {
        let h = Hash::digest_bytes_list(&[ctx, &[version], data]);

        let mut a = [0; ADDRESS_SIZE];
        a[..ADDRESS_VERSION_SIZE].copy_from_slice(&[version]);
        a[ADDRESS_VERSION_SIZE..].copy_from_slice(h.truncated(ADDRESS_DATA_SIZE));

        Address(a)
    }

    /// Tries to create a new address from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() != ADDRESS_SIZE {
            return Err(Error::MalformedAddress);
        }

        let mut a = [0; ADDRESS_SIZE];
        a.copy_from_slice(data);

        Ok(Address(a))
    }

    /// Convert the address into raw bytes.
    pub fn into_bytes(self) -> [u8; ADDRESS_SIZE] {
        self.0
    }

    /// Creates a new address for a specific module and kind.
    pub fn from_module(module: &str, kind: &str) -> Self {
        Address::from_module_raw(module, kind.as_bytes())
    }

    /// Creates a new address for a specific module and raw kind.
    pub fn from_module_raw(module: &str, kind: &[u8]) -> Self {
        Address::new(
            ADDRESS_V0_MODULE_CONTEXT,
            ADDRESS_V0_VERSION,
            &[module.as_bytes(), b".", kind].concat(),
        )
    }

    /// Creates a new runtime address.
    pub fn from_runtime_id(id: &Namespace) -> Self {
        Address::new(
            ADDRESS_RUNTIME_V0_CONTEXT,
            ADDRESS_RUNTIME_V0_VERSION,
            id.as_ref(),
        )
    }

    /// Creates a new address from a public key.
    pub fn from_pk(pk: &PublicKey) -> Self {
        match pk {
            PublicKey::Ed25519(pk) => Address::new(
                ADDRESS_V0_ED25519_CONTEXT,
                ADDRESS_V0_VERSION,
                pk.as_bytes(),
            ),
            PublicKey::Secp256k1(pk) => Address::new(
                ADDRESS_V0_SECP256K1_CONTEXT,
                ADDRESS_V0_VERSION,
                pk.as_bytes(),
            ),
            PublicKey::Sr25519(pk) => Address::new(
                ADDRESS_V0_SR25519_CONTEXT,
                ADDRESS_V0_VERSION,
                pk.as_bytes(),
            ),
        }
    }

    /// Creates a new address from a multisig configuration.
    pub fn from_multisig(config: multisig::Config) -> Self {
        let config_vec = cbor::to_vec(config);
        Address::new(ADDRESS_V0_MULTISIG_CONTEXT, ADDRESS_V0_VERSION, &config_vec)
    }

    /// Tries to create a new address from Bech32-encoded string.
    pub fn from_bech32(data: &str) -> Result<Self, Error> {
        let (hrp, data, variant) = bech32::decode(data).map_err(|_| Error::MalformedAddress)?;
        if hrp != ADDRESS_BECH32_HRP {
            return Err(Error::MalformedAddress);
        }
        if variant != Variant::Bech32 {
            return Err(Error::MalformedAddress);
        }
        let data: Vec<u8> = FromBase32::from_base32(&data).map_err(|_| Error::MalformedAddress)?;

        Address::from_bytes(&data)
    }

    /// Converts an address to Bech32 representation.
    pub fn to_bech32(self) -> String {
        bech32::encode(ADDRESS_BECH32_HRP, self.0.to_base32(), Variant::Bech32).unwrap()
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl From<&'static str> for Address {
    fn from(s: &'static str) -> Address {
        Address::from_bech32(s).unwrap()
    }
}

impl fmt::LowerHex for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in &self.0[..] {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bech32())?;
        Ok(())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bech32())?;
        Ok(())
    }
}

impl cbor::Encode for Address {
    fn into_cbor_value(self) -> cbor::Value {
        cbor::Value::ByteString(self.as_ref().to_vec())
    }
}

impl cbor::Decode for Address {
    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => {
                Self::from_bytes(&data).map_err(|_| cbor::DecodeError::UnexpectedType)
            }
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

impl From<Address> for ConsensusAddress {
    fn from(addr: Address) -> ConsensusAddress {
        ConsensusAddress::from(&addr.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{crypto::signature::PublicKey, testing::keys};

    #[test]
    fn test_address_ed25519() {
        let pk = PublicKey::Ed25519("utrdHlX///////////////////////////////////8=".into());

        let addr = Address::from_pk(&pk);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz"
        );
    }

    #[test]
    fn test_address_secp256k1() {
        let pk = PublicKey::Secp256k1("Arra3R5V////////////////////////////////////".into());

        let addr = Address::from_pk(&pk);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qr4cd0sr32m3xcez37ym7rmjp5g88muu8sdfx8u3"
        );
    }

    #[test]
    fn test_address_multisig() {
        let config = multisig::Config {
            signers: vec![
                multisig::Signer {
                    public_key: keys::alice::pk(),
                    weight: 1,
                },
                multisig::Signer {
                    public_key: keys::bob::pk(),
                    weight: 1,
                },
            ],
            threshold: 2,
        };
        let addr = Address::from_multisig(config);
        assert_eq!(
            addr,
            Address::from_bech32("oasis1qpcprk8jxpsjxw9fadxvzrv9ln7td69yus8rmtux").unwrap(),
        );
    }

    #[test]
    fn test_address_try_from_bytes() {
        let bytes_fixture = vec![42u8; ADDRESS_SIZE + 1];
        assert_eq!(
            Address::try_from(&bytes_fixture[0..ADDRESS_SIZE]).unwrap(),
            Address::from_bytes(&bytes_fixture[0..ADDRESS_SIZE]).unwrap()
        );
        assert!(matches!(
            Address::try_from(bytes_fixture.as_slice()).unwrap_err(),
            Error::MalformedAddress
        ));
    }

    #[test]
    fn test_address_into_consensus_address() {
        let pk = PublicKey::Ed25519("utrdHlX///////////////////////////////////8=".into());
        let addr = Address::from_pk(&pk);

        let consensus_addr: ConsensusAddress = addr.into();
        assert_eq!(addr.to_bech32(), consensus_addr.to_bech32())
    }

    #[test]
    fn test_address_from_runtime_id() {
        let runtime_id =
            Namespace::from("80000000000000002aff7f6dfb62720cfd735f2b037b81572fad1b7937d826b3");
        let addr = Address::from_runtime_id(&runtime_id);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qpllh99nhwzrd56px4txvl26atzgg4f3a58jzzad"
        );
    }
}
