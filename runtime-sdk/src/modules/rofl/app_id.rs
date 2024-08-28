//! ROFL application identifier.
use std::fmt;

use bech32::{Bech32, Hrp};

use crate::{core::common::crypto::hash::Hash, types::address::Address};

const APP_ID_VERSION_SIZE: usize = 1;
const APP_ID_DATA_SIZE: usize = 20;
const APP_ID_SIZE: usize = APP_ID_VERSION_SIZE + APP_ID_DATA_SIZE;

/// V0 identifier version.
const APP_ID_V0_VERSION: u8 = 0;
/// Creator/round/index identifier context.
const APP_ID_CRI_CONTEXT: &[u8] = b"oasis-sdk/rofl: cri app id";
/// Creator/nonce identifier context.
const APP_ID_CN_CONTEXT: &[u8] = b"oasis-sdk/rofl: cn app id";
/// Global name identifier context.
const APP_ID_GLOBAL_NAME_CONTEXT: &[u8] = b"oasis-sdk/rofl: global name app id";

/// Human readable part for Bech32-encoded application identifier.
pub const APP_ID_BECH32_HRP: Hrp = Hrp::parse_unchecked("rofl");

/// Error.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("malformed identifier")]
    MalformedIdentifier,
}

/// ROFL application identifier.
///
/// The application identifier is similar to an address, but using its own separate namespace and
/// derivation scheme as it is not meant to be used as an address.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppId([u8; APP_ID_SIZE]);

impl AppId {
    /// Size of an application identifier in bytes.
    pub const SIZE: usize = APP_ID_SIZE;

    /// Creates a new application identifier from a context, version and data.
    fn new(ctx: &'static [u8], version: u8, data: &[u8]) -> Self {
        let h = Hash::digest_bytes_list(&[ctx, &[version], data]);

        let mut a = [0; APP_ID_SIZE];
        a[..APP_ID_VERSION_SIZE].copy_from_slice(&[version]);
        a[APP_ID_VERSION_SIZE..].copy_from_slice(h.truncated(APP_ID_DATA_SIZE));

        AppId(a)
    }

    /// Creates a new v0 application identifier from a global name.
    pub fn from_global_name(name: &str) -> Self {
        Self::new(
            APP_ID_GLOBAL_NAME_CONTEXT,
            APP_ID_V0_VERSION,
            name.as_bytes(),
        )
    }

    /// Creates a new v0 application identifier from creator/round/index tuple.
    pub fn from_creator_round_index(creator: Address, round: u64, index: u32) -> Self {
        Self::new(
            APP_ID_CRI_CONTEXT,
            APP_ID_V0_VERSION,
            &[creator.as_ref(), &round.to_be_bytes(), &index.to_be_bytes()].concat(),
        )
    }

    /// Creates a new v0 application identifier from creator/nonce tuple.
    pub fn from_creator_nonce(creator: Address, nonce: u64) -> Self {
        Self::new(
            APP_ID_CN_CONTEXT,
            APP_ID_V0_VERSION,
            &[creator.as_ref(), &nonce.to_be_bytes()].concat(),
        )
    }

    /// Tries to create a new identifier from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() != APP_ID_SIZE {
            return Err(Error::MalformedIdentifier);
        }

        let mut a = [0; APP_ID_SIZE];
        a.copy_from_slice(data);

        Ok(AppId(a))
    }

    /// Convert the identifier into raw bytes.
    pub fn into_bytes(self) -> [u8; APP_ID_SIZE] {
        self.0
    }

    /// Tries to create a new identifier from Bech32-encoded string.
    pub fn from_bech32(data: &str) -> Result<Self, Error> {
        let (hrp, data) = bech32::decode(data).map_err(|_| Error::MalformedIdentifier)?;
        if hrp != APP_ID_BECH32_HRP {
            return Err(Error::MalformedIdentifier);
        }

        Self::from_bytes(&data)
    }

    /// Converts an identifier to Bech32 representation.
    pub fn to_bech32(self) -> String {
        bech32::encode::<Bech32>(APP_ID_BECH32_HRP, &self.0).unwrap()
    }
}

impl AsRef<[u8]> for AppId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for AppId {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl From<&'static str> for AppId {
    fn from(s: &'static str) -> AppId {
        AppId::from_bech32(s).unwrap()
    }
}

impl fmt::LowerHex for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in &self.0[..] {
            write!(f, "{i:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bech32())?;
        Ok(())
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bech32())?;
        Ok(())
    }
}

impl cbor::Encode for AppId {
    fn into_cbor_value(self) -> cbor::Value {
        cbor::Value::ByteString(self.as_ref().to_vec())
    }
}

impl cbor::Decode for AppId {
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

impl slog::Value for AppId {
    fn serialize(
        &self,
        _record: &slog::Record<'_>,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_str(key, &self.to_bech32())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::keys;

    #[test]
    fn test_identifier_v0() {
        let creator = keys::alice::address();
        let app_id = AppId::from_creator_round_index(creator, 42, 0);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qr98wz5t6q4x8ng6a5l5v7rqlx90j3kcnun5dwht"
        );

        let creator = keys::bob::address();
        let app_id = AppId::from_creator_round_index(creator, 42, 0);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp"
        );

        let creator = keys::bob::address();
        let app_id = AppId::from_creator_round_index(creator, 1, 0);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qzmuyfwygnmfralgtwrqx8kcm587kwex9y8hf9hf"
        );

        let creator = keys::bob::address();
        let app_id = AppId::from_creator_round_index(creator, 42, 1);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qzmh56f52yd0tcqh757fahzc7ec49s8kaguyylvu"
        );

        let creator = keys::alice::address();
        let app_id = AppId::from_creator_nonce(creator, 0);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qqxxv77j6qy3rh50ah9kxehsh26e2hf7p5r6kwsq"
        );

        let creator = keys::alice::address();
        let app_id = AppId::from_creator_nonce(creator, 1);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qqfuf7u556prwv0wkdt398prhrpat7r3rvr97khf"
        );

        let creator = keys::alice::address();
        let app_id = AppId::from_creator_nonce(creator, 42);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qr90w0m8j7h34c2hhpfmg2wgqmtu0q82vyaxv6e0"
        );

        let creator = keys::bob::address();
        let app_id = AppId::from_creator_nonce(creator, 0);

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qqzuxsh8fkga366kxrze8vpltdjge3rc7qg6tlrg"
        );

        let app_id = AppId::from_global_name("test global app");

        assert_eq!(
            app_id.to_bech32(),
            "rofl1qrev5wq76npkmcv5wxkdxxcu4dhmu704yyl30h43"
        );
    }
}
