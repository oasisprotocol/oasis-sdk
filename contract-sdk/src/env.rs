//! Smart contract environment query interface.
use oasis_contract_sdk_types::address::Address;

use crate::types::{
    env::{QueryRequest, QueryResponse},
    InstanceId,
};

/// Environment query trait.
pub trait Env {
    /// Perform an environment query.
    fn query<Q: Into<QueryRequest>>(&self, query: Q) -> QueryResponse;

    /// Returns an address for the contract instance id.
    fn address_for_instance(&self, instance_id: InstanceId) -> Address;
}

/// Crypto helpers trait.
pub trait Crypto {
    /// ECDSA public key recovery function.
    fn ecdsa_recover(&self, input: &[u8]) -> [u8; 65];
}
