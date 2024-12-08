//! Ed25519 signatures.
use std::convert::TryInto;

use base64::prelude::*;
use curve25519_dalek::{digest::consts::U64, edwards::CompressedEdwardsY};
use ed25519_dalek::Signer as _;
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest as _, Sha512, Sha512_256};

use oasis_core_runtime::common::crypto::signature::{
    PublicKey as CorePublicKey, Signature as CoreSignature,
};

use crate::crypto::signature::{Error, Signature, Signer};

/// An Ed25519 public key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, cbor::Encode, cbor::Decode)]
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
        let a = CompressedEdwardsY::from_slice(bytes).unwrap(); // Length is checked above.
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
        let pk: ed25519_dalek::VerifyingKey = self
            .as_bytes()
            .try_into()
            .map_err(|_| Error::MalformedPublicKey)?;
        pk.verify_prehashed(digest, None, &sig)
            .map_err(|_| Error::VerificationFailed)
    }
}

impl From<&'static str> for PublicKey {
    fn from(s: &'static str) -> PublicKey {
        PublicKey::from_bytes(&BASE64_STANDARD.decode(s).unwrap()).unwrap()
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
    key: Key,
}

/// The original version of the Ed25519 signer returned the "expanded" secret key from `to_bytes`.
/// A contract may have stored the "expanded" key and expects its use to continue to succeed.
/// For backwards compatibility, the signer works with both "expanded" and regular keys.
/// New invocations receive a regular/proper key, and from-"expanded" ones get the old behavior.
enum Key {
    Expanded {
        esk: ed25519_dalek::hazmat::ExpandedSecretKey,
        /// The hash output that is used to create the "expanded" secret key.
        /// It is stored to return from `from_bytes` because it is not recoverable from `esk`.
        hash: zeroize::Zeroizing<[u8; 64]>,
    },
    Regular(ed25519_dalek::SigningKey),
}

impl Key {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        match bytes.len() {
            // It's a new/correct-style secret key.
            32 => bytes
                .try_into()
                .map(ed25519_dalek::SigningKey::from_bytes)
                .map(Self::Regular)
                .map_err(|_| Error::MalformedPrivateKey),
            // It's an "expanded" secret key, which is treated as the output of a 64-byte hash function.
            64 => bytes
                .try_into()
                .map(|hash| Self::Expanded {
                    esk: ed25519_dalek::hazmat::ExpandedSecretKey::from_bytes(&hash),
                    hash: hash.into(),
                })
                .map_err(|_| Error::MalformedPrivateKey),
            _ => Err(Error::MalformedPrivateKey),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Expanded { hash, .. } => hash.to_vec(),
            Self::Regular(sk) => sk.to_bytes().to_vec(),
        }
    }

    fn sign(&self, message: &[u8]) -> Signature {
        match self {
            Self::Expanded { esk, .. } => {
                let verifying_key = ed25519_dalek::VerifyingKey::from(esk);
                ed25519_dalek::hazmat::raw_sign::<Sha512>(esk, message, &verifying_key)
            }
            Self::Regular(sk) => sk.sign(message),
        }
        .to_bytes()
        .to_vec()
        .into()
    }

    fn sign_digest<D>(&self, digest: D) -> Result<Signature, Error>
    where
        D: ed25519_dalek::Digest<OutputSize = U64>,
    {
        match self {
            Key::Expanded { esk, .. } => {
                let verifying_key = ed25519_dalek::VerifyingKey::from(esk);
                ed25519_dalek::hazmat::raw_sign_prehashed::<Sha512, _>(
                    esk,
                    digest,
                    &verifying_key,
                    None,
                )
            }
            Key::Regular(sk) => sk.sign_prehashed(digest, None),
        }
        .map_err(|_| Error::SigningError)
        .map(|sig| sig.to_bytes().to_vec().into())
    }

    fn public_key(&self) -> super::PublicKey {
        let pk = match self {
            Self::Expanded { esk, .. } => ed25519_dalek::VerifyingKey::from(esk),
            Self::Regular(sk) => sk.verifying_key(),
        };
        super::PublicKey::Ed25519(PublicKey::from_bytes(pk.as_bytes()).unwrap())
    }
}

impl MemorySigner {
    pub fn sign_digest<D>(&self, digest: D) -> Result<Signature, Error>
    where
        D: ed25519_dalek::Digest<OutputSize = U64>,
    {
        self.key.sign_digest(digest)
    }
}

impl Signer for MemorySigner {
    fn random(rng: &mut (impl RngCore + CryptoRng)) -> Result<Self, Error> {
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        Self::new_from_seed(&seed)
    }

    fn new_from_seed(seed: &[u8]) -> Result<Self, Error> {
        if seed.len() != 32 {
            return Err(Error::MalformedPublicKey);
        }
        Self::from_bytes(seed)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            key: Key::from_bytes(bytes)?,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.key.to_bytes()
    }

    fn public_key(&self) -> super::PublicKey {
        self.key.public_key()
    }

    fn sign(&self, context: &[u8], message: &[u8]) -> Result<Signature, Error> {
        let mut digest = Sha512_256::new();
        digest.update(context);
        digest.update(message);
        Ok(self.key.sign(&digest.finalize()))
    }

    fn sign_raw(&self, message: &[u8]) -> Result<Signature, Error> {
        Ok(self.key.sign(message))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn legacy_esk_equivalence() {
        let seed = [42u8; 32];
        let signer = MemorySigner::new_from_seed(&seed).unwrap();

        let esk = ed25519_dalek::hazmat::ExpandedSecretKey::from(&seed);
        let esk_hash = Sha512::digest(seed);
        let esk_signer = MemorySigner::from_bytes(&esk_hash).unwrap();

        let esk_public_key = super::super::PublicKey::Ed25519(
            PublicKey::from_bytes(&ed25519_dalek::VerifyingKey::from(&esk).to_bytes()).unwrap(),
        );

        assert_eq!(
            esk_signer.to_bytes().as_slice(),
            esk_hash.as_slice(),
            "esk roundtrip"
        );
        assert_eq!(signer.to_bytes(), seed, "sk roundtrip");

        let context = b"tests";
        let message = b"hello, world!";
        let digest = Sha512::new().chain_update(context).chain_update(message);

        let sig = signer.sign(context, message).unwrap();
        let esk_sig = esk_signer.sign(context, message).unwrap();
        assert_eq!(sig, esk_sig, "sig != esk_sig");

        let raw_sig = signer.sign_raw(message).unwrap();
        let esk_raw_sig = esk_signer.sign_raw(message).unwrap();
        assert_eq!(raw_sig, esk_raw_sig, "raw_sig != esk_raw_sig");

        let digest_sig = signer.sign_digest(digest.clone()).unwrap();
        let esk_digest_sig = esk_signer.sign_digest(digest).unwrap();
        assert_eq!(digest_sig, esk_digest_sig, "digest_sig != esk_digest_sig");

        assert_eq!(
            signer.public_key(),
            esk_public_key,
            "signer pk != esk_public_key"
        );
        assert_eq!(
            esk_signer.public_key(),
            esk_public_key,
            "esk_signer pk != esk_pubic_key"
        );
    }
}
