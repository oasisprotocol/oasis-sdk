//! Secp256k1 signatures.
use std::fmt;

use k256::{
    self,
    ecdsa::{self, digest::Digest, signature::DigestVerifier},
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
        let sig = ecdsa::Signature::from_asn1(signature.0.as_ref())
            .map_err(|_| Error::MalformedSignature)?;
        let verify_key = ecdsa::VerifyingKey::from_encoded_point(&self.0)
            .map_err(|_| Error::MalformedPublicKey)?;

        verify_key
            .verify_digest(digest, &sig)
            .map_err(|_| Error::VerificationFailed)
    }
}

impl From<&'static str> for PublicKey {
    fn from(s: &'static str) -> PublicKey {
        PublicKey::from_bytes(&base64::decode(s).unwrap()).unwrap()
    }
}

impl serde::Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.as_bytes())
    }
}

impl<'de> serde::Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor {
            type Value = PublicKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bytes expected")
            }

            fn visit_bytes<E>(self, data: &[u8]) -> Result<PublicKey, E>
            where
                E: serde::de::Error,
            {
                PublicKey::from_bytes(data).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}
