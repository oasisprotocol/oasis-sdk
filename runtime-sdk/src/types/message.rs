use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use oasis_core_runtime::{common::cbor, consensus};

/// Result of a message being processed by the consensus layer.
pub type MessageEvent = consensus::roothash::MessageEvent;

/// Handler name and context to be called after message is executed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageEventHookInvocation {
    pub hook_name: String,
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
