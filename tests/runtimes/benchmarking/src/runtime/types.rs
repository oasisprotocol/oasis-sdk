use serde::{Deserialize, Serialize};

use oasis_runtime_sdk::types::{address::Address, token};

/// Accounts mint call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountsMint {
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}

/// Accounts transfer call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountsTransfer {
    #[serde(rename = "to")]
    pub to: Address,

    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,
}
