//! Token types.
use serde::{self, Deserialize, Serialize};

pub use oasis_core_runtime::common::quantity::Quantity;

use crate::storage::{DecodableStoreKey, StoreKey};

/// Name/type of the token.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denomination(#[serde(with = "serde_bytes")] Vec<u8>);

impl Denomination {
    /// Maximum length of a remote denomination.
    pub const MAX_LENGTH: usize = 32;
    /// Denomination in native token.
    pub const NATIVE: Denomination = Denomination(Vec::new());

    // TODO: Enforce maximum length during deserialization.

    /// Whether the denomination represents the native token.
    pub fn is_native(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for Denomination {
    fn from(v: &str) -> Denomination {
        Denomination(v.as_bytes().to_vec())
    }
}

impl StoreKey for Denomination {
    fn as_store_key(&self) -> &[u8] {
        &self.0
    }
}

impl StoreKey for &Denomination {
    fn as_store_key(&self) -> &[u8] {
        &self.0
    }
}

impl DecodableStoreKey for Denomination {
    fn from_bytes(v: &[u8]) -> Option<Denomination> {
        if v.len() > Self::MAX_LENGTH {
            return None;
        }
        Some(Denomination(v.into()))
    }
}

/// Token amount of given denomination in base units.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseUnits(Quantity, Denomination);

impl BaseUnits {
    /// Creates a new token amount of the given denomination.
    pub fn new(amount: Quantity, denomination: Denomination) -> Self {
        BaseUnits(amount, denomination)
    }

    /// Token amount in base units.
    pub fn amount(&self) -> &Quantity {
        &self.0
    }

    /// Denomination of the token amount.
    pub fn denomination(&self) -> &Denomination {
        &self.1
    }
}

#[cfg(test)]
mod test {
    use oasis_core_runtime::common::{cbor, quantity::Quantity};

    use super::{BaseUnits, Denomination};

    #[test]
    fn test_basic() {
        let cases = vec![
            // Native denomination.
            (0, Denomination::NATIVE, "824040"),
            (1, Denomination::NATIVE, "82410140"),
            (1000, Denomination::NATIVE, "824203e840"),
            // Custom denomination.
            (0, "test".into(), "82404474657374"),
            (1, "test".into(), "8241014474657374"),
            (1000, "test".into(), "824203e84474657374"),
        ];

        for tc in cases {
            let token = BaseUnits::new(Quantity::from(tc.0), tc.1);
            let enc = cbor::to_vec(&token);
            assert_eq!(hex::encode(&enc), tc.2, "serialization should match");

            let dec: BaseUnits = cbor::from_slice(&enc).expect("deserialization should succeed");
            assert_eq!(dec, token, "serialization should round-trip");
        }
    }
}
