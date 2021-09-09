use oasis_contract_sdk::{self as sdk};
use oasis_contract_sdk_types::{address::Address, InstanceId};

#[cfg(test)]
mod test;

/// OAS20 token instantiation initial balance information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct InitialBalance {
    pub address: Address,
    pub amount: u128,
}

/// Token minting information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct MintingInformation {
    /// Caller address which is allowed to mint new tokens.
    pub minter: Address,
    /// Cap on the total supply of the token.
    #[cbor(optional)]
    pub cap: Option<u128>,
}

/// OAS20 token instantiation information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct TokenInstantiation {
    /// Name of the token.
    pub name: String,
    /// Token symbol.
    pub symbol: String,
    /// Number of decimals.
    pub decimals: u8,
    /// Initial balances of the token.
    #[cbor(optional, default, skip_serializing_if = "Vec::is_empty")]
    pub initial_balances: Vec<InitialBalance>,
    /// Information about minting in case the token supports minting.
    #[cbor(optional)]
    pub minting: Option<MintingInformation>,
}

/// OAS20 token information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct TokenInformation {
    /// Name of the token.
    pub name: String,
    /// Token symbol.
    pub symbol: String,
    /// Number of decimals.
    pub decimals: u8,
    /// Total supply of the token.
    pub total_supply: u128,
    /// Information about minting in case the token supports minting.
    #[cbor(optional)]
    pub minting: Option<MintingInformation>,
}

/// All possible errors that can be returned by the OAS20 contract.
///
/// Each error is a triplet of (module, code, message) which allows it to be both easily
/// human readable and also identifiable programmatically.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("total supply overflow")]
    #[sdk_error(code = 2)]
    TotalSupplyOverflow,

    #[error("zero amount")]
    #[sdk_error(code = 3)]
    ZeroAmount,

    #[error("insufficient funds")]
    #[sdk_error(code = 4)]
    InsufficientFunds,

    #[error("minting forbidden")]
    #[sdk_error(code = 5)]
    MintingForbidden,

    #[error("mint over cap")]
    #[sdk_error(code = 6)]
    MintOverCap,

    #[error("allower and beneficiary same")]
    #[sdk_error(code = 7)]
    SameAllowerAndBeneficiary,

    #[error("insufficient allowance")]
    #[sdk_error(code = 8)]
    InsufficientAllowance,
}

/// All possible events that can be returned by the OAS20 contract.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Oas20Instantiated { token_information: TokenInformation },

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

/// All possible requests that the OAS20 contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    // Calls.
    #[cbor(rename = "instantiate")]
    Instantiate(TokenInstantiation),

    #[cbor(rename = "transfer")]
    Transfer { to: Address, amount: u128 },

    #[cbor(rename = "send")]
    Send {
        to: InstanceId,
        amount: u128,
        data: cbor::Value,
    },

    #[cbor(rename = "burn")]
    Burn { amount: u128 },

    #[cbor(rename = "mint")]
    Mint { to: Address, amount: u128 },

    #[cbor(rename = "allow")]
    Allow {
        beneficiary: Address,
        negative: bool,
        amount_change: u128,
    },

    #[cbor(rename = "withdraw")]
    Withdraw { from: Address, amount: u128 },

    // Queries.
    #[cbor(rename = "token_information")]
    TokenInformation,

    #[cbor(rename = "balance")]
    Balance { address: Address },

    #[cbor(rename = "allowance")]
    Allowance {
        allower: Address,
        beneficiary: Address,
    },
}

/// All possible responses that the OAS20 contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "token_information")]
    TokenInformation { token_information: TokenInformation },

    #[cbor(rename = "balance")]
    Balance { balance: u128 },

    #[cbor(rename = "allowance")]
    Allowance { allowance: u128 },

    #[cbor(rename = "empty")]
    Empty,
}

/// OAS20 receiver request. Contracts expecting to receive OAS20 tokens should
/// implement these requests.
#[derive(Clone, Debug, cbor::Decode, cbor::Encode)]
pub enum ReceiverRequest {
    #[cbor(rename = "oas20_receive")]
    Receive {
        sender: Address,
        amount: u128,
        data: cbor::Value,
    },
}
