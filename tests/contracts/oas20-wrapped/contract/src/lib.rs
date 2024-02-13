//! Oasis Wrapped-OAS20 contract, to be used by the Oasis Wormhole Token Bridge.
//!
//! This is based on the `contract-sdk/specs/oas20` contract with added support
//! for minter/owner to burn tokens from any account.
#![feature(wasm_abi)]

extern crate alloc;

use oasis_contract_sdk::{self as sdk};
use oasis_contract_sdk_oas20_types as oas20;
use oasis_contract_sdk_storage::{cell::Cell, map::Map};
use oasis_contract_sdk_types::address::Address;

use oasis_oas20_wrapped_types as types;
use types::{Error, Event, Request, Response};

/// The contract type.
pub struct Oas20Wrapped;

/// Storage cell for the token information.
const TOKEN_INFO: Cell<oas20::TokenInformation> = Cell::new(b"token_info");
/// Storage map for account balances.
const BALANCES: Map<Address, u128> = Map::new(b"balances");
/// Storage map for allowances.
const ALLOWANCES: Map<(Address, Address), u128> = Map::new(b"allowances");
/// Storage cell for wormhole bridge wrapped token information.
const WRAPPED_INFO: Cell<types::BridgeWrappedInfo> = Cell::new(b"wrapped_info");

impl sdk::Contract for Oas20Wrapped {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        match request {
            Request::Instantiate {
                token_instantiation,
                asset_chain_id,
                asset_address,
            } => {
                // Ensure an owner (minter) is set, otherwise a wrapped token doesn't make sense.
                if token_instantiation.minting.is_none() {
                    return Err(Error::MinterNotConfigured);
                }

                let token_information =
                    oas20::helpers::instantiate(ctx, BALANCES, TOKEN_INFO, token_instantiation)?;

                let wrapped_info = types::BridgeWrappedInfo {
                    asset_chain_id,
                    asset_address,
                    bridge_address: ctx.caller_address().to_owned(),
                };
                WRAPPED_INFO.set(ctx.public_store(), wrapped_info.clone());

                ctx.emit_event(Event::WrappedOas20Instantiated {
                    token_information,
                    wrapped_info,
                });

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Call call. It is supposed to handle the request
        // and return a response.
        match request {
            // Handle basic Oas20 requests.
            Request::Oas20(request) => Ok(oas20::helpers::handle_call(
                ctx, TOKEN_INFO, BALANCES, ALLOWANCES, request,
            )?
            .into()),
            // Handle owner burning request.
            Request::BurnFrom { from, amount } => {
                if amount == 0 {
                    return Err(oas20::Error::ZeroAmount.into());
                }

                let token_info = TOKEN_INFO.get(ctx.public_store()).unwrap();
                // Ensure caller is the minter.
                if &token_info.minting.unwrap().minter != ctx.caller_address() {
                    return Err(Error::BurningForbidden);
                }

                oas20::helpers::burn(ctx, BALANCES, TOKEN_INFO, from, amount)?;

                Ok(Response::Empty)
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Query query. It is supposed to handle the
        // request and return a response.
        match request {
            // Handle basic Oas20 requests.
            Request::Oas20(request) => Ok(oas20::helpers::handle_query(
                ctx, TOKEN_INFO, BALANCES, ALLOWANCES, request,
            )?
            .into()),
            Request::BridgeWrappedInfo => Ok(Response::BridgeWrappedInfo {
                info: WRAPPED_INFO.get(ctx.public_store()).unwrap(),
            }),
            _ => Err(Error::BadRequest),
        }
    }
}

// Create the required WASM exports required for the contract to be runnable.
sdk::create_contract!(Oas20Wrapped);

// We define some simple contract tests below.
#[cfg(test)]
mod test {
    use oasis_contract_sdk::{testing::MockContext, types::ExecutionContext, Contract};
    use oasis_contract_sdk_types::testing::addresses;

    use super::*;

    #[test]
    fn test_basics() {
        // Create a mock execution context with default values.
        let mut ctx: MockContext = ExecutionContext::default().into();

        let alice = addresses::alice::address();
        let bob = addresses::bob::address();
        let charlie = addresses::charlie::address();

        let mut token_instantiation = oas20::TokenInstantiation {
            name: "TEST".to_string(),
            symbol: "TST".to_string(),
            decimals: 8,
            initial_balances: Vec::new(),
            minting: None,
        };
        ctx.ec.caller_address = alice.into();

        // Instantiate without a minter should fail.
        Oas20Wrapped::instantiate(
            &mut ctx,
            Request::Instantiate {
                token_instantiation: token_instantiation.clone(),
                asset_chain_id: 1,
                asset_address: alice.into(),
            },
        )
        .expect_err("instantiation without a minter should fail");

        // Instantiate the contract.
        token_instantiation.minting = Some(oas20::MintingInformation {
            minter: alice.into(),
            cap: None,
        });
        Oas20Wrapped::instantiate(
            &mut ctx,
            Request::Instantiate {
                token_instantiation: token_instantiation.clone(),
                asset_chain_id: 1,
                asset_address: alice.into(),
            },
        )
        .expect("instantiation should work");

        let ti = token_instantiation.clone();
        let rsp = Oas20Wrapped::query(&mut ctx, oas20::Request::TokenInformation.into())
            .expect("token information query should work");
        assert_eq!(
            rsp,
            oas20::Response::TokenInformation {
                token_information: oas20::TokenInformation {
                    name: ti.name,
                    symbol: ti.symbol,
                    decimals: ti.decimals,
                    minting: ti.minting,
                    total_supply: 0,
                }
            }
            .into(),
            "token information query response should be correct"
        );

        let rsp = Oas20Wrapped::query(
            &mut ctx,
            oas20::Request::Balance {
                address: alice.into(),
            }
            .into(),
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            oas20::Response::Balance { balance: 0 }.into(),
            "token balance query response should be correct"
        );

        // Try to transfer some tokens.
        ctx.ec.caller_address = bob.into();
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Transfer {
                amount: 5,
                to: alice.into(),
            }
            .into(),
        )
        .expect_err("transfer empty balances should fail");

        // Mint tokens by non-minter should fail.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Mint {
                amount: 10,
                to: bob.into(),
            }
            .into(),
        )
        .expect_err("minting by non-minter should fail");

