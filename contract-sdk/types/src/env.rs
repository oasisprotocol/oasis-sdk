//! Environment query-related types.
use crate::{address::Address, token::Denomination};

/// A query request.
#[non_exhaustive]
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum QueryRequest {
    /// Information about the current runtime block.
    #[cbor(rename = "block_info")]
    BlockInfo,

    /// Accounts queries.
    #[cbor(rename = "accounts")]
    Accounts(AccountsQuery),
}

/// A query response.
#[non_exhaustive]
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum QueryResponse {
    /// Indication of a failing request.
    #[cbor(rename = "error")]
    Error {
        module: String,
        code: u32,
        message: String,
    },

    /// Information about the current runtime block.
    #[cbor(rename = "block_info")]
    BlockInfo {
        round: u64,
        epoch: u64,
        timestamp: u64,
    },

    /// Accounts queries.
    #[cbor(rename = "accounts")]
    Accounts(AccountsResponse),
}

/// Accounts API queries.
#[non_exhaustive]
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum AccountsQuery {
    /// Query an account's balance.
    #[cbor(rename = "balance")]
    Balance {
        address: Address,
        denomination: Denomination,
    },
}

impl From<AccountsQuery> for QueryRequest {
    fn from(q: AccountsQuery) -> Self {
        Self::Accounts(q)
    }
}

/// Accounts API responses.
#[non_exhaustive]
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum AccountsResponse {
    /// An account's balance of the given denomination.
    Balance { balance: u128 },
}

impl From<AccountsResponse> for QueryResponse {
    fn from(q: AccountsResponse) -> Self {
        Self::Accounts(q)
    }
}
