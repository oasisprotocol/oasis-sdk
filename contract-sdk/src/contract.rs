//! The contract trait.
use crate::{context::Context, error, types};

/// Trait that needs to be implemented by contract implementations.
pub trait Contract {
    /// Type of all requests.
    type Request: cbor::Decode;
    /// Type of all responses.
    type Response: cbor::Encode;
    /// Type of all errors.
    type Error: error::Error;

    /// Instantiate the contract.
    fn instantiate<C: Context>(_ctx: &mut C, _request: Self::Request) -> Result<(), Self::Error> {
        // Default implementation doesn't do anything.
        Ok(())
    }

    /// Call the contract.
    fn call<C: Context>(ctx: &mut C, request: Self::Request)
        -> Result<Self::Response, Self::Error>;

    /// Query the contract.
    fn query<C: Context>(
        _ctx: &mut C,
        _request: Self::Request,
    ) -> Result<Self::Response, Self::Error>;

    /// Handle replies from sent messages.
    fn handle_reply<C: Context>(
        _ctx: &mut C,
        _reply: types::message::Reply,
    ) -> Result<Option<Self::Response>, Self::Error> {
        // Default implementation does not perform any processing.
        Ok(None)
    }

    /// Perform any pre-upgrade tasks. This method is called on the old contract code.
    ///
    /// If this method reports an error the upgrade will be aborted.
    fn pre_upgrade<C: Context>(_ctx: &mut C, _request: Self::Request) -> Result<(), Self::Error> {
        // Default implementation accepts all upgrades.
        Ok(())
    }

    /// Perform any post-upgrade tasks. This method is called on the new contract code.
    ///
    /// If this method reports an error the upgrade will be aborted.
    fn post_upgrade<C: Context>(_ctx: &mut C, _request: Self::Request) -> Result<(), Self::Error> {
        // Default implementation accepts all upgrades.
        Ok(())
    }
}
