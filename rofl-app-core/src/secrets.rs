use anyhow::Result;

use oasis_runtime_sdk::core::common::crypto::{mrae::deoxysii, x25519};
use rand::{rngs::OsRng, Rng};

/// Custom seal options.
#[derive(Clone, Default)]
pub struct SealOptions<'a> {
    /// Domain separation context.
    pub context: &'a str,
    /// Optional private key to use. If `None` an ephemeral key will be generated.
    pub sk: Option<x25519::PrivateKey>,
    /// Optional nonce to use. If `None` a random one will be generated.
    pub nonce: Option<[u8; deoxysii::NONCE_SIZE]>,
}

/// Custom open options.
#[derive(Clone, Default)]
pub struct OpenOptions<'a> {
    /// Domain separation context.
    pub context: &'a str,
}

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
        Self::seal_opts(sek, name, value, Default::default())
    }

    /// Seal the given name/value pair using the provided SEK and a custom domain separation context.
    pub fn seal_opts(
        sek: &x25519::PublicKey,
        name: Vec<u8>,
        value: Vec<u8>,
        opts: SealOptions<'_>,
    ) -> Vec<u8> {
        let sk = opts.sk.unwrap_or_else(x25519::PrivateKey::generate);
        let nonce = opts.nonce.unwrap_or_else(|| {
            let mut nonce = [0u8; deoxysii::NONCE_SIZE];
            OsRng.fill(&mut nonce);
            nonce
        });

        let name = deoxysii::box_seal(
            &nonce,
            name,
            [b"name", opts.context.as_bytes()].concat(), // Prevent mixing name and value.
            &sek.0,
            &sk.0,
        )
        .unwrap();

        let value = deoxysii::box_seal(
            &nonce,
            value,
            [b"value", opts.context.as_bytes()].concat(), // Prevent mixing name and value.
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
        self.open_opts(sek, Default::default())
    }

    /// Open the encrypted envelope using the provided SEK and a custom domain separation context.
    pub fn open_opts(
        self,
        sek: &x25519::PrivateKey,
        opts: OpenOptions<'_>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let name = deoxysii::box_open(
            &self.nonce,
            self.name,
            [b"name", opts.context.as_bytes()].concat(),
            &self.pk.0,
            &sek.0,
        )?;

        let value = deoxysii::box_open(
            &self.nonce,
            self.value,
            [b"value", opts.context.as_bytes()].concat(),
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
        let result = envelope.open_opts(
            &sek_sk,
            OpenOptions {
                context: "custom context",
            },
        );
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
            SealOptions {
                context: "custom context",
                ..Default::default()
            },
        );
        let envelope: SecretEnvelope = cbor::from_slice(&data).unwrap();
        let (key, value) = envelope
            .clone()
            .open_opts(
                &sek_sk,
                OpenOptions {
                    context: "custom context",
                },
            )
            .unwrap();
        assert_eq!(&key, b"TEST_A");
        assert_eq!(&value, b"hello world");

        // Opening with a different context should fail.
        let result = envelope.clone().open_opts(
            &sek_sk,
            OpenOptions {
                context: "another context",
            },
        );
        assert!(result.is_err());
        let result = envelope.clone().open(&sek_sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_vectors() {
        let tvs = vec![(
            "786894cf62ac4f7c32f4faa1f090de99b5781f139703c97ac1fd2fe42a6ca78b",
            "f2a9acca2319c3814adfec136804129860f724f2523d737bfd07d47c88ab4736",
            b"key".to_vec(),
            b"value".to_vec(),
            "",
            "49f7baede65e2b5e930fb0d3f616fe",
            "a462706b5820b9529bf93fa2845b7b2ecf5f472fbb044238fb38f2219f29fa9274f42cd8c33f646e616d65531f98946bf6fd88688312ab1894708b5061a295656e6f6e63654f49f7baede65e2b5e930fb0d3f616fe6576616c756555358249d2da03f725b4138ac0b8b1a59bfd1e3aee89",
        ),
        (
            "786894cf62ac4f7c32f4faa1f090de99b5781f139703c97ac1fd2fe42a6ca78b",
            "f2a9acca2319c3814adfec136804129860f724f2523d737bfd07d47c88ab4736",
            b"key".to_vec(),
            b"value".to_vec(),
            "custom context",
            "49f7baede65e2b5e930fb0d3f616fe",
            "a462706b5820b9529bf93fa2845b7b2ecf5f472fbb044238fb38f2219f29fa9274f42cd8c33f646e616d6553d164de6ba39c2d13758c76613600054b50b8ec656e6f6e63654f49f7baede65e2b5e930fb0d3f616fe6576616c75655520da2f6eeac2985ecfd43c2d4bc9835801e436b567",
        ),
        (
            "786894cf62ac4f7c32f4faa1f090de99b5781f139703c97ac1fd2fe42a6ca78b",
            "f2a9acca2319c3814adfec136804129860f724f2523d737bfd07d47c88ab4736",
            b"key".to_vec(),
            b"value".to_vec(),
            "another context",
            "49f7baede65e2b5e930fb0d3f616fe",
            "a462706b5820b9529bf93fa2845b7b2ecf5f472fbb044238fb38f2219f29fa9274f42cd8c33f646e616d65536d871778fc4e1864364f265765f426fb02e351656e6f6e63654f49f7baede65e2b5e930fb0d3f616fe6576616c756555d69b350049f58ca999177e2e29b0085cce6ee5522f",
        ),
        (
            "786894cf62ac4f7c32f4faa1f090de99b5781f139703c97ac1fd2fe42a6ca78b",
            "f2a9acca2319c3814adfec136804129860f724f2523d737bfd07d47c88ab4736",
            b"key 2".to_vec(),
            b"value 2".to_vec(),
            "",
            "49f7baede65e2b5e930fb0d3f616fe",
            "a462706b5820b9529bf93fa2845b7b2ecf5f472fbb044238fb38f2219f29fa9274f42cd8c33f646e616d6555f6b0e6b3455b09492fde1d385ff85e93816774f342656e6f6e63654f49f7baede65e2b5e930fb0d3f616fe6576616c756557d5ef751e7b5c5be48a35a8b072b203ca7fda1b88bc9601",
        ),
        (
            "786894cf62ac4f7c32f4faa1f090de99b5781f139703c97ac1fd2fe42a6ca78b",
            "f2a9acca2319c3814adfec136804129860f724f2523d737bfd07d47c88ab4736",
            b"key 2".to_vec(),
            b"value 2".to_vec(),
            "",
            "000000000000000000000000000000",
            "a462706b5820b9529bf93fa2845b7b2ecf5f472fbb044238fb38f2219f29fa9274f42cd8c33f646e616d6555220fcc43bbe1b9323287a8f5abacfd7937c865bb59656e6f6e63654f0000000000000000000000000000006576616c75655736b5bb4e6a1418a84a15f1f87f1e85651e1dd4d58e9f5f",
        ),
        (
            "6c27dd81f1e3e52e8d98b02cd52d1799af745a2d57bb3810753868ce06f726cf",
            "ed2cba52ec2583618b4cdcd737928fbb7d749e98e17835ee3575b3b28964a753",
            b"key".to_vec(),
            b"value".to_vec(),
            "",
            "49f7baede65e2b5e930fb0d3f616fe",
            "a462706b582077e51262274c9505a89db23b4921105d4ea1d8bd49bc99087281f3f92c812144646e616d65533fd8dabddfaf5cf0d1505f9ea0c090d241ed71656e6f6e63654f49f7baede65e2b5e930fb0d3f616fe6576616c756555bd080af9cb24129afafc5c461d231385c22ebb83b5",
        )];

        for (idx, tv) in tvs.iter().enumerate() {
            let sk1: [u8; x25519::PRIVATE_KEY_LENGTH] =
                hex::decode(tv.0).unwrap().try_into().unwrap();
            let sk1: x25519::PrivateKey = sk1.into();
            let sk2: [u8; x25519::PRIVATE_KEY_LENGTH] =
                hex::decode(tv.1).unwrap().try_into().unwrap();
            let sk2: x25519::PrivateKey = sk2.into();
            let key = tv.2.clone();
            let value = tv.3.clone();
            let context = tv.4;
            let nonce = hex::decode(tv.5).unwrap().try_into().unwrap();
            let expected_data = tv.6;

            let data = SecretEnvelope::seal_opts(
                &sk1.public_key(),
                key,
                value,
                SealOptions {
                    context,
                    sk: Some(sk2),
                    nonce: Some(nonce),
                },
            );
            assert_eq!(expected_data, &hex::encode(&data), "test vector {idx}");

            let envelope: SecretEnvelope = cbor::from_slice(&data).unwrap();
            envelope.open_opts(&sk1, OpenOptions { context }).unwrap();
        }
    }
}
