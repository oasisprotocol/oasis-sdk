use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Key in the versions map used for the global state version.
pub const VERSION_GLOBAL_KEY: &str = "";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// A set of state versions for all supported modules.
    #[serde(rename = "versions")]
    pub versions: BTreeMap<String, u32>,
}
