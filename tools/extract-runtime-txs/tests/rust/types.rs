//! Contracts module types.
pub use oasis_contract_sdk_types::{CodeId, InstanceId};
use oasis_runtime_sdk::{
    context::TxContext,
    core::common::crypto::hash::Hash,
    types::{address::Address, token},
};

use super::{Error, MODULE_NAME};

/// A generic policy that specifies who is allowed to perform an action.
#[derive(Clone, Copy, Debug, cbor::Encode, cbor::Decode)]
pub enum Policy {
    #[cbor(rename = "nobody", as_struct)]
    Nobody,

    #[cbor(rename = "address")]
    Address(Address),

    #[cbor(rename = "everyone", as_struct)]
    Everyone,
}

impl Policy {
    /// Enforce the given policy by returning an error if the policy is not satisfied by the passed
    /// transaction context.
    pub fn enforce<C: TxContext>(&self, ctx: &mut C) -> Result<(), Error> {
        match self {
            // Nobody is allowed to perform the action.
            Policy::Nobody => Err(Error::Forbidden),
            // Only the given caller is allowed to perform the action.
            Policy::Address(address) if address == &ctx.tx_caller_address() => Ok(()),
            Policy::Address(_) => Err(Error::Forbidden),
            // Anyone is allowed to perform the action.
            Policy::Everyone => Ok(()),
        }
    }
}

/// ABI that the given contract should conform to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum ABI {
    /// Custom Oasis SDK-specific ABI (v1).
    OasisV1 = 1,
}

/// Stored code information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Code {
    // omitted for tests...
}

/// Deployed code instance information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Instance {
    /// Unique instance identifier.
    pub id: InstanceId,

    /// Identifier of code used by the instance.
    pub code_id: CodeId,

    /// Instance creator address.
    pub creator: Address,

    /// Who is allowed to upgrade this instance.
    pub upgrades_policy: Policy,
}

impl Instance {
    /// Address associated with a specific contract instance.
    pub fn address_for(id: InstanceId) -> Address {
        Address::from_module_raw(MODULE_NAME, &id.as_u64().to_be_bytes())
    }

    /// Address associated with the contract.
    pub fn address(&self) -> Address {
        Self::address_for(self.id)
    }
}

/// Upload call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Upload {
    /// ABI.
    pub abi: ABI,

    /// Who is allowed to instantiate this code.
    pub instantiate_policy: Policy,

    /// Compiled contract code.
    pub code: Vec<u8>,
}

/// Upload call result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct UploadResult {
    /// Assigned code identifier.
    pub id: CodeId,
}

/// Instantiate call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Instantiate {
    /// Identifier of code used by the instance.
    pub code_id: CodeId,

    /// Who is allowed to upgrade this instance.
    pub upgrades_policy: Policy,

    /// Arguments to contract's instantiation function.
    pub data: Vec<u8>,

    /// Tokens that should be sent to the contract as part of the instantiate call.
    pub tokens: Vec<token::BaseUnits>,
}

/// Instantiate call result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstantiateResult {
    /// Assigned instance identifier.
    pub id: InstanceId,
}

/// Contract call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Call {
    /// Instance identifier.
    pub id: InstanceId,

    /// Call arguments.
    pub data: Vec<u8>,

    /// Tokens that should be sent to the contract as part of the call.
    pub tokens: Vec<token::BaseUnits>,
}

/// Contract call result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cbor(transparent)]
pub struct CallResult(pub Vec<u8>);

/// Upgrade call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Upgrade {
    /// Instance identifier.
    pub id: InstanceId,

    /// Updated code identifier.
    pub code_id: CodeId,

    /// Arguments to contract's upgrade function.
    pub data: Vec<u8>,

    /// Tokens that should be sent to the contract as part of the call.
    pub tokens: Vec<token::BaseUnits>,
}

/// Code information query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct CodeQuery {
    /// Code identifier.
    pub id: CodeId,
}

/// Instance information query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct InstanceQuery {
    /// Instance identifier.
    pub id: InstanceId,
}


/// Deposit into runtime call.
/// Transfer from consensus staking to an account in this runtime.
/// The transaction signer has a consensus layer allowance benefiting this runtime's staking
/// address. The `to` address runtime account gets the tokens.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Deposit {
}

/// Balance query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct BalanceQuery {
}
