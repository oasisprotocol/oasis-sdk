//! Sr25519 signatures.
use base64::prelude::*;
use rand_core::{CryptoRng, RngCore};
use schnorrkel::{self, context::SigningTranscript};
use sha2::{Digest, Sha512_256};

use crate::crypto::signature::{Error, Signature, Signer};

/// A Sr25519 public key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, cbor::Encode, cbor::Decode)]
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

    /// Verify a signature used in Oasis SDK transactions.
    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        // Convert the context to a Sr25519 SigningContext.
        let context = schnorrkel::context::SigningContext::new(context);

        // Generate a SigningTranscript from the context, and a pre-hash
        // of the message.
        //
        // Note: This requires using Sha512_256 instead of our hash,
        // due to the need for FixedOutput.
        let mut digest = Sha512_256::new();
        digest.update(message);
        let transcript = context.hash256(digest);

        self.verify_transcript(transcript, signature)
    }

    /// Verify a signature.
    pub fn verify_raw(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), Error> {
        // Convert the context to a Sr25519 SigningContext.
        let context = schnorrkel::signing_context(context);
        let transcript = context.bytes(message);
        self.verify_transcript(transcript, signature)
    }

    /// Verify a signature using the given transcript.
    pub fn verify_transcript<T: SigningTranscript>(
        &self,
        transcript: T,
        signature: &Signature,
    ) -> Result<(), Error> {
        let public_key = PublicKey::decompress_public_key(&self.0)?;

        let signature = schnorrkel::Signature::from_bytes(signature.as_ref())
            .map_err(|_| Error::MalformedSignature)?;

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
        PublicKey::from_bytes(&BASE64_STANDARD.decode(s).unwrap()).unwrap()
    }
}

/// A memory-backed signer for Sr25519.
pub struct MemorySigner {
    keypair: schnorrkel::Keypair,
}

impl Signer for MemorySigner {
    fn random(rng: &mut (impl RngCore + CryptoRng)) -> Result<Self, Error> {
        Ok(Self {
            keypair: schnorrkel::Keypair::generate_with(rng),
        })
    }

    fn new_from_seed(seed: &[u8]) -> Result<Self, Error> {
        let msk =
            schnorrkel::MiniSecretKey::from_bytes(seed).map_err(|_| Error::MalformedPrivateKey)?;
        let keypair = msk.expand_to_keypair(schnorrkel::ExpansionMode::Ed25519);
        Ok(Self { keypair })
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            keypair: schnorrkel::Keypair::from_half_ed25519_bytes(bytes)
                .map_err(|_| Error::MalformedPrivateKey)?,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.keypair.to_half_ed25519_bytes().to_vec()
    }

    fn public_key(&self) -> super::PublicKey {
        super::PublicKey::Sr25519(PublicKey(self.keypair.public.to_bytes().to_vec()))
    }

    fn sign(&self, context: &[u8], message: &[u8]) -> Result<Signature, Error> {
        let sig = self.keypair.sign_simple(context, message);
        Ok(Signature(sig.to_bytes().to_vec()))
    }

    fn sign_raw(&self, _message: &[u8]) -> Result<Signature, Error> {
        // Sr25519 requires the use of domain separation context.
        Err(Error::InvalidArgument)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_polkadot_vectors() {
        // Test vectors taken from https://github.com/polkadot-js/wasm/blob/10010830094e7d033bd11b16c5e3bc01a7045309/packages/wasm-crypto/src/rs/sr25519.rs.
        let seed = hex::decode("fac7959dbfe72f052e5a0c3c8d6530f202b02fd8f9f5ca3580ec8deb7797479e")
            .unwrap();
        let signer = MemorySigner::new_from_seed(&seed).unwrap();
        assert_eq!(
            hex::encode(signer.public_key().as_bytes()),
            "46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a",
        );

        let raw = hex::decode("28b0ae221c6bb06856b287f60d7ea0d98552ea5a16db16956849aa371db3eb51fd190cce74df356432b410bd64682309d6dedb27c76845daf388557cbac3ca3446ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a").unwrap();
        let signer = MemorySigner::from_bytes(&raw).unwrap();
        assert_eq!(
            hex::encode(signer.public_key().as_bytes()),
            "46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a",
        );

        // Should verify.
        let msg =
            b"I hereby verify that I control 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
        let signature = hex::decode("1037eb7e51613d0dcf5930ae518819c87d655056605764840d9280984e1b7063c4566b55bf292fcab07b369d01095879b50517beca4d26e6a65866e25fec0d83").unwrap();
        let pk = hex::decode("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")
            .unwrap();
        let pk = PublicKey::from_bytes(&pk).unwrap();
        pk.verify_raw(b"substrate", msg, &signature.into()).unwrap();

        // Should verify "wrapped".
        let msg = b"<Bytes>message to sign</Bytes>";
        let signature = hex::decode("48ce2c90e08651adfc8ecef84e916f6d1bb51ebebd16150ee12df247841a5437951ea0f9d632ca165e6ab391532e75e701be6a1caa88c8a6bcca3511f55b4183").unwrap();
        let pk = hex::decode("f84d048da2ddae2d9d8fd6763f469566e8817a26114f39408de15547f6d47805")
            .unwrap();
        let pk = PublicKey::from_bytes(&pk).unwrap();
        pk.verify_raw(b"substrate", msg, &signature.clone().into())
            .unwrap();

        // Should fail on "unwrapped" message.
        let msg = b"message to sign";
        pk.verify_raw(b"substrate", msg, &signature.into())
            .unwrap_err();
    }
}
