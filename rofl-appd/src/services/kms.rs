use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde_with::serde_as;
use sp800_185::KMac;
use tokio::sync::Notify;

use oasis_runtime_sdk::{
    core::common::{
        crypto::{mrae::deoxysii, x25519},
        logger::get_logger,
    },
    crypto::signature::{ed25519, secp256k1, Signer},
    modules,
    modules::rofl::app::{client::DeriveKeyRequest, prelude::*},
};

use crate::types::SecretEnvelope;

/// A key management service.
#[async_trait]
pub trait KmsService: Send + Sync {
    /// Start the KMS service.
    async fn start(&self) -> Result<(), Error>;

    /// Waits for the service to become ready to accept requests.
    async fn wait_ready(&self) -> Result<(), Error>;

    /// Generate a key based on the passed parameters.
    async fn generate(&self, request: &GenerateRequest<'_>) -> Result<GenerateResponse, Error>;

    /// Decrypt and authenticate a secret using the secret encryption key (SEK).
    async fn open_secret(
        &self,
        request: &OpenSecretRequest<'_>,
    ) -> Result<OpenSecretResponse, Error>;
}

/// Error returned by the key management service.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,

    #[error("not initialized yet")]
    NotInitialized,

    #[error("corrupted secret")]
    CorruptedSecret,

    #[error("internal error")]
    Internal,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Kind of key to generate.
#[derive(Copy, Clone, Debug, Default, serde::Deserialize)]
pub enum KeyKind {
    #[default]
    #[serde(rename = "raw-256")]
    Raw256,

    #[serde(rename = "raw-384")]
    Raw384,

    #[serde(rename = "ed25519")]
    Ed25519,

    #[serde(rename = "secp256k1")]
    Secp256k1,
}

impl KeyKind {
    /// Kind of the key as a stable u8 encoding.
    ///
    /// This is used during key derivation so any changes will change generated keys.
    pub fn as_stable_u8(&self) -> u8 {
        match self {
            Self::Raw256 => 1,
            Self::Raw384 => 2,
            Self::Ed25519 => 3,
            Self::Secp256k1 => 4,
        }
    }
}

/// Key generation request body.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct GenerateRequest<'r> {
    /// Domain separator for deriving different keys inside the application.
    pub key_id: &'r str,
    /// Key kind.
    pub kind: KeyKind,
}

/// Key generation response.
#[serde_as]
#[derive(Clone, Default, serde::Serialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct GenerateResponse {
    /// Generated key.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub key: Vec<u8>,
}

/// Secret decryption and authentication request.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct OpenSecretRequest<'r> {
    /// Plain-text name associated with the secret.
    pub name: &'r str,
    /// Encrypted secret value.
    ///
    /// It is expected that the value contains a CBOR-encoded `SecretEnvelope`.
    pub value: &'r [u8],
}

/// Secret decryption and authentication response.
#[serde_as]
#[derive(Clone, Default, serde::Serialize)]
pub struct OpenSecretResponse {
    /// Decrypted plain-text name.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub name: Vec<u8>,
    /// Decrypted plain-text value.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub value: Vec<u8>,
}

/// Key identifier for the root key from which all per-app keys are derived. The root key is
/// retrieved from the Oasis runtime key manager on initialization and all subsequent keys are
/// derived from that key.
///
/// Changing this identifier will change all generated keys.
const OASIS_KMS_ROOT_KEY_ID: &[u8] = b"oasis-runtime-sdk/rofl-appd: root key v1";

struct Keys {
    root: Vec<u8>,
    sek: x25519::PrivateKey,
}

/// A key management service backed by the Oasis runtime.
pub struct OasisKmsService<A: App> {
    running: AtomicBool,
    env: Environment<A>,
    logger: slog::Logger,
    ready_notify: Notify,
    keys: Arc<Mutex<Option<Keys>>>,
}

