//! Secp256k1 signatures.
use std::{convert::TryFrom, fmt};

use k256::{
    self,
    ecdsa::{self, signature::Verifier},
};

use oasis_core_runtime::common::crypto::hash::Hash;

use crate::crypto::signature::{Error, Signature};

/// A Secp256k1 public key (in compressed form).
#[derive(Clone, Debug)]
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
        let digest = Hash::digest_bytes_list(&[context, message]);
        let sig = ecdsa::Signature::try_from(signature.0.as_ref())
            .map_err(|_| Error::MalformedSignature)?;
        let verify_key = ecdsa::VerifyingKey::from_encoded_point(&self.0)
            .map_err(|_| Error::MalformedPublicKey)?;

        verify_key
            .verify(digest.as_ref(), &sig)
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

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("bytes expected")
            }

            fn visit_bytes<E>(self, data: &[u8]) -> Result<PublicKey, E>
            where
                E: serde::de::Error,
            {
                Ok(PublicKey::from_bytes(data).map_err(serde::de::Error::custom)?)
            }
        }

        Ok(deserializer.deserialize_bytes(BytesVisitor)?)
    }
}
