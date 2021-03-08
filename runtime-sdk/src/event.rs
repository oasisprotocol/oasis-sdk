//! Event types for runtimes.
use oasis_core_runtime::{common::cbor, transaction::tags::Tag};

/// An event emitted by the runtime.
///
/// This trait can be derived when using `features = ["oasis-sdk-runtime-macros"]`:
/// ```no_run
/// # #[cfg(feature = "oasis-sdk-runtime-macros")]
/// # mod example {
/// # use serde::{Serialize, Deserialize};
/// # use oasis_sdk_runtime_macros::Event;
/// #[derive(Clone, Debug, Serialize, Deserialize, Event)]
/// #[event(module = "path::to::MyModule", autonumber)] // `module` is required
/// enum MyEvent {
///    Greeting(String),  // autonumbered to id 0
///    #[event(id = 42)]  // manually numbered to id 42 (`id` is required if not autonumbering)
///    DontPanic,
///    Salutation {       // autonumbered to id 1
///        plural: bool,
///    }
/// }
/// # }
/// ```
pub trait Event {
    /// Name of the module that emitted the event.
    fn module(&self) -> &str;

    /// Code uniquely identifying the event.
    fn code(&self) -> u32;

    /// Serialized event value.
    fn value(&self) -> cbor::Value;

    /// Converts an emitted event into a tag that can be emitted by the runtime.
    ///
    /// # Key
    ///
    /// ```ignore
    /// <module (variable size bytes)> <code (big-endian u32)>
    /// ```
    ///
    /// # Value
    ///
    /// CBOR-serialized event value.
    ///
    fn to_tag(&self) -> Tag {
        Tag::new(
            [self.module().as_bytes(), &self.code().to_be_bytes()]
                .concat()
                .to_vec(),
            cbor::to_vec(&self.value()),
        )
    }
}
