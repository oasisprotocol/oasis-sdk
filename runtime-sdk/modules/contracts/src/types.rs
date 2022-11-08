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
#[cbor(no_default)]
pub struct Code {
    /// Unique code identifier.
    pub id: CodeId,

    /// Code hash.
    pub hash: Hash,

    /// ABI.
    pub abi: ABI,

    /// ABI sub-version.
    #[cbor(optional)]
    pub abi_sv: u32,

    /// Code uploader address.
    pub uploader: Address,

    /// Who is allowed to instantiate this code.
    pub instantiate_policy: Policy,
}

/// Deployed code instance information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
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
#[cbor(no_default)]
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
#[cbor(no_default)]
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
#[cbor(no_default)]
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
#[cbor(no_default)]
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

/// Change upgrade policy call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct ChangeUpgradePolicy {
    /// Instance identifier.
    pub id: InstanceId,

    /// Updated upgrade policy.
    pub upgrades_policy: Policy,
}

/// Code information query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct CodeQuery {
    /// Code identifier.
    pub id: CodeId,
}

/// Code storage information query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct CodeStorageQuery {
    /// Code identifier.
    pub id: CodeId,
}

/// Code storage query result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct CodeStorageQueryResult {
    /// Stored contract code.
    pub code: Vec<u8>,
}

/// Instance information query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceQuery {
    /// Instance identifier.
    pub id: InstanceId,
}

/// Instance storage query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceStorageQuery {
    /// Instance identifier.
    pub id: InstanceId,

    /// Storage key.
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceStorageQueryResult {
    /// Storage value or `None` if key doesn't exist.
    pub value: Option<Vec<u8>>,
}

/// Exposed wrapper for oasis-contract-sdk-types::StoreKind.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[repr(u32)]
pub enum StoreKind {
    Public = 0,
    Confidential = 1,
}

/// Instance raw storage query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct InstanceRawStorageQuery {
    /// Instance identifier.
    pub id: InstanceId,

    /// Kind of the store to query.
    pub store_kind: StoreKind,

    /// Maximum number of items per page.
    #[cbor(optional)]
    pub limit: Option<u64>,

    /// Number of skipped items.
    #[cbor(optional)]
    pub offset: Option<u64>,
}

/// Instance raw storage query result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceRawStorageQueryResult {
    /// List of key-value pairs in contract's public store.
    pub items: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Public key kind.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum PublicKeyKind {
    Transaction = 1,
}

/// Public key query.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct PublicKeyQuery {
    /// Instance identifier.
    pub id: InstanceId,

    /// Kind of public key.
    pub kind: PublicKeyKind,
}

/// Public key query result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct PublicKeyQueryResult {
    /// Public key.
    pub key: Vec<u8>,

    /// Checksum of the key manager state.
    pub checksum: Vec<u8>,

    /// Sign(sk, (key || checksum)) from the key manager.
    pub signature: Vec<u8>,
}

/// Custom contract query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct CustomQuery {
    /// Instance identifier.
    pub id: InstanceId,

    /// Query arguments.
    pub data: Vec<u8>,
}

/// Custom query result.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cbor(transparent)]
pub struct CustomQueryResult(pub Vec<u8>);

/// An event emitted from a contract, wrapped to include additional metadata.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ContractEvent {
    /// Identifier of the instance that emitted the event.
    pub id: InstanceId,
    /// Raw event data emitted by the instance.
    #[cbor(optional)]
    pub data: Vec<u8>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instanceid_to_address() {
        let tcs = vec![
            (
                InstanceId::from(0),
                "oasis1qq08mjlkztsgpgrar082rzzxwjaplxmgjs5ftugn",
            ),
            (
                InstanceId::from(1),
                "oasis1qpg6jv8mxwlv4z578xyjxl7d793jamltdg9czzkx",
            ),
            (
                InstanceId::from(14324),
                "oasis1qzasj0kq0hlq6vzw4ajhrwgp3tqx6rnwvg2ylu2v",
            ),
            (
                InstanceId::from(u64::MAX),
                "oasis1qqr0nxsu5aqpu4k85z4h5z08vrfmawnnqycl6gup",
            ),
        ];

        for (id, address) in tcs {
            let instance_address = Instance::address_for(id);
            assert_eq!(
                instance_address.to_bech32(),
                address.to_string(),
                "instance address should match"
            );
        }
    }
}
