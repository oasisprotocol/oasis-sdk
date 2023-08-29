//! Consensus module types.
use oasis_core_runtime::consensus::beacon::EpochTime;

use crate::types::{address::Address, message::MessageEvent, token};

/// Deposit into runtime call.
/// Transfer from consensus staking to an account in this runtime.
/// The transaction signer has a consensus layer allowance benefiting this runtime's staking
/// address. The `to` address runtime account gets the tokens.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Deposit {
    #[cbor(optional)]
    pub to: Option<Address>,
    pub amount: token::BaseUnits,
}

/// Withdraw from runtime call.
/// Transfer from an account in this runtime to consensus staking.
/// The `to` address consensus staking account gets the tokens.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Withdraw {
    #[cbor(optional)]
    pub to: Option<Address>,
    pub amount: token::BaseUnits,
}

/// Delegate from runtime call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Delegate {
    pub to: Address,
    pub amount: token::BaseUnits,
}

/// Undelegate into runtime call.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Undelegate {
    pub from: Address,
    pub shares: u128,
}

/// Balance query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct BalanceQuery {
    pub address: Address,
}

/// Consensus account query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ConsensusAccountQuery {
    pub address: Address,
}

/// Delegation query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DelegationQuery {
    pub from: Address,
    pub to: Address,
}

/// Delegations query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DelegationsQuery {
    pub from: Address,
}

/// Undelegations query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct UndelegationsQuery {
    pub to: Address,
}

#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AccountBalance {
    pub balance: u128,
}

/// Information about a delegation.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DelegationInfo {
    /// The amount of owned shares.
    pub shares: u128,
}

/// Extended information about a delegation.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ExtendedDelegationInfo {
    /// Address delegated to.
    pub to: Address,
    /// The amount of owned shares.
    pub shares: u128,
}

/// Information about an undelegation.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct UndelegationInfo {
    /// Address being undelegated from.
    pub from: Address,
    /// Epoch when the undelegation will be complete.
    pub epoch: EpochTime,
    /// The amount of undelegated shares.
    pub shares: u128,
}

/// Context for consensus transfer message handler.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ConsensusTransferContext {
    pub address: Address,
    #[cbor(optional)]
    pub nonce: u64,
    #[cbor(optional)]
    pub to: Address,
    pub amount: token::BaseUnits,
}

/// Context for consensus withdraw message handler.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ConsensusWithdrawContext {
    #[cbor(optional)]
    pub from: Address,
    #[cbor(optional)]
    pub nonce: u64,
    pub address: Address,
    pub amount: token::BaseUnits,
}

/// Context for consensus delegate message handler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode, Default)]
pub struct ConsensusDelegateContext {
    pub from: Address,
    pub nonce: u64,
    pub to: Address,
    pub amount: token::BaseUnits,
}

/// Context for consensus undelegate message handler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode, Default)]
pub struct ConsensusUndelegateContext {
    pub from: Address,
    pub nonce: u64,
    pub to: Address,
    pub shares: u128,
}

/// Error details from the consensus layer.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct ConsensusError {
    #[cbor(optional)]
    pub module: String,

    #[cbor(optional)]
    pub code: u32,
}

impl From<MessageEvent> for ConsensusError {
    fn from(me: MessageEvent) -> Self {
        Self {
            module: me.module,
            code: me.code,
        }
    }
}
