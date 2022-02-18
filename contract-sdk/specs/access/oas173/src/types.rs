use oasis_contract_sdk::{self as sdk, types::address::Address};

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    // Calls.
    /// Set up the `Ownable` contract.
    ///
    /// The owner information is public. If you need privacy, set the owner to a mixer of some sort.
    #[cbor(rename = "instantiate")]
    Instantiate,

    /// Transfers ownership to a new account.
    ///
    /// Can only be called by the current owner. Emits an `OnwershipTransferred` event.
    #[cbor(rename = "transfer_ownership")]
    TransferOwnership { new_owner: Address },

    /// Unsets the owner. Calls to `require_owner` will forever return `false`.
    ///
    /// Can only be called by the current owner. Emits an `OnwershipTransferred` event.
    #[cbor(rename = "renounce_ownership")]
    RenounceOwnership,

    // Queries
    /// Returns the current owner.
    #[cbor(rename = "owner")]
    Owner,
}

#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub enum Response {
    /// Returned as a result of an `Owner` query. `None` if ownership has been renounced.
    #[cbor(rename = "owner")]
    Owner(Option<Address>),

    #[cbor(rename = "empty")]
    Empty,
}

impl From<()> for Response {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,

    #[error("permission denied")]
    #[sdk_error(code = 2)]
    PermissionDenied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
#[sdk_event(module_name = "ownable")]
pub enum Event {
    #[sdk_event(code = 1)]
    OwnershipTransferred {
        previous_owner: Address,
        /// The new owner, or `None` if ownership has been renounced.
        new_owner: Option<Address>,
    },
}
