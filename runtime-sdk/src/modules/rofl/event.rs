use crate::crypto::signature::PublicKey;

use super::{app_id::AppId, MODULE_NAME};

/// Events emitted by the ROFL module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    AppCreated { id: AppId },

    #[sdk_event(code = 2)]
    AppUpdated { id: AppId },

    #[sdk_event(code = 3)]
    AppRemoved { id: AppId },

    #[sdk_event(code = 4)]
    InstanceRegistered { app_id: AppId, rak: PublicKey },
}
