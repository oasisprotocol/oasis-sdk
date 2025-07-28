use anyhow::Result;

use oasis_runtime_sdk::core::common::crypto::{mrae::deoxysii, x25519};
use rand::{rngs::OsRng, Rng};

/// Envelope used for storing encrypted secrets.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct SecretEnvelope {
    /// Ephemeral public key used for X25519.
    pub pk: x25519::PublicKey,
    /// Nonce.
    pub nonce: [u8; deoxysii::NONCE_SIZE],
    /// Encrypted secret name.
    pub name: Vec<u8>,
    /// Encrypted secret value.
    pub value: Vec<u8>,
}

impl SecretEnvelope {
    /// Seal the given name/value pair using the provided SEK.
    pub fn seal(sek: &x25519::PublicKey, name: Vec<u8>, value: Vec<u8>) -> Vec<u8> {
        Self::seal_opts(sek, name, value, "")
    }

    /// Seal the given name/value pair using the provided SEK and a custom domain separation context.
    pub fn seal_opts(
        sek: &x25519::PublicKey,
        name: Vec<u8>,
        value: Vec<u8>,
        context: &str,
    ) -> Vec<u8> {
        let sk = x25519::PrivateKey::generate();
        let mut nonce = [0u8; deoxysii::NONCE_SIZE];
        OsRng.fill(&mut nonce);

        let name = deoxysii::box_seal(
            &nonce,
            name,
            [context.as_bytes(), b"name"].concat(),
            &sek.0,
            &sk.0,
        )
        .unwrap();

        let value = deoxysii::box_seal(
            &nonce,
            value,
            [context.as_bytes(), b"value"].concat(),
            &sek.0,
            &sk.0,
        )
        .unwrap();

        let envelope = Self {
            pk: sk.public_key(),
            nonce,
            name,
            value,
        };
        cbor::to_vec(envelope)
    }

    /// Open the encrypted envelope using the provided SEK.
    pub fn open(self, sek: &x25519::PrivateKey) -> Result<(Vec<u8>, Vec<u8>)> {
        self.open_opts(sek, "")
    }

    /// Open the encrypted envelope using the provided SEK and a custom domain separation context.
    pub fn open_opts(self, sek: &x25519::PrivateKey, context: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        let name = deoxysii::box_open(
            &self.nonce,
            self.name,
            [context.as_bytes(), b"name"].concat(), // Prevent mixing name and value.
            &self.pk.0,
            &sek.0,
        )?;

        let value = deoxysii::box_open(
            &self.nonce,
            self.value,
            [context.as_bytes(), b"value"].concat(), // Prevent mixing name and value.
            &self.pk.0,
            &sek.0,
        )?;

        Ok((name, value))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_seal_open() {
        let sek_sk = x25519::PrivateKey::generate();
        let sek_pk = sek_sk.public_key();

        let data = SecretEnvelope::seal(&sek_pk, b"TEST_A".to_vec(), b"hello world".to_vec());
        let envelope: SecretEnvelope = cbor::from_slice(&data).unwrap();
        let (key, value) = envelope.clone().open(&sek_sk).unwrap();
        assert_eq!(&key, b"TEST_A");
        assert_eq!(&value, b"hello world");

        // Opening with a different context should fail.
        let result = envelope.open_opts(&sek_sk, "custom context");
        assert!(result.is_err());
    }

    #[test]
    fn test_seal_open_context() {
        let sek_sk = x25519::PrivateKey::generate();
        let sek_pk = sek_sk.public_key();

        let data = SecretEnvelope::seal_opts(
            &sek_pk,
            b"TEST_A".to_vec(),
            b"hello world".to_vec(),
            "custom context",
        );
        let envelope: SecretEnvelope = cbor::from_slice(&data).unwrap();
        let (key, value) = envelope
            .clone()
            .open_opts(&sek_sk, "custom context")
            .unwrap();
        assert_eq!(&key, b"TEST_A");
        assert_eq!(&value, b"hello world");

        // Opening with a different context should fail.
        let result = envelope.clone().open_opts(&sek_sk, "another context");
        assert!(result.is_err());
        let result = envelope.clone().open(&sek_sk);
        assert!(result.is_err());
    }
}
