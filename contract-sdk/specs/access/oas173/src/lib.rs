//! Specification of an Ownable contract.
#![deny(rust_2018_idioms, unreachable_pub)]
#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]
#![forbid(unsafe_code)]

pub mod helpers;
pub mod types;

use oasis_contract_sdk as sdk;

use types::{Error, Request, Response};

struct Ownable;

impl sdk::Contract for Ownable {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        if let Request::Instantiate = request {
            helpers::instantiate(ctx)
        } else {
            Err(Error::BadRequest)
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        Ok(match request {
            Request::TransferOwnership { new_owner } => {
                helpers::transfer_ownership(ctx, new_owner)?.into()
            }
            Request::RenounceOwnership => helpers::renounce_ownership(ctx)?.into(),
            _ => return Err(Error::BadRequest),
        })
    }

    fn query<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        Ok(match request {
            Request::Owner => Response::Owner(helpers::owner(ctx)),
            _ => return Err(Error::BadRequest),
        })
    }
}

sdk::create_contract!(Ownable);
