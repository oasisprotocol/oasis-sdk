//! Consensus module types.
use serde::{Deserialize, Serialize};

use crate::types::{address::Address, token};

/// Deposit into runtime call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deposit {
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}

/// Withdraw from runtime call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Withdraw {
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}

/// Balance query.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalanceQuery {
    #[serde(rename = "addr")]
    pub addr: Address,
}

/// Consensus account query.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsensusAccountQuery {
    #[serde(rename = "addr")]
    pub addr: Address,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountBalance {
    #[serde(rename = "balance")]
    pub balance: token::Quantity,
}

/// Context for consensus transfer message handler.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConsensusTransferContext {
    #[serde(rename = "address")]
    pub address: Address,
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}

/// Context for consensus withdraw message handler.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConsensusWithdrawContext {
    #[serde(rename = "address")]
    pub address: Address,
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}
