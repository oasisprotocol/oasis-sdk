//! Events.

/// An event emitted from the contract.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct Event {
    /// Optional module name.
    #[cbor(optional)]
    pub module: String,

    /// Unique code representing the event for the given module.
    pub code: u32,

    /// Arbitrary data associated with the event.
    #[cbor(optional)]
    pub data: Vec<u8>,
}
