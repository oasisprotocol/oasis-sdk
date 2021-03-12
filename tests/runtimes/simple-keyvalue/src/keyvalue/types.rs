use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    #[serde(rename = "key")]
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyValue {
    #[serde(rename = "key")]
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,

    #[serde(rename = "value")]
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}
