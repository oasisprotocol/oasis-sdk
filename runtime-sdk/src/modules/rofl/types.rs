use std::collections::BTreeMap;

use crate::{
    core::{
        common::crypto::{signature, x25519},
        consensus::{beacon::EpochTime, registry},
    },
    crypto::signature::PublicKey,
    types::{address::Address, token},
};

use super::{app_id::AppId, policy::AppAuthPolicy};

/// Create new ROFL application call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Create {
    /// Application authentication policy.
    pub policy: AppAuthPolicy,
    /// Identifier generation scheme.
    pub scheme: IdentifierScheme,
    /// Metadata (arbitrary key/value pairs).
    pub metadata: BTreeMap<String, String>,
    // Note that we cannot pass secrets here as the SEK is not yet available.
}

/// ROFL application identifier generation scheme.
#[derive(Clone, Copy, Debug, Default, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum IdentifierScheme {
    #[default]
    CreatorRoundIndex = 0,
    CreatorNonce = 1,
}

/// Update an existing ROFL application call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Update {
    /// ROFL application identifier.
    pub id: AppId,
    /// Authentication policy.
    pub policy: AppAuthPolicy,
    /// Application administrator address.
    pub admin: Option<Address>,

    /// Metadata (arbitrary key/value pairs).
    pub metadata: BTreeMap<String, String>,
    /// Secrets (arbitrary encrypted key/value pairs).
    pub secrets: BTreeMap<String, Vec<u8>>,
}

/// Remove an existing ROFL application call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Remove {
    /// ROFL application identifier.
    pub id: AppId,
}

/// ROFL application configuration.
///
/// # Metadata
///
/// Metadata contains arbitrary key-value pairs.
///
/// # Secrets
///
/// In addition to metadata, the configuration can also contain secrets which are encrypted with a
/// shared secret derived from the secret encryption key (SEK). Since the SEK is only available once
/// the application has been registered, the initial create cannot contain secrets.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AppConfig {
    /// ROFL application identifier.
    pub id: AppId,
    /// Authentication policy.
    pub policy: AppAuthPolicy,
    /// Application administrator address.
    pub admin: Option<Address>,
    /// Staked amount.
    pub stake: token::BaseUnits,

    /// Metadata (arbitrary key/value pairs).
    pub metadata: BTreeMap<String, String>,
    /// Secrets (arbitrary encrypted key/value pairs).
    pub secrets: BTreeMap<String, Vec<u8>>,
    /// Secret encryption public key. The key is used to derive a shared secret used for symmetric
    /// encryption (e.g. using Deoxys-II or similar).
    pub sek: x25519::PublicKey,
}

/// Register ROFL call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Register {
    /// ROFL application identifier.
    pub app: AppId,
    /// Endorsed TEE capability.
    pub ect: registry::EndorsedCapabilityTEE,
    /// Epoch when the ROFL registration expires if not renewed.
    pub expiration: EpochTime,
    /// Extra public keys to endorse (e.g. secp256k1 keys).
    ///
    /// All of these keys need to co-sign the registration transaction to prove ownership.
    pub extra_keys: Vec<PublicKey>,
    /// Arbitrary app-specific metadata.
    #[cbor(optional)]
    pub metadata: BTreeMap<String, String>,
}

/// Kind of key for derivation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum KeyKind {
    /// Raw entropy derivation.
    #[default]
    EntropyV0 = 0,

    /// X25519 key pair.
    X25519 = 1,
}

/// Scope of key for derivation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[cbor(with_default)]
#[repr(u8)]
pub enum KeyScope {
    /// Global application scope (e.g. all instances get the same key).
    #[default]
    Global = 0,

    /// Node scope (e.g. all instances endorsed by the same node get the same key).
    Node = 1,

    /// Entity scope (e.g. all instances endorsed by nodes from the same entity get the same key).
    Entity = 2,
}

impl KeyScope {
    /// Whether this key scope is the global key scope.
    pub fn is_global(&self) -> bool {
        matches!(self, Self::Global)
    }
}

/// Derive key call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DeriveKey {
    /// ROFL application identifier.
    pub app: AppId,
    /// Key kind.
    pub kind: KeyKind,
    /// Key scope.
    #[cbor(optional, skip_serializing_if = "KeyScope::is_global")]
    pub scope: KeyScope,
    /// Key generation.
    pub generation: u64,
    /// Key identifier.
    pub key_id: Vec<u8>,
}

/// Response from the derive key call.
#[derive(Clone, Default, cbor::Encode, cbor::Decode)]
pub struct DeriveKeyResponse {
    /// Derived key.
    pub key: Vec<u8>,
}

/// ROFL registration descriptor.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Registration {
    /// Application this enclave is registered for.
    pub app: AppId,
    /// Identifier of the endorsing node.
    pub node_id: signature::PublicKey,
    /// Optional identifier of the endorsing entity.
    pub entity_id: Option<signature::PublicKey>,
    /// Runtime Attestation Key.
    pub rak: signature::PublicKey,
    /// Runtime Encryption Key.
    pub rek: x25519::PublicKey,
    /// Epoch when the ROFL registration expires if not renewed.
    pub expiration: EpochTime,
    /// Extra public keys to endorse (e.g. secp256k1 keys).
    pub extra_keys: Vec<PublicKey>,
    /// Arbitrary app-specific metadata.
    #[cbor(optional)]
    pub metadata: BTreeMap<String, String>,
}

/// Application-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AppQuery {
    /// ROFL application identifier.
    pub id: AppId,
}

/// Application instance query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct AppInstanceQuery {
    /// ROFL application identifier.
    pub app: AppId,
    /// Runtime Attestation Key.
    pub rak: PublicKey,
}

/// Stake thresholds for managing ROFL.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct StakeThresholds {
    /// Required stake for creating new ROFL application.
    pub app_create: token::BaseUnits,
}
