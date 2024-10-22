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
}

/// Remove an existing ROFL application call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Remove {
    /// ROFL application identifier.
    pub id: AppId,
}

/// ROFL application configuration.
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
