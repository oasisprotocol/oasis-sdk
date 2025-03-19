//! Account address type.
use std::{convert::TryFrom, fmt};

use bech32::{Bech32, Hrp};
use sha3::Digest;
use thiserror::Error;

use oasis_core_runtime::{
    common::{
        crypto::{hash::Hash, signature::PublicKey as ConsensusPublicKey},
        namespace::Namespace,
    },
    consensus::address::Address as ConsensusAddress,
};

use crate::crypto::{
    multisig,
    signature::{ed25519, secp256k1, sr25519, PublicKey},
};

const ADDRESS_VERSION_SIZE: usize = 1;
const ADDRESS_DATA_SIZE: usize = 20;
const ADDRESS_SIZE: usize = ADDRESS_VERSION_SIZE + ADDRESS_DATA_SIZE;

/// V0 address version.
pub const ADDRESS_V0_VERSION: u8 = 0;
/// V0 Ed25519 addres context (shared with consensus layer).
pub const ADDRESS_V0_ED25519_CONTEXT: &[u8] = b"oasis-core/address: staking";
/// V0 Secp256k1 address context.
pub const ADDRESS_V0_SECP256K1ETH_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: secp256k1eth";
/// V0 Sr25519 address context.
pub const ADDRESS_V0_SR25519_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: sr25519";

/// V0 module address context.
pub const ADDRESS_V0_MODULE_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: module";

/// V0 runtime address context.
pub const ADDRESS_RUNTIME_V0_CONTEXT: &[u8] = b"oasis-core/address: runtime";
/// V0 runtime address version.
pub const ADDRESS_RUNTIME_V0_VERSION: u8 = 0;

/// V0 multisig address context.
pub const ADDRESS_V0_MULTISIG_CONTEXT: &[u8] = b"oasis-runtime-sdk/address: multisig";

/// Human readable part for Bech32-encoded addresses.
pub const ADDRESS_BECH32_HRP: Hrp = Hrp::parse_unchecked("oasis");

/// Information for signature-based authentication and public key-based address derivation.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub enum SignatureAddressSpec {
    /// Ed25519 address derivation compatible with the consensus layer.
    #[cbor(rename = "ed25519")]
    Ed25519(ed25519::PublicKey),

    /// Ethereum-compatible address derivation from Secp256k1 public keys.
    #[cbor(rename = "secp256k1eth")]
    Secp256k1Eth(secp256k1::PublicKey),

    /// Sr25519 address derivation.
    #[cbor(rename = "sr25519")]
    Sr25519(sr25519::PublicKey),
}

impl SignatureAddressSpec {
    /// Try to construct an authentication/address derivation specification from the given public
    /// key. In case the given scheme is not supported, it returns `None`.
    pub fn try_from_pk(pk: &PublicKey) -> Option<Self> {
        match pk {
            PublicKey::Ed25519(pk) => Some(Self::Ed25519(pk.clone())),
            PublicKey::Secp256k1(pk) => Some(Self::Secp256k1Eth(pk.clone())),
            PublicKey::Sr25519(pk) => Some(Self::Sr25519(pk.clone())),
            _ => None,
        }
    }

