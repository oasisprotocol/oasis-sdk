//! Historic state access.
use oasis_core_runtime::{
    consensus::{state::ConsensusState, verifier::Verifier, Event},
    future::block_on,
    types::EventKind,
};

/// Unique module name.
const MODULE_NAME: &str = "history";

/// History host errors.
#[derive(Debug, thiserror::Error, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("failed to fetch block from host")]
    #[sdk_error(code = 1)]
    FailedToFetchBlock,

    #[error("failed to fetch events from host")]
    #[sdk_error(code = 2)]
    FailedToFetchEvents,
}

/// Interface to the runtime host to fetch historic information.
pub trait HistoryHost {
    /// Fetch historic consensus state after executing the block at given height.
    fn consensus_state_at(&self, height: u64) -> Result<ConsensusState, Error>;

    /// Fetch events emitted during execution of the block at given height.
    fn consensus_events_at(&self, height: u64, kind: EventKind) -> Result<Vec<Event>, Error>;
}

impl HistoryHost for Box<dyn HistoryHost> {
    fn consensus_state_at(&self, height: u64) -> Result<ConsensusState, Error> {
        HistoryHost::consensus_state_at(&**self, height)
    }

    fn consensus_events_at(&self, height: u64, kind: EventKind) -> Result<Vec<Event>, Error> {
        HistoryHost::consensus_events_at(&**self, height, kind)
    }
}

impl<V: Verifier> HistoryHost for V {
    fn consensus_state_at(&self, height: u64) -> Result<ConsensusState, Error> {
        block_on(self.state_at(height)).map_err(|_| Error::FailedToFetchBlock)
    }

    fn consensus_events_at(&self, height: u64, kind: EventKind) -> Result<Vec<Event>, Error> {
        block_on(self.events_at(height, kind)).map_err(|_| Error::FailedToFetchEvents)
    }
}
