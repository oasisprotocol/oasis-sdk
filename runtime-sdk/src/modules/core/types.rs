use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// A set of state versions for all supported modules.
    #[serde(rename = "versions")]
    pub versions: BTreeMap<String, u32>,
}
