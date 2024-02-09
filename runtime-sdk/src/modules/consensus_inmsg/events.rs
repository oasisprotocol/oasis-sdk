use super::MODULE_NAME;
use crate::error;

/// Events emitted by the consensus incoming message handler module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk_macros::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Processed {
        id: u64,
        #[cbor(optional)]
        tag: u64,
        #[cbor(optional)]
        error: Option<error::SerializableError>,
    },
}
