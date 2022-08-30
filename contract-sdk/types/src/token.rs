//! Token types.
use std::{convert::TryFrom, fmt};

/// Name/type of the token.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, cbor::Encode)]
#[cbor(transparent)]
pub struct Denomination(Vec<u8>);

impl Denomination {
    /// Maximum length of a denomination.
    pub const MAX_LENGTH: usize = 32;
    /// Denomination in native token.
    pub const NATIVE: Denomination = Denomination(Vec::new());

    /// Whether the denomination represents the native token.
    pub fn is_native(&self) -> bool {
        self.0.is_empty()
    }

    /// Raw representation of a denomination.
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for Denomination {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Denomination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_native() {
            write!(f, "<native>")?;
        } else {
            write!(f, "{}", String::from_utf8_lossy(&self.0))?;
        }
        Ok(())
    }
}

impl std::str::FromStr for Denomination {
    type Err = Error;

    fn from_str(v: &str) -> Result<Self, Self::Err> {
        Self::try_from(v.as_bytes())
    }
}

impl TryFrom<&[u8]> for Denomination {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() > Self::MAX_LENGTH {
            return Err(Error::NameTooLong {
                length: bytes.len(),
            });
        }
        Ok(Self(bytes.to_vec()))
    }
}

impl cbor::Decode for Denomination {
    fn try_default() -> Result<Self, cbor::DecodeError> {
        Ok(Default::default())
    }

    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => {
                Self::try_from(data.as_ref()).map_err(|_| cbor::DecodeError::UnexpectedType)
            }
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<oasis_runtime_sdk::types::token::Denomination> for Denomination {
    fn from(d: oasis_runtime_sdk::types::token::Denomination) -> Self {
        Self(d.into_vec())
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<Denomination> for oasis_runtime_sdk::types::token::Denomination {
    fn from(d: Denomination) -> Self {
        oasis_runtime_sdk::types::token::Denomination::try_from(d.0.as_ref()).unwrap()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "denomination name too long. received length {length} exceeded maximum of {}",
        Denomination::MAX_LENGTH
    )]
    NameTooLong { length: usize },
}

/// Token amount of given denomination in base units.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, cbor::Encode, cbor::Decode)]
pub struct BaseUnits(pub u128, pub Denomination);

impl BaseUnits {
    /// Creates a new token amount of the given denomination.
    pub fn new(amount: u128, denomination: Denomination) -> Self {
        BaseUnits(amount, denomination)
    }

    /// Token amount in base units.
    pub fn amount(&self) -> u128 {
        self.0
    }

    /// Denomination of the token amount.
    pub fn denomination(&self) -> &Denomination {
        &self.1
    }
}

impl fmt::Display for BaseUnits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.0, self.1)?;
        Ok(())
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<oasis_runtime_sdk::types::token::BaseUnits> for BaseUnits {
    fn from(a: oasis_runtime_sdk::types::token::BaseUnits) -> Self {
        Self(a.0, a.1.into())
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<&oasis_runtime_sdk::types::token::BaseUnits> for BaseUnits {
    fn from(a: &oasis_runtime_sdk::types::token::BaseUnits) -> Self {
        Self(a.0, a.1.clone().into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic() {
        let cases = vec![
            // Native denomination.
            (0, Denomination::NATIVE, "824040"),
            (1, Denomination::NATIVE, "82410140"),
            (1000, Denomination::NATIVE, "824203e840"),
            // Custom denomination.
            (0, "test".parse().unwrap(), "82404474657374"),
            (1, "test".parse().unwrap(), "8241014474657374"),
            (1000, "test".parse().unwrap(), "824203e84474657374"),
        ];

        for tc in cases {
            let token = BaseUnits::new(tc.0, tc.1);
            let enc = cbor::to_vec(token.clone());
            assert_eq!(hex::encode(&enc), tc.2, "serialization should match");

            let dec: BaseUnits = cbor::from_slice(&enc).expect("deserialization should succeed");
            assert_eq!(dec, token, "serialization should round-trip");
        }
    }

    #[test]
    fn test_decoding_denomination() {
        macro_rules! assert_rountrip_ok {
            ($bytes:expr) => {
                let enc = cbor::to_vec($bytes.to_vec());
                let dec: Denomination = cbor::from_slice(&enc).unwrap();
                assert_eq!(dec, Denomination::try_from($bytes).unwrap());
                assert_eq!(dec.0, $bytes);
            };
        }

        let bytes_fixture = vec![42u8; Denomination::MAX_LENGTH + 1];

        assert_rountrip_ok!(&bytes_fixture[0..0]);
        assert_rountrip_ok!(&bytes_fixture[0..1]);
        assert_rountrip_ok!(&bytes_fixture[0..Denomination::MAX_LENGTH]);

        // Too long denomination:
        let dec_result: Result<Denomination, _> = cbor::from_slice(&cbor::to_vec(bytes_fixture));
        assert!(dec_result.is_err());
    }
}
