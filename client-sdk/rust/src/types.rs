use oasis_runtime_sdk::core::{common::crypto::hash::Hash, consensus::roothash::Block};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Numbered(u64),
    Latest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxResult {
    /// The block containing the transaction.
    pub block: Block,

    /// The index of the transaction in the block.
    pub index: u32,

    /// The raw transaction input.
    pub input: Vec<u8>,

    /// The raw transaction output.
    pub output: Vec<u8>,
}

impl From<crate::requests::TxResult> for TxResult {
    fn from(r: crate::requests::TxResult) -> Self {
        Self {
            block: r.block,
            index: r.index,
            input: r.input.to_vec(),
            output: r.output.to_vec(),
        }
    }
}

impl From<crate::requests::Tag> for oasis_runtime_sdk::core::transaction::tags::Tag {
    fn from(tag: crate::requests::Tag) -> Self {
        Self {
            key: tag.key.to_vec(),
            value: tag.value.to_vec(),
            tx_hash: Hash(tag.tx_hash),
        }
    }
}
