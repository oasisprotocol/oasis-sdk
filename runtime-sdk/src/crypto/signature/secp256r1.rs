//! Secp256r1 signatures.
use base64::prelude::*;
use digest::{consts::U32, core_api::BlockSizeUser, Digest, FixedOutput, FixedOutputReset};
use k256::sha2::Sha512_256;
use p256::{
    self,
    ecdsa::{
        self,
        signature::{DigestSigner as _, DigestVerifier, Signer as _, Verifier as _},
    },
};
use rand_core::{CryptoRng, RngCore};

use crate::crypto::signature::{Error, Signature};

/// A Secp256r1 public key (in compressed form).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey(p256::EncodedPoint);

impl PublicKey {
    /// Return a byte representation of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Construct a public key from a slice of bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        p256::EncodedPoint::from_bytes(bytes)
            .map_err(|_| Error::MalformedPublicKey)
            .map(PublicKey)
    }

    /// Verify a signature.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        let digest = Sha512_256::new()
            .chain_update(context)
            .chain_update(message);
        self.verify_digest(digest, signature)
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

    /// Verify signature of a pre-hashed message.
    pub fn verify_digest<D>(&self, digest: D, signature: &Signature) -> Result<(), Error>
    where
        D: Digest + FixedOutput<OutputSize = U32>,
    {
        let sig = ecdsa::Signature::from_der(signature.as_ref())
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
        PublicKey::from_bytes(&BASE64_STANDARD.decode(s).unwrap()).unwrap()
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

/// A memory-backed signer for Secp256r1.
pub struct MemorySigner {
    sk: ecdsa::SigningKey,
}

impl MemorySigner {
    pub fn sign_digest<D>(&self, digest: D) -> Result<Signature, Error>
    where
        D: Digest + FixedOutput<OutputSize = U32> + BlockSizeUser + FixedOutputReset,
    {
        let signature: ecdsa::Signature = self.sk.sign_digest(digest);
        Ok(signature.to_der().as_bytes().to_vec().into())
    }
}

impl super::Signer for MemorySigner {
    fn random(rng: &mut (impl RngCore + CryptoRng)) -> Result<Self, Error> {
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        Self::new_from_seed(&seed)
    }

    fn new_from_seed(seed: &[u8]) -> Result<Self, Error> {
        let sk = ecdsa::SigningKey::from_slice(seed).map_err(|_| Error::InvalidArgument)?;
        Ok(Self { sk })
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            sk: ecdsa::SigningKey::from_slice(bytes).map_err(|_| Error::MalformedPrivateKey)?,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.sk.to_bytes().to_vec()
    }

    fn public_key(&self) -> super::PublicKey {
        super::PublicKey::Secp256r1(PublicKey(self.sk.verifying_key().to_encoded_point(true)))
    }

    fn sign(&self, context: &[u8], message: &[u8]) -> Result<Signature, Error> {
        let digest = sha2::Sha256::new()
            .chain_update(context)
            .chain_update(message);
        let signature: ecdsa::Signature = self.sk.sign_digest(digest);
        Ok(signature.to_der().as_bytes().to_vec().into())
    }

    fn sign_raw(&self, message: &[u8]) -> Result<Signature, Error> {
        let signature: ecdsa::Signature = self.sk.sign(message);
        Ok(signature.to_der().as_bytes().to_vec().into())
    }
}
