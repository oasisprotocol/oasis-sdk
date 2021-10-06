//! Types for the keyvalue module.

use oasis_runtime_sdk::crypto::signature::{ed25519, Signature};

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Key {
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct KeyValue {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct ConfidentialKey {
    pub key_id: Vec<u8>,
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct ConfidentialKeyValue {
    pub key_id: Vec<u8>,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct SpecialGreetingParams {
    pub nonce: u64,
    pub greeting: String,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct SpecialGreeting {
    pub params_cbor: Vec<u8>,
    pub from: ed25519::PublicKey,
    pub signature: Signature,
}