impl<A: App> OasisKmsService<A> {
    pub fn new(env: Environment<A>) -> Self {
        Self {
            running: AtomicBool::new(false),
            env,
            logger: get_logger("appd/services/kms"),
            ready_notify: Notify::new(),
            keys: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl<A: App> KmsService for OasisKmsService<A> {
    async fn start(&self) -> Result<(), Error> {
        let is_running = self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err();
        if is_running {
            return Ok(());
        }

        slog::info!(self.logger, "starting KMS service");

        // Ensure we keep retrying until the root key is derived.
        let retry_strategy = || {
            tokio_retry::strategy::ExponentialBackoff::from_millis(4)
                .max_delay(std::time::Duration::from_millis(1000))
                .map(tokio_retry::strategy::jitter)
        };

        slog::info!(
            self.logger,
            "attempting to obtain the root key for our application"
        );

        // Generate the root key for the application and store it in memory to derive all other
        // requested keys.
        let root_key_task = tokio_retry::Retry::spawn(retry_strategy(), || {
            self.env.client().derive_key(
                self.env.signer(),
                DeriveKeyRequest {
                    key_id: OASIS_KMS_ROOT_KEY_ID.to_vec(),
                    kind: modules::rofl::types::KeyKind::EntropyV0,
                    ..Default::default()
                },
            )
        });

        // Generate the secrets encryption key (SEK) and store it in memory.
        // TODO: Consider caching key in encrypted persistent storage.
        let sek_task = tokio_retry::Retry::spawn(retry_strategy(), || {
            self.env.client().derive_key(
                self.env.identity(),
                DeriveKeyRequest {
                    key_id: modules::rofl::ROFL_KEY_ID_SEK.to_vec(),
                    kind: modules::rofl::types::KeyKind::X25519,
                    ..Default::default()
                },
            )
        });

        // Perform requests in parallel.
        let (root_key, sek) = tokio::try_join!(root_key_task, sek_task,)?;

        let sek: [u8; 32] = sek.key.try_into().map_err(|_| Error::Internal)?;
        let sek = sek.into();

        // Store the keys in memory.
        *self.keys.lock().unwrap() = Some(Keys {
            root: root_key.key,
            sek,
        });

        self.ready_notify.notify_waiters();

        slog::info!(self.logger, "KMS service initialized");

        Ok(())
    }

    async fn wait_ready(&self) -> Result<(), Error> {
        let handle = self.ready_notify.notified();

        if self.keys.lock().unwrap().is_some() {
            return Ok(());
        }

        handle.await;

        Ok(())
    }

    async fn generate(&self, request: &GenerateRequest<'_>) -> Result<GenerateResponse, Error> {
        let keys_guard = self.keys.lock().unwrap();
        let root_key = &keys_guard.as_ref().ok_or(Error::NotInitialized)?.root;

        let key = Kdf::derive_key(root_key.as_ref(), request.kind, request.key_id.as_bytes())?;

        Ok(GenerateResponse { key })
    }

    async fn open_secret(
        &self,
        request: &OpenSecretRequest<'_>,
    ) -> Result<OpenSecretResponse, Error> {
        let envelope: SecretEnvelope =
            cbor::from_slice(request.value).map_err(|_| Error::InvalidArgument)?;

        let keys_guard = self.keys.lock().unwrap();
        let sek = &keys_guard.as_ref().ok_or(Error::NotInitialized)?.sek;
        let sek = sek.clone().into(); // Fine as the clone will be zeroized on drop.

        // Name.
        let name = deoxysii::box_open(
            &envelope.nonce,
            envelope.name.clone(),
            b"name".into(), // Prevent mixing name and value.
            &envelope.pk.0,
            &sek,
        )
        .map_err(|_| Error::CorruptedSecret)?;

        // Value.
        let value = deoxysii::box_open(
            &envelope.nonce,
            envelope.value.clone(),
            b"value".into(), // Prevent mixing name and value.
            &envelope.pk.0,
            &sek,
        )
        .map_err(|_| Error::CorruptedSecret)?;

        Ok(OpenSecretResponse { name, value })
    }
}

/// Insecure mock root key used to derive keys in the mock KMS.
const INSECURE_MOCK_ROOT_KEY: &[u8] = b"oasis-runtime-sdk/rofl-appd: mock root key";

/// A mock in-memory key management service.
pub struct MockKmsService;

#[async_trait]
impl KmsService for MockKmsService {
    async fn start(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn wait_ready(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn generate(&self, request: &GenerateRequest<'_>) -> Result<GenerateResponse, Error> {
        let key = Kdf::derive_key(
            INSECURE_MOCK_ROOT_KEY,
            request.kind,
            request.key_id.as_bytes(),
        )?;

        Ok(GenerateResponse { key })
    }

    async fn open_secret(
        &self,
        _request: &OpenSecretRequest<'_>,
    ) -> Result<OpenSecretResponse, Error> {
        Err(Error::NotInitialized)
    }
}

/// Domain separation tag for deriving a key-derivation key from a shared secret.
const DERIVE_KDK_CUSTOM: &[u8] = b"oasis-runtime-sdk/rofl-appd: derive key derivation key";
/// Domain separation tag for deriving a raw-256 subkey from a key-derivation key.
const DERIVE_SUBKEY_RAW256_CUSTOM: &[u8] = b"oasis-runtime-sdk/rofl-appd: derive subkey raw-256";
/// Domain separation tag for deriving a raw-384 subkey from a key-derivation key.
const DERIVE_SUBKEY_RAW384_CUSTOM: &[u8] = b"oasis-runtime-sdk/rofl-appd: derive subkey raw-384";
/// Domain separation tag for deriving a ed25519 subkey from a key-derivation key.
const DERIVE_SUBKEY_ED25519_CUSTOM: &[u8] = b"oasis-runtime-sdk/rofl-appd: derive subkey ed25519";
/// Domain separation tag for deriving a secp256k1 subkey from a key-derivation key.
const DERIVE_SUBKEY_SECP256K1_CUSTOM: &[u8] =
    b"oasis-runtime-sdk/rofl-appd: derive subkey secp256k1";

/// Key derivation function which derives keys from a root secret.
struct Kdf;

impl Kdf {
    /// Derive a key of the given kind from a root key and key identifier.
    fn derive_key(root_key: &[u8], kind: KeyKind, key_id: &[u8]) -> Result<Vec<u8>, Error> {
        let key_id = Self::generate_key_id(kind, key_id);

        // Generate a 256-bit key-derivation key.
        let mut entropy = vec![0; 32];
        Kdf::extract_randomness(root_key, &key_id, DERIVE_KDK_CUSTOM, &mut entropy);

        // Generate requested subkey.
        let key = match kind {
            KeyKind::Raw256 => {
                let mut key = vec![0; 32];
                Kdf::expand_key(&entropy, &key_id, DERIVE_SUBKEY_RAW256_CUSTOM, &mut key);

                key
            }
            KeyKind::Raw384 => {
                let mut key = vec![0; 48];
                Kdf::expand_key(&entropy, &key_id, DERIVE_SUBKEY_RAW384_CUSTOM, &mut key);

                key
            }
            KeyKind::Ed25519 => {
                let mut key = vec![0; 32];
                Kdf::expand_key(&entropy, &key_id, DERIVE_SUBKEY_ED25519_CUSTOM, &mut key);

                ed25519::MemorySigner::new_from_seed(&key)
                    .map_err(|_| Error::Internal)?
                    .to_bytes()
            }
            KeyKind::Secp256k1 => {
                let mut key = vec![0; 32];
                Kdf::expand_key(&entropy, &key_id, DERIVE_SUBKEY_SECP256K1_CUSTOM, &mut key);

                secp256k1::MemorySigner::new_from_seed(&key)
                    .map_err(|_| Error::Internal)?
                    .to_bytes()
            }
        };

        Ok(key)
    }

    /// Generate the key identifier as:
    ///
    /// ```text
    ///     key_id = u8(request.kind) || request.key_id
    /// ```
    fn generate_key_id(kind: KeyKind, key_id: &[u8]) -> Vec<u8> {
        [&[kind.as_stable_u8()], key_id].concat()
    }

    /// Derives secret keying material from a shared secret established during
    /// a key-establishment scheme using KMAC256 as the key-derivation method.
    ///
    /// ```text
    ///     keying_material = KMAC256(salt, secret, length, custom)
    /// ```
    ///
    /// The output produced by this method shall only be used as secret keying
    /// material – such as a symmetric key used for data encryption or message
    /// integrity, a secret initialization vector, or, perhaps, a key-derivation
    /// key that will be used to generate additional keying material.
    ///
    /// For more details, see: NIST SP 800-56Cr2.
    fn extract_randomness(secret: &[u8], salt: &[u8], custom: &[u8], buf: &mut [u8]) {
        let mut kmac = KMac::new_kmac256(salt, custom);
        kmac.update(secret);
        kmac.finalize(buf);
    }

    /// Derives secret keying material from a key-derivation key using KMAC256
    /// as the pseudo-random function.
    ///
    /// ```text
    ///     keying_material = KMAC256(key, salt, length, custom)
    /// ```
    /// The derived keying material may subsequently be segmented into multiple
    /// disjoint (i.e., non-overlapping) keys.
    ///
    /// For more details, see: NIST SP 800-108r1-upd1.
    fn expand_key(key: &[u8], salt: &[u8], custom: &[u8], buf: &mut [u8]) {
        let mut kmac = KMac::new_kmac256(key, custom);
        kmac.update(salt);
        kmac.finalize(buf);
    }
}

#[cfg(test)]
mod test {
    use rustc_hex::ToHex;

    use super::*;

    const KEY: &[u8] = b"key";
    const SALT: &[u8] = b"salt";
    const CUSTOM: &[u8] = b"custom";
    const SECRET: &[u8] = b"secret";
    const KEY_ID: &[u8] = b"key id";

    #[test]
    fn test_derive_key_consistency() {
        // Different kinds.
        let tcs = [
            (KeyKind::Raw256, "1cba05338c243901f5784a57746ad972e4aa609ab632b3431d85cb8c2f1d72b3"),
            (KeyKind::Raw384, "89b9c608809516f749489e2c78d98e4cddda04c7a5695dbf212036aeb80ed2c193f6dccd74745dc2fc0e1799073ce9cf"),
            (KeyKind::Ed25519, "f5d58be0f6df3b91e8c1e918d71c0cf717a4efc35790c9534b58f8c3b2b6163f"),
            (KeyKind::Secp256k1, "97d87b946892a8cfbc2773f13f89a98ef1db9fa15ef799ee9e9841a6f7f28842"),
        ];
        for tc in tcs {
            let key = Kdf::derive_key(KEY, tc.0, KEY_ID).unwrap();
            assert_eq!(key.to_hex::<String>(), tc.1,);
        }

        // Different root keys.
        let tcs = [
            (
                KEY,
                "1cba05338c243901f5784a57746ad972e4aa609ab632b3431d85cb8c2f1d72b3",
            ),
            (
                b"root key 2",
                "1b381b7f55e71d60807b799b8bca726e9a00a13f7c49e0496b84458c28273741",
            ),
            (
                b"root key 3",
                "a9344d82255c7f29f83030d49db678021381197ae0c252e8d564cd903c9c66f4",
            ),
        ];
        for tc in tcs {
            let key = Kdf::derive_key(tc.0, KeyKind::Raw256, KEY_ID).unwrap();
            assert_eq!(key.to_hex::<String>(), tc.1,);
        }

        // Different key IDs.
        let tcs = [
            (
                KEY_ID,
                "1cba05338c243901f5784a57746ad972e4aa609ab632b3431d85cb8c2f1d72b3",
            ),
            (
                b"key id 2",
                "7c042ebcd9f154191e8be9e75b32f81a9dfab48bf2b776db63ae1f8d7304eef6",
            ),
            (
                b"key id 3",
                "59939554aa7768e01eb786903856803ab48b0c582d66a97e2a33ca369bbf8c6f",
            ),
        ];
        for tc in tcs {
            let key = Kdf::derive_key(KEY, KeyKind::Raw256, tc.0).unwrap();
            assert_eq!(key.to_hex::<String>(), tc.1,);
        }
    }

    #[test]
    fn test_extract_randomness_consistency() {
        let mut buf = [0u8; 32];
        Kdf::extract_randomness(SECRET, SALT, CUSTOM, &mut buf);

        assert_eq!(
            buf.to_hex::<String>(),
            "33a1d8b06e8c08c762caf2abc6a6ccdaad9b1bab4f89bead69de66d3e165612c"
        );
    }

    #[test]
    fn test_expand_key_consistency() {
        let mut buf = [0u8; 32];
        Kdf::expand_key(KEY, SALT, CUSTOM, &mut buf);

        assert_eq!(
            buf.to_hex::<String>(),
            "c80574bfd7c5a3f5234c2cf7b72ac457204ee6cf9f75c1e15ce3a6d992a11d29"
        );
    }
}
