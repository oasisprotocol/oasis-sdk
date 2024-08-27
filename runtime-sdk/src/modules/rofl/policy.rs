use crate::core::{
    common::{
        crypto::signature::PublicKey,
        sgx::{EnclaveIdentity, QuotePolicy},
    },
    consensus::beacon::EpochTime,
};

/// Per-application ROFL policy.
#[derive(Clone, Debug, PartialEq, Eq, Default, cbor::Encode, cbor::Decode)]
pub struct AppAuthPolicy {
    /// Quote policy.
    pub quotes: QuotePolicy,
    /// The set of allowed enclave identities.
    pub enclaves: Vec<EnclaveIdentity>,
    /// The set of allowed endorsements.
    pub endorsements: Vec<AllowedEndorsement>,
    /// Gas fee payment policy.
    pub fees: FeePolicy,
    /// Maximum number of future epochs for which one can register.
    pub max_expiration: EpochTime,
}

/// An allowed endorsement policy.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub enum AllowedEndorsement {
    /// Any node can endorse the enclave.
    #[cbor(rename = "any", as_struct)]
    Any,
    /// Compute node for the current runtime can endorse the enclave.
    #[cbor(rename = "role_compute", as_struct)]
    ComputeRole,
    /// Observer node for the current runtime can endorse the enclave.
    #[cbor(rename = "role_observer", as_struct)]
    ObserverRole,
    /// Registered node from a specific entity can endorse the enclave.
    #[cbor(rename = "entity")]
    Entity(PublicKey),
    /// Specific node can endorse the enclave.
    #[cbor(rename = "node")]
    Node(PublicKey),
}

/// Gas fee payment policy.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum FeePolicy {
    /// Application enclave pays the gas fees.
    InstancePays = 1,
    /// Endorsing node pays the gas fees.
    #[default]
    EndorsingNodePays = 2,
}
