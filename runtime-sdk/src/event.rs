//! Event types for runtimes.
use oasis_core_runtime::transaction::tags::Tag;

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
///    DontPanic,             // autonumbered to 1
///    Salutation {           // autonumbered to 3
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

    /// Converts an emitted event into a tag that can be emitted by the runtime.
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
    fn into_tag(self) -> Tag {
        Tag::new(
            [Self::module_name().as_bytes(), &self.code().to_be_bytes()]
                .concat()
                .to_vec(),
            cbor::to_vec(self),
        )
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
