//! Cryptography-related types.
use std::convert::TryFrom;

/// Signature kind.
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum SignatureKind {
    Ed25519 = 0,
    Secp256k1 = 1,
    Sr25519 = 2,
}

impl TryFrom<u32> for SignatureKind {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ed25519),
            1 => Ok(Self::Secp256k1),
            2 => Ok(Self::Sr25519),
            _ => Err(value),
        }
    }
}
