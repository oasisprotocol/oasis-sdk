//! Account module types.
use std::collections::BTreeMap;

use num_traits::identities::Zero;
use serde::{Deserialize, Serialize};

use crate::types::{address::Address, token};

/// Transfer call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    #[serde(rename = "to")]
    pub to: Address,

    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}

/// Account metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    #[serde(rename = "nonce")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Zero::is_zero")]
    pub nonce: u64,
}

/// Arguments for the Nonce query.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonceQuery {
    #[serde(rename = "address")]
    pub address: Address,
}

/// Arguments for the Balances query.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalancesQuery {
    #[serde(rename = "address")]
    pub address: Address,
}

/// Balances in an account.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountBalances {
    #[serde(rename = "balances")]
    pub balances: BTreeMap<token::Denomination, token::Quantity>,
}
