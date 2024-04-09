use std::collections::BTreeMap;

use crate::core::common::{
    crypto::signature::PublicKey,
    sgx::{EnclaveIdentity, QuotePolicy},
};

/// Authorization policy for ROFL applications.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AuthPolicy {
    /// Per-application policies.
    pub apps: BTreeMap<String, AppAuthPolicy>,
}

/// Per-application ROFL policy.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AppAuthPolicy {
    /// Quote policy.
    pub quotes: QuotePolicy,
    /// The set of allowed enclave identities.
    pub enclaves: Vec<EnclaveIdentity>,
    /// The set of allowed endorsements.
    pub endorsement: Vec<AllowedEndorsement>,
    /// Gas fee payement policy.
    pub fees: FeePolicy,
    /// Maximum number of future epochs for which one can register.
    pub max_expiration: u64,
    /// Priority of transactions from the ROFL application.
    pub priority: Option<u64>,
}

/// An allowed endorsement policy.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub enum AllowedEndorsement {
    /// Any registered node can endorse the enclave.
    #[cbor(rename = "any")]
    Any,
    /// Compute node can endorse the enclave.
    #[cbor(rename = "role_compute")]
    ComputeRole,
    /// Observer node can endorse the enclave.
    #[cbor(rename = "role_observer")]
    ObserverRole,
    /// Registered node from a specific entity can endorse the enclave.
    #[cbor(rename = "entity")]
    Entity(PublicKey),
}

/// Gas fee payment policy.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub enum FeePolicy {
    /// Application enclave pays the gas fees.
    #[cbor(rename = "app_pays")]
    AppPays,
    /// Endorsing node pays the gas fees.
    #[default]
    #[cbor(rename = "endorsing_node_pays")]
    EndorsingNodePays,
    /// Endorsing node pays the gas fees and is reimbursed in case of success.
    #[cbor(rename = "endorsing_node_pays_reimburse")]
    EndorsingNodePaysWithReimbursement,
}
