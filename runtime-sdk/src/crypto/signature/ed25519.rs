//! Ed25519 signatures.
use std::convert::TryInto;

use curve25519_dalek::{
    digest::{consts::U64, Digest},
    edwards::CompressedEdwardsY,
};
use sha2::Sha512Trunc256;

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

    /// Verify signature of a pre-hashed message.
    pub fn verify_digest<D>(&self, digest: D, signature: &Signature) -> Result<(), Error>
    where
        D: ed25519_dalek::Digest<OutputSize = U64>,
    {
        let sig: ed25519_dalek::Signature = signature
            .as_ref()
            .try_into()
            .map_err(|_| Error::MalformedSignature)?;
        let pk = ed25519_dalek::PublicKey::from_bytes(self.as_bytes())
            .map_err(|_| Error::MalformedPublicKey)?;
        pk.verify_prehashed(digest, None, &sig)
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

/// A memory-backed signer for Ed25519.
pub struct MemorySigner {
    sk: ed25519_dalek::ExpandedSecretKey,
}

impl MemorySigner {
    pub fn sign_digest<D>(&self, digest: D) -> Result<Signature, Error>
    where
        D: ed25519_dalek::Digest<OutputSize = U64>,
    {
        let pk = ed25519_dalek::PublicKey::from(&self.sk);
        self.sk
            .sign_prehashed(digest, &pk, None)
            .map_err(|_| Error::SigningError)
            .map(|sig| sig.to_bytes().to_vec().into())
    }
}

impl super::Signer for MemorySigner {
    fn new_from_seed(seed: &[u8]) -> Result<Self, Error> {
        let sk = ed25519_dalek::SecretKey::from_bytes(seed).map_err(|_| Error::InvalidArgument)?;
        let esk = ed25519_dalek::ExpandedSecretKey::from(&sk);
        Ok(Self { sk: esk })
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            sk: ed25519_dalek::ExpandedSecretKey::from_bytes(bytes)
                .map_err(|_| Error::MalformedPrivateKey)?,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.sk.to_bytes().to_vec()
    }

    fn public_key(&self) -> super::PublicKey {
        let pk = ed25519_dalek::PublicKey::from(&self.sk);
        super::PublicKey::Ed25519(PublicKey::from_bytes(pk.as_bytes()).unwrap())
    }

    fn sign(&self, context: &[u8], message: &[u8]) -> Result<Signature, Error> {
        let mut digest = Sha512Trunc256::new();
        digest.update(context);
        digest.update(message);
        let message = digest.finalize();

        let pk = ed25519_dalek::PublicKey::from(&self.sk);
        let signature = self.sk.sign(&message, &pk);

        Ok(signature.to_bytes().to_vec().into())
    }

    fn sign_raw(&self, message: &[u8]) -> Result<Signature, Error> {
        let pk = ed25519_dalek::PublicKey::from(&self.sk);
        let signature = self.sk.sign(message, &pk);
        Ok(signature.to_bytes().to_vec().into())
    }
}
