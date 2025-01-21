//! Various types used by rofl-appd.
use oasis_runtime_sdk::core::common::crypto::{mrae::deoxysii, x25519};

/// Envelope used for storing encrypted secrets.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct SecretEnvelope {
    /// Ephemeral public key used for X25519.
    pub pk: x25519::PublicKey,
    /// Nonce.
    pub nonce: [u8; deoxysii::NONCE_SIZE],
    /// Encrypted secret name.
    pub name: Vec<u8>,
    /// Encrypted secret value.
    pub value: Vec<u8>,
}
