use super::{types::InstanceId, MODULE_NAME};
use crate::types::address::Address;

/// Events emitted by the ROFL market module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    ProviderCreated { address: Address },

    #[sdk_event(code = 2)]
    ProviderUpdated { address: Address },

    #[sdk_event(code = 3)]
    ProviderRemoved { address: Address },

    #[sdk_event(code = 4)]
    InstanceCreated { id: InstanceId, provider: Address },
}
