//! Consensus module types.
use crate::types::{address::Address, token};

/// Deposit into runtime call.
/// Transfer from consensus staking to an account in this runtime.
/// The transaction signer has a consensus layer allowance benefiting this runtime's staking
/// address. The `to` address runtime account gets the tokens.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Deposit {
    pub to: Address,
    pub amount: token::BaseUnits,
}

/// Withdraw from runtime call.
/// Transfer from an account in this runtime to consensus staking.
/// The `to` address consensus staking account gets the tokens.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Withdraw {
    pub to: Address,
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
