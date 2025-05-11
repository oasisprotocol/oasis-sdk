//! Execution context.
use std::{collections::btree_map::BTreeMap, marker::PhantomData};

use slog::{self, o};

use oasis_core_runtime::{
    common::{logger::get_logger, namespace::Namespace},
    consensus,
    consensus::roothash,
    protocol::HostInfo,
};

use crate::{
    history,
    keymanager::KeyManager,
    module::MethodHandler as _,
    runtime,
    state::{self, CurrentState},
};

/// Local configuration key the value of which determines whether expensive queries should be
/// allowed or not, and also whether smart contracts should be simulated for `core.EstimateGas`.
/// DEPRECATED and superseded by LOCAL_CONFIG_ESTIMATE_GAS_BY_SIMULATING_CONTRACTS and LOCAL_CONFIG_ALLOWED_QUERIES.
const LOCAL_CONFIG_ALLOW_EXPENSIVE_QUERIES: &str = "allow_expensive_queries";
/// Local configuration key the value of which determines whether smart contracts should
/// be simulated when estimating gas in `core.EstimateGas`.
const LOCAL_CONFIG_ESTIMATE_GAS_BY_SIMULATING_CONTRACTS: &str =
    "estimate_gas_by_simulating_contracts";
/// Local configuration key the value of which determines the set of allowed queries.
const LOCAL_CONFIG_ALLOWED_QUERIES: &str = "allowed_queries";
/// Special key inside the `allowed_queries` list; represents the set of all queries.
const LOCAL_CONFIG_ALLOWED_QUERIES_ALL: &str = "all";
/// Special key inside the `allowed_queries` list; represents the set of all queries
/// that are tagged `expensive`.
const LOCAL_CONFIG_ALLOWED_QUERIES_ALL_EXPENSIVE: &str = "all_expensive";

/// Runtime SDK context.
pub trait Context {
    /// Runtime that the context is being invoked in.
    type Runtime: runtime::Runtime;

    /// Clone this context.
    fn clone(&self) -> Self;

    /// Returns a logger.
    fn get_logger(&self, module: &'static str) -> slog::Logger;

    /// Whether smart contracts should be executed in this context.
    fn should_execute_contracts(&self) -> bool {
        match CurrentState::with_env(|env| env.mode()) {
            // When actually executing a transaction, we always run contracts.
            state::Mode::Execute => true,
            state::Mode::Simulate => {
                // Backwards compatibility for the deprecated `allow_expensive_queries`.
                if let Some(allow_expensive_queries) =
                    self.local_config::<bool>(LOCAL_CONFIG_ALLOW_EXPENSIVE_QUERIES)
                {
                    slog::warn!(
                        self.get_logger("runtime-sdk"),
                        "The {} config option is DEPRECATED since April 2022 and will be removed in a future release. Use {} and {} instead.",
                        LOCAL_CONFIG_ALLOW_EXPENSIVE_QUERIES,
                        LOCAL_CONFIG_ESTIMATE_GAS_BY_SIMULATING_CONTRACTS,
                        LOCAL_CONFIG_ALLOWED_QUERIES
                    );
                    return allow_expensive_queries;
                };

                // The non-deprecated config option.
                self.local_config(LOCAL_CONFIG_ESTIMATE_GAS_BY_SIMULATING_CONTRACTS)
                    .unwrap_or_default()
            }
            // When just checking a transaction, we always want to be fast and skip contracts.
            state::Mode::Check | state::Mode::PreSchedule => false,
        }
    }

    /// Whether `method` is an allowed query per policy in the local config.
    fn is_allowed_query<R: crate::runtime::Runtime>(&self, method: &str) -> bool {
        let config: Vec<BTreeMap<String, bool>> = self
            .local_config(LOCAL_CONFIG_ALLOWED_QUERIES)
            .unwrap_or_default();
        let is_expensive = R::Modules::is_expensive_query(method);

        // Backwards compatibility for the deprecated `allow_expensive_queries`.
        if let Some(allow_expensive_queries) =
            self.local_config::<bool>(LOCAL_CONFIG_ALLOW_EXPENSIVE_QUERIES)
        {
            slog::warn!(
                self.get_logger("runtime-sdk"),
                "The {} config option is DEPRECATED since April 2022 and will be removed in a future release. Use {} and {} instead.",
                LOCAL_CONFIG_ALLOW_EXPENSIVE_QUERIES,
                LOCAL_CONFIG_ESTIMATE_GAS_BY_SIMULATING_CONTRACTS,
                LOCAL_CONFIG_ALLOWED_QUERIES
            );
            return (!is_expensive) || allow_expensive_queries;
        };

        // The non-deprecated config option.
        config
            .iter()
            .find_map(|item| {
                item.get(method)
                    .or_else(|| {
                        if !is_expensive {
                            return None;
                        }
                        item.get(LOCAL_CONFIG_ALLOWED_QUERIES_ALL_EXPENSIVE)
                    })
                    .or_else(|| item.get(LOCAL_CONFIG_ALLOWED_QUERIES_ALL))
                    .copied()
            })
            // If no config entry matches, the default is to allow only non-expensive queries.
            .unwrap_or(!is_expensive)
    }

    /// Returns node operator-provided local configuration.
    ///
    /// This method will always return `None` in `Mode::ExecuteTx` contexts.
    fn local_config<T>(&self, key: &str) -> Option<T>
    where
        T: cbor::Decode,
    {
        if CurrentState::with_env(|env| env.is_execute()) {
            return None;
        }

        self.host_info().local_config.get(key).and_then(|v| {
            cbor::from_value(v.clone()).unwrap_or_else(|e| {
                let msg = format!(
                    "Cannot interpret the value of \"{}\" in runtime's local config as a {}: {:?}",
                    key,
                    std::any::type_name::<T>(),
                    e
                );
                slog::error!(self.get_logger("runtime-sdk"), "{}", msg);
                panic!("{}", msg);
            })
        })
    }

    /// Information about the host environment.
    fn host_info(&self) -> &HostInfo;

    /// Runtime ID.
    fn runtime_id(&self) -> &Namespace {
        &self.host_info().runtime_id
    }

    /// The key manager, if the runtime is confidential.
    fn key_manager(&self) -> Option<&dyn KeyManager>;

    /// Whether the context has a key manager available (e.g. the runtime is confidential).
    fn is_confidential(&self) -> bool {
        self.key_manager().is_some()
    }

    /// Last runtime block header.
    fn runtime_header(&self) -> &roothash::Header;

    /// Results of executing the last successful runtime round.
    fn runtime_round_results(&self) -> &roothash::RoundResults;

    /// Consensus state.
    fn consensus_state(&self) -> &consensus::state::ConsensusState;

    /// Historical state.
    fn history(&self) -> &dyn history::HistoryHost;

    /// Current epoch.
    fn epoch(&self) -> consensus::beacon::EpochTime;

    /// Maximum number of consensus messages that the runtime can emit in this block.
    fn max_messages(&self) -> u32;

    /// UNIX timestamp of the current block.
    fn now(&self) -> u64 {
        self.runtime_header().timestamp
    }
}

/// Dispatch context for the whole batch.
pub struct RuntimeBatchContext<'a, R: runtime::Runtime> {
    host_info: &'a HostInfo,
    key_manager: Option<Box<dyn KeyManager>>,
    runtime_header: &'a roothash::Header,
    runtime_round_results: &'a roothash::RoundResults,
    consensus_state: &'a consensus::state::ConsensusState,
    history: &'a dyn history::HistoryHost,
    epoch: consensus::beacon::EpochTime,
    logger: slog::Logger,

