//! Smart contract environment query interface.
use oasis_contract_sdk_types::address::Address;

use crate::types::{
    env::{QueryRequest, QueryResponse},
    InstanceId,
};

/// Environment query trait.
pub trait Env {
    /// Perform an environment query.
    fn query<Q: Into<QueryRequest>>(&self, query: Q) -> QueryResponse;

    /// Returns an address for the contract instance id.
    fn address_for_instance(&self, instance_id: InstanceId) -> Address;

    /// Prints a message to the console. Useful when debugging.
    #[cfg(feature = "debug-utils")]
    fn debug_print(&self, msg: &str);
}

/// Errors that can be returned from crypto functions.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("decryption or additional data authentication failed")]
    DecryptionFailed,
}

/// Crypto helpers trait.
pub trait Crypto {
    /// ECDSA public key recovery function.
    fn ecdsa_recover(&self, input: &[u8]) -> [u8; 65];

    /// Verify an ed25519 message signature.
    fn signature_verify_ed25519(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool;

    /// Verify a secp256k1 message signature.
    fn signature_verify_secp256k1(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool;

    /// Verify an sr25519 message signature.
    fn signature_verify_sr25519(
        &self,
        key: &[u8],
        context: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool;

    /// Derive a symmetric key from a public/private key pair.
    fn x25519_derive_symmetric(&self, public_key: &[u8], private_key: &[u8]) -> [u8; 32];

    /// Encrypt and authenticate a message and authenticate additional data using DeoxysII.
    fn deoxysii_seal(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError>;

    /// Decrypt and authenticate a message and authenticate additional data using DeoxysII.
    fn deoxysii_open(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError>;

    /// Fills `dst` with cryptographically secure random bytes.
    /// Returns the number of bytes written.
    /// If the optional personalization string (`pers`) is provided, it will be mixed into the RNG to provide additional domain separation.
    fn random_bytes(&self, pers: &[u8], dst: &mut [u8]) -> usize;
}
