use crate::InstanceId;

/// Instantiate call result.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct InstantiateResult {
    /// Assigned instance identifier.
    pub id: InstanceId,
}
