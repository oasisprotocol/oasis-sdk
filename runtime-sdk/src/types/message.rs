use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use oasis_core_runtime::{common::cbor, consensus};

/// Result of a message being processed by the consensus layer.
pub type MessageEvent = consensus::roothash::MessageEvent;

/// Handler name and context to be called after message is executed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageEventHookInvocation {
    #[serde(rename = "hook_name")]
    pub hook_name: String,

    #[serde(rename = "payload")]
    pub payload: cbor::Value,
}

impl MessageEventHookInvocation {
    /// Constructs a new message hook invocation.
    pub fn new<S: Serialize>(name: String, payload: S) -> Self {
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
