//! Sr25519 signatures.
use schnorrkel;
use sha2::{Digest, Sha512Trunc256};

use crate::crypto::signature::{Error, Signature};

/// A Sr25519 public key.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(transparent, no_default)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        // schnorrkel::keys::PublicKey only has to_bytes, which
        // returns a new array.
        //
        // Since we need to return a reference the easiest way to
        // placate the borrow-checker involves just keeping the
        // byte-serialized form of the public key instead of the
        // decompressed one, and doing point-decompression each
        // time we want to actually do something useful.
        &self.0
    }

    /// Construct a public key from a slice of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        // Ensure the bytes represents a valid public key.
        PublicKey::decompress_public_key(bytes)?;
        Ok(PublicKey(bytes.to_vec()))
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        let public_key = PublicKey::decompress_public_key(&self.0)?;

        let signature = schnorrkel::Signature::from_bytes(signature.as_ref())
            .map_err(|_| Error::MalformedSignature)?;

        // Convert the context to a Sr25519 SigningContext.
        let context = schnorrkel::context::SigningContext::new(context);

        // Generate a SigningTranscript from the context, and a pre-hash
        // of the message.
        //
        // Note: This requires using Sha512Trunc256 instead of our hash,
        // due to the need for FixedOutput.
        let mut digest = Sha512Trunc256::new();
        digest.update(message);
        let transcript = context.hash256(digest);

        public_key
            .verify(transcript, &signature)
            .map_err(|_| Error::VerificationFailed)
    }

    fn decompress_public_key(bytes: &[u8]) -> Result<schnorrkel::PublicKey, Error> {
        schnorrkel::PublicKey::from_bytes(bytes).map_err(|_| Error::MalformedPublicKey)
    }
}

impl From<&'static str> for PublicKey {
    fn from(s: &'static str) -> PublicKey {
        PublicKey::from_bytes(&base64::decode(s).unwrap()).unwrap()
    }
}
