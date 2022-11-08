//! Transaction sender metadata.
use crate::types::address::Address;

/// Transaction sender metadata.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SenderMeta {
    /// Sender address.
    pub address: Address,
    /// Sender nonce contained in the transaction.
    pub tx_nonce: u64,
    /// Sender nonce contained in runtime state.
    pub state_nonce: u64,
}

impl SenderMeta {
    /// Unique identifier of the sender, currently derived from the sender address.
    pub fn id(&self) -> Vec<u8> {
        if self.address == Default::default() {
            // Use an empty value for the default address as that signals to the host that the
            // sender should be ignored.
            vec![]
        } else {
            self.address.into_bytes().to_vec()
        }
    }
}
