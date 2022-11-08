//! Account module types.
use std::collections::BTreeMap;

use crate::types::{address::Address, token};

/// Transfer call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Transfer {
    pub to: Address,
    pub amount: token::BaseUnits,
}

/// Account metadata.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Account {
    #[cbor(optional)]
    pub nonce: u64,
}

/// Arguments for the Nonce query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct NonceQuery {
    pub address: Address,
}

/// Arguments for the Addresses query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AddressesQuery {
    pub denomination: token::Denomination,
}

/// Arguments for the Balances query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct BalancesQuery {
    pub address: Address,
}

/// Balances in an account.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AccountBalances {
    pub balances: BTreeMap<token::Denomination, u128>,
}

/// Arguments for the DenominationInfo query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DenominationInfoQuery {
    pub denomination: token::Denomination,
}

/// Information about a denomination.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DenominationInfo {
    /// Number of decimals that the denomination is using.
    pub decimals: u8,
}
