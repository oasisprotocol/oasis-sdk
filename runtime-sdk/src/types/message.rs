use std::fmt::Debug;

use oasis_core_runtime::consensus;

/// Result of a message being processed by the consensus layer.
pub type MessageEvent = consensus::roothash::MessageEvent;

/// Handler name and context to be called after message is executed.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct MessageEventHookInvocation {
    pub hook_name: String,
    pub payload: cbor::Value,
}

impl MessageEventHookInvocation {
    /// Constructs a new message hook invocation.
    pub fn new<S: cbor::Encode>(name: String, payload: S) -> Self {
        Self {
            hook_name: name,
            payload: cbor::to_value(payload),
        }
    }
}

/// Result of a message being processed by the consensus layer combined with the context for the
/// result handler.
#[derive(Clone, Debug)]
pub struct MessageResult {
    pub event: MessageEvent,
    pub context: cbor::Value,
}