        // Minting zero tokens should fail.
        ctx.ec.caller_address = alice.into();
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Mint {
                amount: 0,
                to: bob.into(),
            }
            .into(),
        )
        .expect_err("minting zero tokens should fail");

        // Mint some tokens as minter.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Mint {
                amount: 10,
                to: bob.into(),
            }
            .into(),
        )
        .expect("minting should work");

        // Minting should overflow.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Mint {
                amount: u128::MAX,
                to: bob.into(),
            }
            .into(),
        )
        .expect_err("minting should overflow");

        // Transfering zero amount should fail.
        ctx.ec.caller_address = bob.into();
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Transfer {
                amount: 0,
                to: charlie.into(),
            }
            .into(),
        )
        .expect_err("transfer of zero tokens should fail");

        // Transfering more than available tokens should fail.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Transfer {
                amount: 100_000,
                to: charlie.into(),
            }
            .into(),
        )
        .expect_err("transfer of more tokens than available should fail");

        // Transfer some tokens.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Transfer {
                amount: 4,
                to: charlie.into(),
            }
            .into(),
        )
        .expect("transfer of tokens should work");

        // Burning zero tokens should fail.
        Oas20Wrapped::call(&mut ctx, oas20::Request::Burn { amount: 0 }.into())
            .expect_err("burning of zero tokens should fail");

        // Burning more than available tokens should fail.
        Oas20Wrapped::call(&mut ctx, oas20::Request::Burn { amount: 100_000 }.into())
            .expect_err("burning more than available tokens should fail");

        // Burn some tokens.
        Oas20Wrapped::call(&mut ctx, oas20::Request::Burn { amount: 1 }.into())
            .expect("burning of tokens should work");

        // Sending some tokens should work.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Send {
                amount: 1,
                to: 0.into(),
                data: cbor::to_value("test"),
            }
            .into(),
        )
        .expect("transfer of tokens should work");

        // Burn from.
        Oas20Wrapped::call(
            &mut ctx,
            Request::BurnFrom {
                from: bob.into(),
                amount: 1,
            },
        )
        .expect_err("burning as non-minter should not work");

        ctx.ec.caller_address = alice.into();
        Oas20Wrapped::call(
            &mut ctx,
            Request::BurnFrom {
                from: bob.into(),
                amount: 1,
            },
        )
        .expect("burning as the minter should work");

        // Query balances.
        let rsp = Oas20Wrapped::query(
            &mut ctx,
            oas20::Request::Balance {
                address: alice.into(),
            }
            .into(),
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            oas20::Response::Balance { balance: 0 }.into(),
            "token balance query response should be correct"
        );
        let rsp = Oas20Wrapped::query(
            &mut ctx,
            oas20::Request::Balance {
                address: bob.into(),
            }
            .into(),
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            oas20::Response::Balance { balance: 3 }.into(),
            "token balance query response should be correct"
        );
        let rsp = Oas20Wrapped::query(
            &mut ctx,
            oas20::Request::Balance {
                address: charlie.into(),
            }
            .into(),
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            oas20::Response::Balance { balance: 4 }.into(),
            "token balance query response should be correct"
        );

        // Total supply should be updated.
        let rsp = Oas20Wrapped::query(&mut ctx, oas20::Request::TokenInformation.into())
            .expect("token information query should work");
        assert_eq!(
            rsp,
            oas20::Response::TokenInformation {
                token_information: oas20::TokenInformation {
                    name: token_instantiation.name,
                    symbol: token_instantiation.symbol,
                    decimals: token_instantiation.decimals,
                    minting: token_instantiation.minting,
                    total_supply: 8,
                }
            }
            .into(),
            "token information query response should be correct"
        );
    }

    #[test]
    fn test_allowances() {
        // Create a mock execution context with default values.
        let mut ctx: MockContext = ExecutionContext::default().into();

        let alice = addresses::alice::address();
        let bob = addresses::bob::address();
        let charlie = addresses::charlie::address();

        let token_instantiation = oas20::TokenInstantiation {
            name: "TEST".to_string(),
            symbol: "TST".to_string(),
            decimals: 8,
            initial_balances: vec![
                oas20::InitialBalance {
                    address: alice,
                    amount: 100,
                },
                oas20::InitialBalance {
                    address: bob,
                    amount: 10,
                },
                oas20::InitialBalance {
                    address: charlie,
                    amount: 1,
                },
            ],
            minting: Some(oas20::MintingInformation {
                cap: None,
                minter: alice.into(),
            }),
        };

        // Instantiate the contract.
        ctx.ec.caller_address = alice.into();
        Oas20Wrapped::instantiate(
            &mut ctx,
            Request::Instantiate {
                token_instantiation: token_instantiation.clone(),
                asset_chain_id: 1,
                asset_address: alice.into(),
            },
        )
        .expect("instantiation should work");

        // Allowing zero amount should fail.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Allow {
                beneficiary: bob,
                negative: false,
                amount_change: 0,
            }
            .into(),
        )
        .expect_err("allowing of zero amount should fail");

        // Same allower and beneficiary should fail.
        ctx.ec.caller_address = alice.into();
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Allow {
                beneficiary: alice,
                negative: false,
                amount_change: 10,
            }
            .into(),
        )
        .expect_err("allowing to self should fail");

        // Allowing should work.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Allow {
                beneficiary: bob,
                negative: false,
                amount_change: 10,
            }
            .into(),
        )
        .expect("allowing should work");

        // Reducing allowance should work.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Allow {
                beneficiary: bob,
                negative: true,
                amount_change: 1,
            }
            .into(),
        )
        .expect("allowing should work");

        // Withdrawing zero amount should fail.
        ctx.ec.caller_address = bob.into();
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Withdraw {
                from: alice,
                amount: 0,
            }
            .into(),
        )
        .expect_err("withdrawing zero amount should fail");

        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Withdraw {
                from: bob,
                amount: 0,
            }
            .into(),
        )
        .expect_err("withdrawing from self should fail");

        // Withdrawing should work.
        Oas20Wrapped::call(
            &mut ctx,
            oas20::Request::Withdraw {
                from: alice,
                amount: 2,
            }
            .into(),
        )
        .expect("withdrawing should work");

        // Query allowance.
        let rsp = Oas20Wrapped::query(
            &mut ctx,
            oas20::Request::Allowance {
                allower: alice,
                beneficiary: bob,
            }
            .into(),
        )
        .expect("token allowance query should work");
        assert_eq!(
            rsp,
            oas20::Response::Allowance { allowance: 7 }.into(),
            "token allowance query response should be correct"
        );
    }
}
