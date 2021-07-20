//! Events.

/// An event emitted from the contract.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Event {
    /// Optional module name.
    #[cbor(optional, default, skip_serializing_if = "String::is_empty")]
    pub module: String,

    /// Unique code representing the event for the given module.
    pub code: u32,

    /// Arbitrary data associated with the event.
    #[cbor(optional, default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
}
