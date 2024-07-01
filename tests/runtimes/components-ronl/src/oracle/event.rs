use super::{types, MODULE_NAME};

/// Events emitted by the oracle module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    ValueUpdated(Option<types::Observation>),
}
