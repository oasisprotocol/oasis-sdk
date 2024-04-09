use crate::{
    core::{
        common::crypto::{signature, x25519},
        consensus::{beacon::EpochTime, registry},
    },
    crypto::signature::PublicKey,
};

/// Register ROFL call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Register {
    /// ROFL application identifier.
    pub app: String,
    /// Endorsed TEE capability.
    pub ect: registry::EndorsedCapabilityTEE,
    /// Epoch when the ROFL registration expires if not renewed.
    pub expiration: EpochTime,
    /// Extra public keys to endorse (e.g. secp256k1 keys).
    pub extra_keys: Vec<PublicKey>,
}

/// ROFL registration descriptor.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Registration {
    /// Application this enclave is registered for.
    pub app: String,
    /// Identifier of the endorsing node.
    pub node_id: signature::PublicKey,
    /// Runtime Attestation Key.
    pub rak: signature::PublicKey,
    /// Runtime Encryption Key.
    pub rek: x25519::PublicKey,
    /// Epoch when the ROFL registration expires if not renewed.
    pub expiration: EpochTime,
    /// Extra public keys to endorse (e.g. secp256k1 keys).
    pub extra_keys: Vec<PublicKey>,
}
