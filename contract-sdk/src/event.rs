//! Contract event trait.
use crate::types;

/// An event emitted by the contract.
///
/// This trait can be derived:
/// ```
/// # #[cfg(feature = "oasis-contract-sdk-macros")]
/// # mod example {
/// # use oasis_contract_sdk_macros::Event;
/// #[derive(Clone, Debug, cbor::Encode, Event)]
/// #[cbor(untagged)]
/// #[sdk_event(autonumber)]
/// enum MyEvent {
///    Greeting(String),      // autonumbered to 0
///    #[sdk_event(code = 2)] // manually numbered to 2 (`code` is required if not autonumbering)
///    DontPanic,
///    Salutation {           // autonumbered to 1
///        plural: bool,
///    }
/// }
/// # }
/// ```
pub trait Event: Sized + cbor::Encode {
    /// Name of the module that emitted the event.
    fn module_name(&self) -> &str;

    /// Code uniquely identifying the event.
    fn code(&self) -> u32;

    /// Converts an event into the raw event type that can be emitted from the contract.
    fn into_raw(self) -> types::event::Event {
        types::event::Event {
            module: self.module_name().to_string(),
            code: self.code(),
            data: cbor::to_vec(self),
        }
    }
}

impl Event for types::event::Event {
    fn module_name(&self) -> &str {
        &self.module
    }

    fn code(&self) -> u32 {
        self.code
    }

    fn into_raw(self) -> types::event::Event {
        self
    }
}
