//! Types related to schedule control.
use oasis_core_runtime::{
    common::crypto::hash::Hash, transaction::types::TxnBatch, types::Body, Protocol,
};

/// Unique module name.
const MODULE_NAME: &str = "schedule_control";

/// Schedule control errors.
#[derive(Debug, thiserror::Error, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("failed to fetch batch from host")]
    #[sdk_error(code = 1)]
    FailedToFetchBatch,
}

/// Interface to the runtime host that supports schedule control features.
pub trait ScheduleControlHost: Send + Sync {
    /// Fetch the specified set of transactions from the host's transaction queue.
    ///
    /// Offset specifies the transaction hash that should serve as an offset when returning
    /// transactions from the pool. Transactions will be skipped until the given hash is encountered
    /// and only following transactions will be returned.
    fn fetch_tx_batch(&self, offset: Option<Hash>, limit: u32) -> Result<Option<TxnBatch>, Error>;
}

impl ScheduleControlHost for Protocol {
    fn fetch_tx_batch(&self, offset: Option<Hash>, limit: u32) -> Result<Option<TxnBatch>, Error> {
        match self.call_host(
            io_context::Context::background(),
            Body::HostFetchTxBatchRequest { offset, limit },
        ) {
            Ok(Body::HostFetchTxBatchResponse { batch }) => Ok(batch),
            _ => Err(Error::FailedToFetchBatch),
        }
    }
}
