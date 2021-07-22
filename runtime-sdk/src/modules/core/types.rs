use std::collections::BTreeMap;

/// Key in the versions map used for the global state version.
pub const VERSION_GLOBAL_KEY: &str = "";

#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Metadata {
    /// A set of state versions for all supported modules.
    pub versions: BTreeMap<String, u32>,
}
