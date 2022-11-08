//! Ed25519 signatures.
use curve25519_dalek::edwards::CompressedEdwardsY;

use oasis_core_runtime::common::crypto::signature::{
    PublicKey as CorePublicKey, Signature as CoreSignature,
};

use crate::crypto::signature::{Error, Signature};

/// An Ed25519 public key.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(transparent, no_default)]
pub struct PublicKey(CorePublicKey);

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    /// Construct a public key from a slice of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        // CorePublicKey::from doesn't support error checking.
        if bytes.len() != CorePublicKey::len() {
            return Err(Error::MalformedPublicKey);
        }

        // Ensure that the public key is a valid compressed point.
        //
        // Note: This could do the small order public key check,
        // but just assume that signature verification will impose
        // whatever semantics it desires.
        let a = CompressedEdwardsY::from_slice(bytes);
        let _a = match a.decompress() {
            Some(point) => point,
            None => return Err(Error::MalformedPublicKey),
        };

        Ok(PublicKey(CorePublicKey::from(bytes)))
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        // CoreSignature::from doesn't support error checking either.
        if signature.0.len() != CoreSignature::len() {
            return Err(Error::MalformedSignature);
        }
        let sig: &[u8] = signature.0.as_ref();
        let sig = CoreSignature::from(sig);

        sig.verify(&self.0, context, message)
            .map_err(|_| Error::VerificationFailed)
    }

    /// Verify signature without applying domain separation.
    pub fn verify_raw(&self, message: &[u8], signature: &Signature) -> Result<(), Error> {
        // CoreSignature::from doesn't support error checking either.
        if signature.0.len() != CoreSignature::len() {
            return Err(Error::MalformedSignature);
        }
        let sig: &[u8] = signature.0.as_ref();
        let sig = CoreSignature::from(sig);

        sig.verify_raw(&self.0, message)
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
        PublicKey(pk)
    }
}

impl From<&CorePublicKey> for PublicKey {
    fn from(pk: &CorePublicKey) -> PublicKey {
        PublicKey(*pk)
    }
}

impl From<PublicKey> for CorePublicKey {
    fn from(pk: PublicKey) -> CorePublicKey {
        pk.0
    }
}
