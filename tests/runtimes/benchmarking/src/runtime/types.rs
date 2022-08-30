use oasis_runtime_sdk::types::{address::Address, token};

/// Accounts mint call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AccountsMint {
    pub amount: token::BaseUnits,
}

/// Accounts transfer call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AccountsTransfer {
    pub to: Address,
    pub amount: token::BaseUnits,
}
