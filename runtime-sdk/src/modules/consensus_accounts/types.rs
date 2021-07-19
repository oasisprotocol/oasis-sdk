//! Consensus module types.
use crate::types::{address::Address, token};

/// Deposit into runtime call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Deposit {
    pub amount: token::BaseUnits,
}

/// Withdraw from runtime call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Withdraw {
    pub amount: token::BaseUnits,
}

/// Balance query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct BalanceQuery {
    pub address: Address,
}

/// Consensus account query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct ConsensusAccountQuery {
    pub address: Address,
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct AccountBalance {
    pub balance: u128,
}

/// Context for consensus transfer message handler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode, Default)]
pub struct ConsensusTransferContext {
    pub address: Address,
    pub amount: token::BaseUnits,
}

/// Context for consensus withdraw message handler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode, Default)]
pub struct ConsensusWithdrawContext {
    pub address: Address,
    pub amount: token::BaseUnits,
}
