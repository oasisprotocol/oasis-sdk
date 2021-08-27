//! A minimal representation of an Oasis Runtime SDK address.
use std::convert::TryFrom;

use thiserror::Error;

const ADDRESS_VERSION_SIZE: usize = 1;
const ADDRESS_DATA_SIZE: usize = 20;
const ADDRESS_SIZE: usize = ADDRESS_VERSION_SIZE + ADDRESS_DATA_SIZE;

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("malformed address")]
    MalformedAddress,
}

/// An account address.
#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, cbor::Encode, cbor::Decode,
)]
#[cbor(transparent)]
pub struct Address([u8; ADDRESS_SIZE]);

impl Address {
    /// Size of an address in bytes.
    pub const SIZE: usize = ADDRESS_SIZE;

    /// Tries to create a new address from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() != ADDRESS_SIZE {
            return Err(Error::MalformedAddress);
        }

        let mut a = [0; ADDRESS_SIZE];
        a.copy_from_slice(data);

        Ok(Self(a))
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

#[cfg(feature = "oasis-runtime-sdk")]
impl From<oasis_runtime_sdk::types::address::Address> for Address {
    fn from(a: oasis_runtime_sdk::types::address::Address) -> Self {
        Self(a.into_bytes())
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<Address> for oasis_runtime_sdk::types::address::Address {
    fn from(a: Address) -> Self {
        oasis_runtime_sdk::types::address::Address::from_bytes(&a.0).unwrap()
    }
}
