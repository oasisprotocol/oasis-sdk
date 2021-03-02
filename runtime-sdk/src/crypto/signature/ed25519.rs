//! Ed25519 signatures.
use ed25519_dalek::{self, ed25519::signature::Signature as _};
use serde::{Deserialize, Serialize};

use oasis_core_runtime::common::crypto::{hash::Hash, signature::PublicKey as CorePublicKey};

use crate::crypto::signature::{Error, Signature};

/// An Ed25519 public key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicKey(ed25519_dalek::PublicKey);

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Construct a public key from a slice of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(PublicKey(
            ed25519_dalek::PublicKey::from_bytes(bytes).map_err(|_| Error::MalformedPublicKey)?,
        ))
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        let digest = Hash::digest_bytes_list(&[context, message]);
        let sig = ed25519_dalek::Signature::from_bytes(signature.0.as_ref())
            .map_err(|_| Error::MalformedSignature)?;

        self.0
            .verify_strict(digest.as_ref(), &sig)
            .map_err(|_| Error::VerificationFailed)
    }
}

impl From<&'static str> for PublicKey {
    fn from(s: &'static str) -> PublicKey {
        PublicKey::from_bytes(&base64::decode(s).unwrap()).unwrap()
    }
}

impl From<CorePublicKey> for PublicKey {
    fn from(pk: CorePublicKey) -> PublicKey {
        PublicKey::from_bytes(pk.as_ref())
            .expect("types are compatible so conversion must always succeed")
    }
}

impl From<&CorePublicKey> for PublicKey {
    fn from(pk: &CorePublicKey) -> PublicKey {
        PublicKey::from_bytes(pk.as_ref())
            .expect("types are compatible so conversion must always succeed")
    }
}
