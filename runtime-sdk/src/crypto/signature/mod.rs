//! Cryptographic signatures.
use thiserror::Error;

pub mod context;
pub mod ed25519;
pub mod secp256k1;

/// A public key used for signing.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub enum PublicKey {
    #[cbor(rename = "ed25519")]
    Ed25519(ed25519::PublicKey),

    #[cbor(rename = "secp256k1")]
    Secp256k1(secp256k1::PublicKey),
}

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("malformed public key")]
    MalformedPublicKey,
    #[error("malformed signature")]
    MalformedSignature,
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("invalid argument")]
    InvalidArgument,
}

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            PublicKey::Ed25519(pk) => pk.as_bytes(),
            PublicKey::Secp256k1(pk) => pk.as_bytes(),
        }
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        match self {
            PublicKey::Ed25519(pk) => pk.verify(context, message, signature),
            PublicKey::Secp256k1(pk) => pk.verify(context, message, signature),
        }
    }

    /// Verify a batch of signatures of the same message.
    pub fn verify_batch_multisig(
        context: &[u8],
        message: &[u8],
        public_keys: &[PublicKey],
        signatures: &[Signature],
    ) -> Result<(), Error> {
        if public_keys.len() != signatures.len() {
            return Err(Error::InvalidArgument);
        }

        // TODO: Use actual batch verification.
        for (pk, sig) in public_keys.iter().zip(signatures.iter()) {
            pk.verify(context, message, sig)?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Variable-length opaque signature.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(transparent)]
pub struct Signature(Vec<u8>);

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for Signature {
    fn from(v: Vec<u8>) -> Signature {
        Signature(v)
    }
}

impl From<Signature> for Vec<u8> {
    fn from(s: Signature) -> Vec<u8> {
        s.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_signature_conversion() {
        let raw = vec![0x00, 0x01, 0x02, 0x03];
        let sig = Signature::from(raw.clone());
        let v: Vec<u8> = sig.clone().into();
        assert_eq!(v, raw);

        let vref: &[u8] = v.as_ref();
        assert_eq!(vref, sig.as_ref());
    }
}
