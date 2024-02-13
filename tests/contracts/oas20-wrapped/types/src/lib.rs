use oasis_contract_sdk::{self as sdk};
use oasis_contract_sdk_oas20_types as oas20;
use oasis_contract_sdk_types::{address::Address, InstanceId};

use oasis_wormhole_types as wormhole;

/// All possible requests that the OAS20-wrapped contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    #[cbor(embed)]
    Oas20(oas20::Request),

    // Overrides Oas20::Instantiate.
    #[cbor(rename = "instantiate")]
    Instantiate {
        token_instantiation: oas20::TokenInstantiation,
        asset_chain_id: u16,
        asset_address: wormhole::spec::Address,
    },

    #[cbor(rename = "burn_from")]
    BurnFrom { from: Address, amount: u128 },

    #[cbor(rename = "bridge_wrapped_info")]
    BridgeWrappedInfo,
}

impl From<oas20::Request> for Request {
    fn from(request: oas20::Request) -> Self {
        Self::Oas20(request)
    }
}

/// All possible responses that the OAS20-wrapped contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(embed)]
    Oas20(oas20::Response),

    #[cbor(rename = "empty")]
    Empty,

    #[cbor(rename = "bridge_wrapped_info")]
    BridgeWrappedInfo { info: BridgeWrappedInfo },
}

impl From<oas20::Response> for Response {
    fn from(response: oas20::Response) -> Self {
        Self::Oas20(response)
    }
}

/// All possible errors that can be returned by the OAS20-wrapped contract.
///
/// Each error is a triplet of (module, code, message) which allows it to be both easily
/// human readable and also identifiable programmatically.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("burning forbidden")]
    #[sdk_error(code = 2)]
    BurningForbidden,

    #[error("minter not configured")]
    #[sdk_error(code = 3)]
    MinterNotConfigured,

    #[error("{0}")]
    #[sdk_error(transparent)]
    Oas20(#[from] oas20::Error),
}

/// All possible events that can be returned by the OAS20-wrapped contract.
//
// XXX: could the OAS-20 events be "embedded" somehow?
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    WrappedOas20Instantiated {
        token_information: oas20::TokenInformation,
        wrapped_info: BridgeWrappedInfo,
    },

    #[sdk_event(code = 2)]
    Oas20Transferred {
        from: Address,
        to: Address,
        amount: u128,
    },

    #[sdk_event(code = 3)]
    Oas20Sent {
        from: Address,
        to: InstanceId,
        amount: u128,
    },

    #[sdk_event(code = 4)]
    Oas20Burned { from: Address, amount: u128 },

    #[sdk_event(code = 5)]
    Oas20AllowanceChanged {
        owner: Address,
        beneficiary: Address,
        allowance: u128,
        negative: bool,
        amount_change: u128,
    },

    #[sdk_event(code = 6)]
    Oas20Withdraw {
        from: Address,
        to: Address,
        amount: u128,
    },

    #[sdk_event(code = 7)]
    Oas20Minted { to: Address, amount: u128 },
}

/// Wormhole token bridge information about the wrapped asset.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct BridgeWrappedInfo {
    pub asset_chain_id: u16,
    pub asset_address: wormhole::spec::Address,
    pub bridge_address: Address,
}
