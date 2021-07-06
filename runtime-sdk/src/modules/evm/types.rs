//! EVM module types.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTx {
    #[serde(rename = "value")]
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>, // U256
    #[serde(rename = "init_code")]
    #[serde(with = "serde_bytes")]
    pub init_code: Vec<u8>,
    #[serde(rename = "gas_limit")]
    pub gas_limit: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallTx {
    #[serde(rename = "address")]
    #[serde(with = "serde_bytes")]
    pub address: Vec<u8>, // H160
    #[serde(rename = "value")]
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>, // U256
    #[serde(rename = "data")]
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    #[serde(rename = "gas_limit")]
    pub gas_limit: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeekStorageQuery {
    #[serde(rename = "address")]
    #[serde(with = "serde_bytes")]
    pub address: Vec<u8>, // H160
    #[serde(rename = "index")]
    #[serde(with = "serde_bytes")]
    pub index: Vec<u8>, // H256
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeekCodeQuery {
    #[serde(rename = "address")]
    #[serde(with = "serde_bytes")]
    pub address: Vec<u8>, // H160
}
