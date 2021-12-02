//! Event types for runtimes.
use std::collections::BTreeMap;

use oasis_core_runtime::transaction::tags::{Tag, Tags};

/// An event emitted by the runtime.
///
/// This trait can be derived:
/// ```
/// # #[cfg(feature = "oasis-runtime-sdk-macros")]
/// # mod example {
/// # use oasis_runtime_sdk_macros::Event;
/// const MODULE_NAME: &str = "my-module";
/// #[derive(Clone, Debug, cbor::Encode, Event)]
/// #[cbor(untagged)]
/// #[sdk_event(autonumber)] // `module_name` meta is required if `MODULE_NAME` isn't in scope
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
    fn module_name() -> &'static str;

    /// Code uniquely identifying the event.
    fn code(&self) -> u32;

    /// Converts an event into an event tag.
    ///
    /// # Key
    ///
    /// ```text
    /// <module (variable size bytes)> <code (big-endian u32)>
    /// ```
    ///
    /// # Value
    ///
    /// CBOR-serialized event value.
    ///
    fn into_event_tag(self) -> EventTag {
        etag_for_event(Self::module_name(), self.code(), cbor::to_value(self))
    }
}

impl Event for () {
    fn module_name() -> &'static str {
        "(none)"
    }

    fn code(&self) -> u32 {
        Default::default()
    }
}

/// Generate an EventTag corresponding to the passed event triple.
pub fn etag_for_event(module_name: &str, code: u32, value: cbor::Value) -> EventTag {
    EventTag {
        key: [module_name.as_bytes(), &code.to_be_bytes()]
            .concat()
            .to_vec(),
        value,
    }
}

/// A key-value pair representing an emitted event that will be emitted as a tag.
#[derive(Clone, Debug)]
pub struct EventTag {
    pub key: Vec<u8>,
    pub value: cbor::Value,
}

/// Event tags with values accumulated by key.
pub type EventTags = BTreeMap<Vec<u8>, Vec<cbor::Value>>;

/// Provides method for converting event tags into events.
pub trait IntoTags {
    fn into_tags(self) -> Tags;
}

impl IntoTags for EventTags {
    fn into_tags(self) -> Tags {
        self.into_iter()
            .map(|(k, v)| Tag::new(k, cbor::to_vec(v)))
            .collect()
    }
}
