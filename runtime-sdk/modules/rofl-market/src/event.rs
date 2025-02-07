use oasis_runtime_sdk::types::address::Address;

use super::{types::InstanceId, MODULE_NAME};

/// Events emitted by the ROFL market module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    ProviderCreated { address: Address },

    #[sdk_event(code = 2)]
    ProviderUpdated { address: Address },

    #[sdk_event(code = 3)]
    ProviderRemoved { address: Address },

    #[sdk_event(code = 4)]
    InstanceCreated { provider: Address, id: InstanceId },

    #[sdk_event(code = 5)]
    InstanceUpdated { provider: Address, id: InstanceId },

    #[sdk_event(code = 6)]
    InstanceAccepted { provider: Address, id: InstanceId },

    #[sdk_event(code = 7)]
    InstanceCancelled { provider: Address, id: InstanceId },

    #[sdk_event(code = 8)]
    InstanceRemoved { provider: Address, id: InstanceId },

    #[sdk_event(code = 9)]
    InstanceCommandQueued { provider: Address, id: InstanceId },
}
