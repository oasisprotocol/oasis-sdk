//! Types related to call formats.
use crate::core::common::crypto::mrae::deoxysii;

/// A call envelope when using the EncryptedX25519DeoxysII format.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct CallEnvelopeX25519DeoxysII {
    /// Caller's ephemeral public key used for X25519.
    pub pk: [u8; 32],
    /// Nonce.
    pub nonce: [u8; deoxysii::NONCE_SIZE],
    /// Encrypted call data.
    pub data: Vec<u8>,
}
