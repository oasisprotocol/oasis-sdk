//! Account address type.
use std::{convert::TryFrom, fmt};

use bech32::{self, FromBase32, ToBase32, Variant};
use thiserror::Error;

use oasis_core_runtime::{
    common::crypto::hash::Hash, consensus::address::Address as ConsensusAddress,
};

use crate::crypto::{multisig, signature::PublicKey};
use oasis_core_runtime::common::cbor;

const ADDRESS_VERSION_SIZE: usize = 1;
const ADDRESS_DATA_SIZE: usize = 20;
const ADDRESS_SIZE: usize = ADDRESS_VERSION_SIZE + ADDRESS_DATA_SIZE;

const ADDRESS_V0_VERSION: u8 = 0;
/// V0 Ed25519 addres context (shared with consensus layer).
const ADDRESS_V0_ED25519_CONTEXT: &[u8] = b"oasis-core/address: staking";
/// V0 Secp256k1 address context.
const ADDRESS_V0_SECP256K1_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: secp256k1";

const ADDRESS_V0_MODULE_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: module";

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

    /// Creates a new address for a specific module.
    pub fn from_module(module: &'static str, kind: &'static str) -> Self {
        Address::new(
            ADDRESS_V0_MODULE_CONTEXT,
            ADDRESS_V0_VERSION,
            [module, kind].join(".").as_bytes(),
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
        }
    }

    /// Creates a new address from a multisig configuration.
    pub fn from_multisig(config: &multisig::Config) -> Self {
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

impl serde::Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let is_human_readable = serializer.is_human_readable();
        if is_human_readable {
            serializer.serialize_str(&self.to_bech32())
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor {
            type Value = Address;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bytes or string expected")
            }

            fn visit_str<E>(self, data: &str) -> Result<Address, E>
            where
                E: serde::de::Error,
            {
                Address::from_bech32(data).map_err(serde::de::Error::custom)
            }

            fn visit_bytes<E>(self, data: &[u8]) -> Result<Address, E>
            where
                E: serde::de::Error,
            {
                if data.len() != ADDRESS_SIZE {
                    return Err(serde::de::Error::custom(format!(
                        "invalid address length: {}",
                        data.len()
                    )));
                }

                let mut a = [0; ADDRESS_SIZE];
                a.copy_from_slice(&data);
                Ok(Address(a))
            }
        }

        if deserializer.is_human_readable() {
            Ok(deserializer.deserialize_string(BytesVisitor)?)
        } else {
            Ok(deserializer.deserialize_bytes(BytesVisitor)?)
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
        let addr = Address::from_multisig(&config);
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
}
