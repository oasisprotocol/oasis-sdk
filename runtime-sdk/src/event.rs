//! Event types for runtimes.
use oasis_core_runtime::{common::cbor, transaction::tags::Tag};

/// An event emitted by the runtime.
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
