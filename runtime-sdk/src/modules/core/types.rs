use std::collections::BTreeMap;

use crate::{
    keymanager::SignedPublicKey,
    types::transaction::{CallResult, CallerAddress, Transaction},
};

/// Key in the versions map used for the global state version.
pub const VERSION_GLOBAL_KEY: &str = "";

/// Basic per-module metadata; tracked in core module's state.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Metadata {
    /// A set of state versions for all supported modules.
    pub versions: BTreeMap<String, u32>,
}

/// Arguments for the EstimateGas query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct EstimateGasQuery {
    /// The address of the caller for which to do estimation. If not specified the authentication
    /// information from the passed transaction is used.
    #[cbor(optional)]
    pub caller: Option<CallerAddress>,
    /// The unsigned transaction to estimate.
    pub tx: Transaction,
    /// If the estimate gas query should fail in case of transaction failures.
    /// If true, the query will return the transaction error and not the gas estimation.
    /// Defaults to false.
    #[cbor(optional)]
    pub propagate_failures: bool,
}

/// Response to the call data public key query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct CallDataPublicKeyQueryResponse {
    /// Public key used for deriving the shared secret for encrypting call data.
    pub public_key: SignedPublicKey,
}

#[derive(Debug, Copy, Clone, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum MethodHandlerKind {
    #[cbor(rename = "call")]
    Call,
    // `Prefetch` is omitted because it is an implementation detail of handling `Call`s.
    #[cbor(rename = "query")]
    Query,
    #[cbor(rename = "message_result")]
    MessageResult,
}

#[derive(Debug, Clone, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[cbor(no_default)]
pub struct MethodHandlerInfo {
    pub kind: MethodHandlerKind,
    pub name: String,
}

/// Metadata for an individual module.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[cbor(no_default)]
pub struct ModuleInfo {
    pub version: u32,
    pub params: cbor::Value,
    pub methods: Vec<MethodHandlerInfo>,
}

/// Response to the RuntimeInfo query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[cbor(no_default)]
pub struct RuntimeInfoResponse {
    pub runtime_version: oasis_core_runtime::common::version::Version,
    pub state_version: u32,
    pub modules: BTreeMap<String, ModuleInfo>,
}

/// Arguments for the ExecuteReadOnlyTx query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ExecuteReadOnlyTxQuery {
    pub tx: Vec<u8>,
}

/// Response to the ExecuteReadOnlyTx query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ExecuteReadOnlyTxResponse {
    pub result: CallResult,
}
