use oasis_contract_sdk::{self as sdk, types::token};

pub mod spec;

/// State of the wormhole contract.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub struct Config {
    /// Currently active guardian set.
    pub guardian_set_index: u32,

    /// Period for which a guardian set stays active after it has been replaced.
    pub guardian_set_expiry: u64,

    /// Chain ID of the wormhole governance chain.
    pub governance_chain: u16,

    /// Address of the wormhole governance contract.
    pub governance_address: spec::Address,

    /// Wormhole message sending fee.
    pub fee: token::BaseUnits,
}

/// Parameters used to instantiate the contract state.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct InstantiateParameters {
    /// Chain ID of the wormhole governance chain.
    pub governance_chain: u16,
    /// Governance address of the governance wormhole contract.
    pub governance_address: spec::Address,

    // Initial guardian set index.
    pub initial_guardian_set_index: u32,
    // Initial guardian set.
    pub initial_guardian_set: spec::GuardianSet,
    // Period for which the guardian sets stay active after it's been replaced.
    pub guardian_set_expiry: u64,

    // Initial wormhole massages fee. Wormhole fees are in native denomination.
    pub fee: u128,
}

/// All possible errors that can be returned by the contract.
///
/// Each error is a triplet of (module, code, message) which allows it to be both easily
/// human readable and also identifiable programmatically.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("query failed")]
    #[sdk_error(code = 2)]
    QueryFailed,

    #[error("VAA already executed")]
    #[sdk_error(code = 3)]
    VAAAlreadyExecuted,

    #[error("insufficient fee paid")]
    #[sdk_error(code = 4)]
    InsufficientFeePaid,

    #[error("invalid VAA: {0}")]
    #[sdk_error(code = 5)]
    InvalidVAA(#[source] anyhow::Error),

    #[error("invalid VAA version")]
    #[sdk_error(code = 6)]
    InvalidVAAVersion,

    #[error("invalid VAA action")]
    #[sdk_error(code = 7)]
    InvalidVAAAction,

    #[error("invalid guardian set for governance")]
    #[sdk_error(code = 8)]
    InvalidGuardianSetForGovernance,

    #[error("invalid VAA module")]
    #[sdk_error(code = 9)]
    InvalidVAAModule,

    #[error("invalid VAA chain ID")]
    #[sdk_error(code = 10)]
    InvalidVAAChainId,

    #[error("invalid guardian set upgrade index")]
    #[sdk_error(code = 11)]
    InvalidGuardianSetUpgradeIndex,

    #[error("invalid VAA payload")]
    #[sdk_error(code = 12)]
    InvalidVAAPayload,

    #[error("invalid VAA invalid guardian set index")]
    #[sdk_error(code = 13)]
    VAAInvalidGuardianSetIndex,

    #[error("invalid VAA guardian set expired")]
    #[sdk_error(code = 14)]
    VAAGuardianSetExpired,

    #[error("invalid VAA quorum not reached")]
    #[sdk_error(code = 15)]
    VAANoQuorum,

    #[error("invalid VAA to many signatures")]
    #[sdk_error(code = 16)]
    VAATooManySignatures,

    #[error("invalid VAA guardian signature error")]
    #[sdk_error(code = 17)]
    VAAGuardianSignatureError,

    #[error("empty guardian set")]
    #[sdk_error(code = 18)]
    EmptyGuardianSet,
}

/// All possible events that can be returned by the contract.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    PostMessage {
        message: Vec<u8>,
        nonce: u32,
        sender: spec::Address,
        chain_id: u16,
        sequence: u64,
        block_time: u64,
    },

    #[sdk_event(code = 2)]
    GuardianSetUpdate { index: u32 },

    #[sdk_event(code = 3)]
    FeeUpdate { fee: token::BaseUnits },
}

/// All possible requests that the contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    // Calls.
    #[cbor(rename = "instantiate")]
    Instantiate { params: InstantiateParameters },

    #[cbor(rename = "post_message")]
    PostMessage { message: Vec<u8>, nonce: u32 },

    #[cbor(rename = "submit_vaa")]
    SubmitVAA { vaa: Vec<u8> },

    // Queries.
    #[cbor(rename = "guardian_state_info")]
    GuardianSetInfo,

    #[cbor(rename = "verify_vaa")]
    VerifyVAA { vaa: Vec<u8>, block_time: u64 },

    #[cbor(rename = "get_config")]
    GetConfig,
}

/// All possible responses that the contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "empty")]
    Empty,

    #[cbor(rename = "get_config")]
    GetConfig { config: Config },

    #[cbor(rename = "guardian_set_info")]
    GuardianSetInfo { guardian_set: spec::GuardianSet },

    #[cbor(rename = "verified_vaa")]
    VerifiedVAA { vaa: spec::ParsedVAA },
}