    /// Maximum number of messages that can be emitted.
    max_messages: u32,

    _runtime: PhantomData<R>,
}

impl<'a, R: runtime::Runtime> RuntimeBatchContext<'a, R> {
    /// Create a new dispatch context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host_info: &'a HostInfo,
        key_manager: Option<Box<dyn KeyManager>>,
        runtime_header: &'a roothash::Header,
        runtime_round_results: &'a roothash::RoundResults,
        consensus_state: &'a consensus::state::ConsensusState,
        history: &'a dyn history::HistoryHost,
        epoch: consensus::beacon::EpochTime,
        max_messages: u32,
    ) -> Self {
        Self {
            host_info,
            runtime_header,
            runtime_round_results,
            consensus_state,
            history,
            epoch,
            key_manager,
            logger: get_logger("runtime-sdk"),
            max_messages,
            _runtime: PhantomData,
        }
    }
}

impl<R: runtime::Runtime> Context for RuntimeBatchContext<'_, R> {
    type Runtime = R;

    fn clone(&self) -> Self {
        Self {
            host_info: self.host_info,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            consensus_state: self.consensus_state,
            history: self.history,
            epoch: self.epoch,
            key_manager: self.key_manager.clone(),
            logger: get_logger("runtime-sdk"),
            max_messages: self.max_messages,
            _runtime: PhantomData,
        }
    }

    fn get_logger(&self, module: &'static str) -> slog::Logger {
        self.logger.new(o!("sdk_module" => module))
    }

    fn host_info(&self) -> &HostInfo {
        self.host_info
    }

    fn key_manager(&self) -> Option<&dyn KeyManager> {
        self.key_manager.as_ref().map(Box::as_ref)
    }

    fn runtime_header(&self) -> &roothash::Header {
        self.runtime_header
    }

    fn runtime_round_results(&self) -> &roothash::RoundResults {
        self.runtime_round_results
    }

    fn consensus_state(&self) -> &consensus::state::ConsensusState {
        self.consensus_state
    }

    fn history(&self) -> &dyn history::HistoryHost {
        self.history
    }

    fn epoch(&self) -> consensus::beacon::EpochTime {
        self.epoch
    }

    fn max_messages(&self) -> u32 {
        self.max_messages
    }
}
