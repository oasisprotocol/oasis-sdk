//! Secp256k1 signatures.
use k256::{
    self,
    ecdsa::{
        self,
        digest::Digest,
        signature::{DigestVerifier, Verifier as _},
    },
    elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint},
};
use sha2::Sha512Trunc256;

use crate::crypto::signature::{Error, Signature};

/// A Secp256k1 public key (in compressed form).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey(k256::EncodedPoint);

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Return an alternative byte representation used in deriving Ethereum-compatible addresses.
    pub fn to_uncompressed_untagged_bytes(&self) -> Vec<u8> {
        // Our wrapper type only accepts compressed points, so we shouldn't get None.
        let pk = k256::PublicKey::from_encoded_point(&self.0).unwrap();
        pk.to_encoded_point(false).as_bytes()[1..].to_vec()
    }

    /// Construct a public key from a slice of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 33 {
            return Err(Error::MalformedPublicKey);
        }
        let ep = k256::EncodedPoint::from_bytes(bytes).map_err(|_| Error::MalformedPublicKey)?;
        if !ep.is_compressed() {
            // This should never happen due to the size check above.
            return Err(Error::MalformedPublicKey);
        }
        Ok(PublicKey(ep))
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        // Note that we must use Sha512Trunc256 instead of our Hash here,
        // even though it's the same thing, because it implements the Digest
        // trait, so we can use verify_digest() below, which doesn't pre-hash
        // the data (verify() does).
        let mut digest = Sha512Trunc256::new();
        for byte in &[context, message] {
            digest.update(byte);
        }
        let sig = ecdsa::Signature::from_der(signature.0.as_ref())
            .map_err(|_| Error::MalformedSignature)?;
        let verify_key = ecdsa::VerifyingKey::from_encoded_point(&self.0)
            .map_err(|_| Error::MalformedPublicKey)?;

        verify_key
            .verify_digest(digest, &sig)
            .map_err(|_| Error::VerificationFailed)
    }

    /// Verify signature without using any domain separation scheme.
    pub fn verify_raw(&self, message: &[u8], signature: &Signature) -> Result<(), Error> {
        let sig = ecdsa::Signature::from_der(signature.0.as_ref())
            .map_err(|_| Error::MalformedSignature)?;
        let verify_key = ecdsa::VerifyingKey::from_encoded_point(&self.0)
            .map_err(|_| Error::MalformedPublicKey)?;
        verify_key
            .verify(message, &sig)
            .map_err(|_| Error::VerificationFailed)
    }
}

impl From<&'static str> for PublicKey {
    fn from(s: &'static str) -> PublicKey {
        PublicKey::from_bytes(&base64::decode(s).unwrap()).unwrap()
    }
}

impl cbor::Encode for PublicKey {
    fn into_cbor_value(self) -> cbor::Value {
        cbor::Value::ByteString(self.as_bytes().to_vec())
    }
}

impl cbor::Decode for PublicKey {
    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::ByteString(data) => {
                Self::from_bytes(&data).map_err(|_| cbor::DecodeError::UnexpectedType)
            }
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}
