//! Mock dispatch context for use in tests.
use io_context::Context as IoContext;

use oasis_core_runtime::{
    common::{cbor, version::Version},
    consensus::{beacon, roothash, state::ConsensusState},
    storage::mkvs,
};

use crate::{
    context::{Mode, RuntimeBatchContext},
    module::MigrationHandler,
    modules,
    runtime::Runtime,
    storage,
    types::transaction,
};

/// A mock runtime that only has the core module.
pub struct EmptyRuntime;

impl Runtime for EmptyRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Modules = modules::core::Module;

    fn genesis_state() -> <Self::Modules as MigrationHandler>::Genesis {
        Default::default()
    }
}

/// Mock dispatch context factory.
pub struct Mock {
    pub runtime_header: roothash::Header,
    pub runtime_round_results: roothash::RoundResults,
    pub mkvs: Box<dyn mkvs::MKVS>,
    pub consensus_state: ConsensusState,
    pub epoch: beacon::EpochTime,

    pub max_messages: u32,
}

impl Mock {
    /// Create a new mock dispatch context.
    pub fn create_ctx(
        &mut self,
    ) -> RuntimeBatchContext<'_, EmptyRuntime, storage::MKVSStore<&mut dyn mkvs::MKVS>> {
        self.create_ctx_for_runtime(Mode::ExecuteTx)
    }

    /// Create a new mock dispatch context.
    pub fn create_ctx_for_runtime<R: Runtime>(
        &mut self,
        mode: Mode,
    ) -> RuntimeBatchContext<'_, R, storage::MKVSStore<&mut dyn mkvs::MKVS>> {
        RuntimeBatchContext::new(
            mode,
            &self.runtime_header,
            &self.runtime_round_results,
            storage::MKVSStore::new(IoContext::background().freeze(), self.mkvs.as_mut()),
            &self.consensus_state,
            self.epoch,
            IoContext::background().freeze(),
            self.max_messages,
        )
    }
}

impl Default for Mock {
    fn default() -> Self {
        let mkvs = mkvs::OverlayTree::new(
            mkvs::Tree::make()
                .with_root_type(mkvs::RootType::State)
                .new(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let consensus_tree = mkvs::Tree::make()
            .with_root_type(mkvs::RootType::State)
            .new(Box::new(mkvs::sync::NoopReadSyncer));

        Self {
            runtime_header: roothash::Header::default(),
            runtime_round_results: roothash::RoundResults::default(),
            mkvs: Box::new(mkvs),
            consensus_state: ConsensusState::new(consensus_tree),
            epoch: 1,
            max_messages: 32,
        }
    }
}

/// Create a new mock transaction.
pub fn transaction() -> transaction::Transaction {
    transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: "mock".to_owned(),
            body: cbor::Value::Null,
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 0,
            },
        },
    }
}
