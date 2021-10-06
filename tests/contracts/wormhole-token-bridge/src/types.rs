use oasis_contract_sdk::{
    self as sdk,
    types::{address::Address, token, CodeId, InstanceId},
};

use oasis_wormhole_types as wormhole;

/// State of the wormhole contract.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Configuration {
    /// Chain ID of the wormhole governance chain.
    pub governance_chain: u16,
    /// Address of the wormhole governance "contract".
    pub governance_address: wormhole::spec::Address,

    // Wormhole contract ID on the oasis network.
    pub wormhole_contract: InstanceId,
    // ID of the wrapped asset code.
    pub wrapped_asset_code_id: CodeId,
}

/// All possible errors that can be returned by the contract.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("VAA already executed")]
    #[sdk_error(code = 2)]
    VaaAlreadyExecuted,

    #[error("amount too high")]
    #[sdk_error(code = 3)]
    AmountTooHigh,

    #[error("amount too low")]
    #[sdk_error(code = 4)]
    AmountTooLow,

    #[error("invalid VAA action")]
    #[sdk_error(code = 5)]
    InvalidVaaAction,

    #[error("transfer recipient oasis chain")]
    #[sdk_error(code = 6)]
    TransferRecipientOasisChain,

    #[error("transfer fee greater than amount")]
    #[sdk_error(code = 7)]
    TransferFeeGreaterThanAmount,

    #[error("query failed")] // timestamp query failed?
    #[sdk_error(code = 8)]
    QueryFailed,

    #[error("wormhole error: {0}")]
    #[sdk_error(transparent)]
    Wormhole(#[from] wormhole::Error),

    #[error("invalid VAA module")]
    #[sdk_error(code = 9)]
    InvalidVAAModule,

    #[error("invalid VAA payload")]
    #[sdk_error(code = 10)]
    InvalidVAAPayload,

    #[error("invalid VAA payload")]
    #[sdk_error(code = 11)]
    InvalidVAAAction,

    #[error("invalid VAA chain ID")]
    #[sdk_error(code = 12)]
    InvalidVAAChainId,

    #[error("chain already registered")]
    #[sdk_error(code = 13)]
    ChainAlreadyRegistered,

    #[error("vaa already executed")]
    #[sdk_error(code = 14)]
    VAAAlreadyExecuted,

    #[error("chain not registered")]
    #[sdk_error(code = 15)]
    ChainNotRegistered,

    #[error("invalid emitter address")]
    #[sdk_error(code = 16)]
    InvalidEmitterAddress,

    #[error("attesting native asset")]
    #[sdk_error(code = 17)]
    AttestingNativeAsset,

    #[error("asset already attested")]
    #[sdk_error(code = 18)]
    AssetAlreadyAttested,

    #[error("transfer not for oasis")]
    #[sdk_error(code = 19)]
    TransferNotForOasis,

    #[error("wrapped asset not deployed")]
    #[sdk_error(code = 20)]
    WrappedAssetNotDeployed,

    #[error("transfer failed")]
    #[sdk_error(code = 21)]
    TransferFailed,

    #[error("create asset meta failed")]
    #[sdk_error(code = 22)]
    CreateAssetMetaFailed,

    #[error("invalid transfer recipient")]
    #[sdk_error(code = 23)]
    InvalidTransferRecipient,

    #[error("locked asset limit exceeded")]
    #[sdk_error(code = 24)]
    LockedAssetLimitExceeded,
}

/// All possible events that can be returned by the contract.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    ChainRegistered {
        chain_id: u16,
        contract_address: wormhole::spec::Address,
    },

    #[sdk_event(code = 2)]
    InboundTransfer {
        contract_address: Address,
        recipient: Address,
        amount: u128,
        wrapped: bool,
    },

    #[sdk_event(code = 3)] // TODO: check addresses.
    OutboundTransfer {
        token_chain_id: u16,
        token: Address,
        sender: Address,
        recipient_chain: u16,
        recipient: wormhole::spec::Address,
        amount: u128,
        nonce: u64,
    },

    #[sdk_event(code = 4)]
    AssetRegistered { contract_instance_id: InstanceId },
}

/// All possible requests that the contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    // Calls.
    #[cbor(rename = "instantiate")]
    Instantiate(Configuration),

    #[cbor(rename = "initiate_transfer")]
    InitiateTransfer {
        asset: InstanceId, // TODO/NOTE: tokens are referred at by instance ID in our chain.
        amount: u128,
        recipient_chain: u16,
        recipient: wormhole::spec::Address,
        fee: u128,
        nonce: u32,
    },

    #[cbor(rename = "submit_vaa")]
    SubmitVAA { data: Vec<u8> },

    #[cbor(rename = "create_asset_meta")]
    CreateAssetMeta {
        asset_instance_id: InstanceId,
        nonce: u32,
    },
    // Queries.
}

/// All possible responses that the contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "empty")]
    Empty,
}

#[derive(Clone, Debug, Default, PartialEq, cbor::Encode, cbor::Decode)]
pub(crate) struct OutboundTransferData {
    pub amount: u128,
    pub fee: u128,
    pub recipient: wormhole::spec::Address,
    pub recipient_chain: u16,
    pub asset: InstanceId,
    pub nonce: u32,
    pub deposited_tokens: Vec<token::BaseUnits>,
}

#[derive(Clone, Debug, Default, PartialEq, cbor::Encode, cbor::Decode)]
pub(crate) struct InboundTransferData {
    pub amount: u128,
    pub fee: u128,
    pub asset: InstanceId,
    pub recipient: Address,
}

#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub(crate) struct AttestMetaData {
    pub asset: [u8; 34],
}

#[derive(Clone, Debug, Default, PartialEq, cbor::Encode, cbor::Decode)]
pub(crate) struct CreateAssetMetaData {
    pub asset: InstanceId,
    pub nonce: u32,
    pub deposited_tokens: Vec<token::BaseUnits>,
}
