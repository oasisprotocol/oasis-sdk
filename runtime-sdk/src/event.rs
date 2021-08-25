//! Event types for runtimes.
use oasis_core_runtime::{common::cbor, transaction::tags::Tag};

/// An event emitted by the runtime.
///
/// This trait can be derived:
/// ```
/// # #[cfg(feature = "oasis-runtime-sdk-macros")]
/// # mod example {
/// # use serde::{Serialize, Deserialize};
/// # use oasis_runtime_sdk_macros::Event;
/// const MODULE_NAME: &str = "my-module";
/// #[derive(Clone, Debug, Serialize, Deserialize, Event)]
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
pub trait Event: Sized {
    /// Name of the module that emitted the event.
    fn module_name() -> &'static str;

    /// Returns whether this event has a variant with the provided code.
    fn has_variant_with_code(code: u32) -> bool;

    /// Code uniquely identifying the event.
    fn code(&self) -> u32;

    /// Serialized event value.
    fn value(&self) -> cbor::Value;

    /// Deserializes the [`Event`] from the provided [`cbor::Value`].
    fn from_value(value: cbor::Value) -> Result<Self, cbor::Error>;

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
    fn to_tag(&self) -> Tag {
        Tag::new(
            [Self::module_name().as_bytes(), &self.code().to_be_bytes()]
                .concat()
                .to_vec(),
            cbor::to_vec(&self.value()),
        )
    }

    /// Converts an emitted tag back into an event.
    fn from_tag(tag: &Tag) -> Result<Self, EventDecodeError> {
        let module_name = Self::module_name();

        if tag.key.len() != module_name.len() + std::mem::size_of::<u32>() {
            return Err(EventDecodeError::KeyFormat);
        }

        let (tag_module_name_bytes, tag_code_bytes) = tag.key.split_at(module_name.len());
        let tag_module_name =
            std::str::from_utf8(tag_module_name_bytes).map_err(|_| EventDecodeError::KeyFormat)?;

        if tag_module_name != Self::module_name() {
            return Err(EventDecodeError::WrongModule {
                expected: Self::module_name(),
                actual: tag_module_name.into(),
            });
        }

        let mut tag_code_be_arr = [0u8; std::mem::size_of::<u32>()];
        tag_code_be_arr.copy_from_slice(tag_code_bytes);
        let tag_code = u32::from_be_bytes(tag_code_be_arr);
        if !Self::has_variant_with_code(tag_code) {
            return Err(EventDecodeError::UnknownCode(tag_code));
        }

        Ok(Self::from_value(cbor::from_slice(&tag.value)?)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventDecodeError {
    #[error("tag key does not follow the SDK event forat")]
    KeyFormat,

    #[error("mismatched module name: expected {expected}, but found {actual}")]
    WrongModule {
        expected: &'static str,
        actual: String,
    },

    #[error("event does not have code {0}")]
    UnknownCode(u32),

    #[error("event deserialize error: {0}")]
    Deserialize(#[from] cbor::Error),
}

impl Event for () {
    fn module_name() -> &'static str {
        "(none)"
    }

    fn code(&self) -> u32 {
        0
    }

    fn has_variant_with_code(code: u32) -> bool {
        code == 0
    }

    fn value(&self) -> cbor::Value {
        cbor::Value::Null
    }

    fn from_value(value: cbor::Value) -> Result<Self, cbor::Error> {
        cbor::from_value(value)
    }
}