    /// Public key of the authentication/address derivation specification.
    pub fn public_key(&self) -> PublicKey {
        match self {
            Self::Ed25519(pk) => PublicKey::Ed25519(pk.clone()),
            Self::Secp256k1Eth(pk) => PublicKey::Secp256k1(pk.clone()),
            Self::Sr25519(pk) => PublicKey::Sr25519(pk.clone()),
        }
    }
}

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

        Self(a)
    }

    /// Tries to create a new address from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() != ADDRESS_SIZE {
            return Err(Error::MalformedAddress);
        }

        let mut a = [0; ADDRESS_SIZE];
        a.copy_from_slice(data);

        Ok(Self(a))
    }

    /// Convert the address into raw bytes.
    pub fn into_bytes(self) -> [u8; ADDRESS_SIZE] {
        self.0
    }

    /// Creates a new address for a specific module and kind.
    pub fn from_module(module: &str, kind: &str) -> Self {
        Self::from_module_raw(module, kind.as_bytes())
    }

    /// Creates a new address for a specific module and raw kind.
    pub fn from_module_raw(module: &str, kind: &[u8]) -> Self {
        Self::new(
            ADDRESS_V0_MODULE_CONTEXT,
            ADDRESS_V0_VERSION,
            &[module.as_bytes(), b".", kind].concat(),
        )
    }

    /// Creates a new runtime address.
    pub fn from_runtime_id(id: &Namespace) -> Self {
        Self::new(
            ADDRESS_RUNTIME_V0_CONTEXT,
            ADDRESS_RUNTIME_V0_VERSION,
            id.as_ref(),
        )
    }

    /// Creates a new address from a public key.
    pub fn from_sigspec(spec: &SignatureAddressSpec) -> Self {
        match spec {
            SignatureAddressSpec::Ed25519(pk) => Self::new(
                ADDRESS_V0_ED25519_CONTEXT,
                ADDRESS_V0_VERSION,
                pk.as_bytes(),
            ),
            SignatureAddressSpec::Secp256k1Eth(pk) => Self::new(
                ADDRESS_V0_SECP256K1ETH_CONTEXT,
                ADDRESS_V0_VERSION,
                // Use a scheme such that we can compute Secp256k1 addresses from Ethereum
                // addresses as this makes things more interoperable.
                &pk.to_eth_address(),
            ),
            SignatureAddressSpec::Sr25519(pk) => Self::new(
                ADDRESS_V0_SR25519_CONTEXT,
                ADDRESS_V0_VERSION,
                pk.as_bytes(),
            ),
        }
    }

    /// Creates a new address from a multisig configuration.
    pub fn from_multisig(config: multisig::Config) -> Self {
        let config_vec = cbor::to_vec(config);
        Self::new(ADDRESS_V0_MULTISIG_CONTEXT, ADDRESS_V0_VERSION, &config_vec)
    }

    /// Creates a new address from an Ethereum-compatible address.
    pub fn from_eth(eth_address: &[u8]) -> Self {
        Self::new(
            ADDRESS_V0_SECP256K1ETH_CONTEXT,
            ADDRESS_V0_VERSION,
            eth_address,
        )
    }

    /// Creates a new address from a consensus-layer Ed25519 public key.
    ///
    /// This is a convenience wrapper and the same result can be obtained by going via the
    /// `from_sigspec` method using the same Ed25519 public key.
    pub fn from_consensus_pk(pk: &ConsensusPublicKey) -> Self {
        Self::from_bytes(ConsensusAddress::from_pk(pk).as_ref()).unwrap()
    }

    /// Tries to create a new address from Bech32-encoded string.
    pub fn from_bech32(data: &str) -> Result<Self, Error> {
        let (hrp, data) = bech32::decode(data).map_err(|_| Error::MalformedAddress)?;
        if hrp != ADDRESS_BECH32_HRP {
            return Err(Error::MalformedAddress);
        }

        Self::from_bytes(&data)
    }

    /// Converts an address to Bech32 representation.
    pub fn to_bech32(self) -> String {
        bech32::encode::<Bech32>(ADDRESS_BECH32_HRP, &self.0).unwrap()
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
    fn from(s: &'static str) -> Self {
        Self::from_bech32(s).unwrap()
    }
}

impl From<Address> for Vec<u8> {
    fn from(a: Address) -> Self {
        a.into_bytes().into()
    }
}

