use std::collections::BTreeMap;

use crate::keymanager::SignedPublicKey;

/// Key in the versions map used for the global state version.
pub const VERSION_GLOBAL_KEY: &str = "";

/// Per-module metadata.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Metadata {
    /// A set of state versions for all supported modules.
    pub versions: BTreeMap<String, u32>,
}

/// Response to the call data public key query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct CallDataPublicKeyQueryResponse {
    /// Public key used for deriving the shared secret for encrypting call data.
    pub public_key: SignedPublicKey,
}