impl fmt::LowerHex for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in &self.0[..] {
            write!(f, "{i:02x}")?;
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
    fn try_default() -> Result<Self, cbor::DecodeError> {
        Ok(Default::default())
    }

    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => {
                Self::from_bytes(&data).map_err(|_| cbor::DecodeError::UnexpectedType)
            }
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

impl slog::Value for Address {
    fn serialize(
        &self,
        _record: &slog::Record<'_>,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_str(key, &self.to_bech32())
    }
}

impl From<Address> for ConsensusAddress {
    fn from(addr: Address) -> ConsensusAddress {
        ConsensusAddress::from(&addr.0)
    }
}

/// Generate a custom Ethereum address with proper domain separation.
pub fn generate_custom_eth_address(domain: &str, kind: &[u8]) -> [u8; 20] {
    sha3::Keccak256::digest(
        [
            &[0xFFu8] as &[u8],                  // Same as CREATE2.
            &[0x00; 20], // Use 0x00000...00 as the creator since this will never be used.
            b"oasis-runtime-sdk/address: ethxx", // Same as salt in CREATE2.
            &sha3::Keccak256::digest(
                [
                    &[0xFEu8] as &[u8], // Use invalid bytecode.
                    b"oasis:",
                    domain.as_bytes(),
                    b".",
                    kind,
                ]
                .concat(),
            ),
        ]
        .concat(),
    )[32 - 20..]
        .try_into()
        .unwrap()
}

#[cfg(test)]
mod test {
    use base64::prelude::*;
    use bech32::Bech32m;

    use super::*;
    use crate::testing::keys;

    #[test]
    fn test_address_ed25519() {
        let spec =
            SignatureAddressSpec::Ed25519("utrdHlX///////////////////////////////////8=".into());

        let addr = Address::from_sigspec(&spec);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz"
        );
    }

    #[test]
    fn test_address_secp256k1eth() {
        let spec = SignatureAddressSpec::Secp256k1Eth(
            "Arra3R5V////////////////////////////////////".into(),
        );

        let addr = Address::from_sigspec(&spec);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qzd7akz24n6fxfhdhtk977s5857h3c6gf5583mcg"
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
    fn test_address_from_bech32_invalid_hrp() {
        assert!(matches!(
            Address::from_bech32("sisoa1qpcprk8jxpsjxw9fadxvzrv9ln7td69yus8rmtux").unwrap_err(),
            Error::MalformedAddress,
        ));
    }

    #[test]
    fn test_address_from_bech32_variants() {
        let b = vec![42u8; ADDRESS_SIZE];
        let bech32_addr = bech32::encode::<Bech32>(ADDRESS_BECH32_HRP, &b).unwrap();
        let bech32m_addr = bech32::encode::<Bech32m>(ADDRESS_BECH32_HRP, &b).unwrap();

        assert!(
            Address::from_bech32(&bech32_addr).is_ok(),
            "bech32 address should be ok"
        );
        assert!(
            Address::from_bech32(&bech32m_addr).is_ok(),
            "bech32m address should be ok",
        );
    }

    #[test]
    fn test_address_into_consensus_address() {
        let spec =
            SignatureAddressSpec::Ed25519("utrdHlX///////////////////////////////////8=".into());
        let addr = Address::from_sigspec(&spec);

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

    #[test]
    fn test_address_from_module() {
        let id: u64 = 42;
        let addr = Address::from_module_raw("contracts", &id.to_be_bytes());

        assert_eq!(
            addr.to_bech32(),
            "oasis1qq398yyk4wt2zxhtt8c66raynelgt6ngh5yq87xg"
        );
    }

    #[test]
    fn test_address_from_eth() {
        let eth_address = hex::decode("dce075e1c39b1ae0b75d554558b6451a226ffe00").unwrap();
        let addr = Address::from_eth(&eth_address);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt"
        );
    }

    #[test]
    fn test_address_from_consensus_pk() {
        // Same test vector as in `test_address_ed25519`.
        let pk: ConsensusPublicKey = BASE64_STANDARD
            .decode("utrdHlX///////////////////////////////////8=")
            .unwrap()
            .into();

        let addr = Address::from_consensus_pk(&pk);
        assert_eq!(
            addr.to_bech32(),
            "oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz"
        );
    }

    #[test]
    fn test_address_raw() {
        let eth_address = hex::decode("dce075e1c39b1ae0b75d554558b6451a226ffe00").unwrap();
        let addr = Address::new(
            ADDRESS_V0_SECP256K1ETH_CONTEXT,
            ADDRESS_V0_VERSION,
            &eth_address,
        );
        assert_eq!(
            addr.to_bech32(),
            "oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt"
        );
    }
}
