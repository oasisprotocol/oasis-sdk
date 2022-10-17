//! An OAS20 contract.
extern crate alloc;

pub mod helpers;
pub mod types;

use oasis_contract_sdk::{self as sdk, types::message::Reply};
use oasis_contract_sdk_storage::{cell::PublicCell, map::PublicMap};
use oasis_contract_sdk_types::address::Address;

use types::{Error, Event, Request, Response};

/// The contract type.
pub struct Oas20Token;

/// Storage cell for the token information.
const TOKEN_INFO: PublicCell<types::TokenInformation> = PublicCell::new(b"token_info");
/// Storage map for account balances.
const BALANCES: PublicMap<Address, u128> = PublicMap::new(b"balances");
/// Storage map for allowances.
const ALLOWANCES: PublicMap<(Address, Address), u128> = PublicMap::new(b"allowances");

// Implementation of the sdk::Contract trait is required in order for the type to be a contract.
impl sdk::Contract for Oas20Token {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        // This method is called during the contracts.Instantiate call when the contract is first
        // instantiated. It can be used to initialize the contract state.
        match request {
            // We require the caller to always pass the Instantiate request.
            Request::Instantiate(token_instantiation) => {
                let token_information =
                    helpers::instantiate(ctx, BALANCES, TOKEN_INFO, token_instantiation)?;

                ctx.emit_event(Event::Oas20Instantiated { token_information });

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        helpers::handle_call(ctx, TOKEN_INFO, BALANCES, ALLOWANCES, request)
    }

    fn query<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        helpers::handle_query(ctx, TOKEN_INFO, BALANCES, ALLOWANCES, request)
    }

    fn handle_reply<C: sdk::Context>(ctx: &mut C, reply: Reply) -> Result<Option<Response>, Error> {
        helpers::handle_reply(ctx, TOKEN_INFO, BALANCES, ALLOWANCES, reply)
    }
}

// Create the required WASM exports required for the contract to be runnable.
sdk::create_contract!(Oas20Token);

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

        let token_instantiation = types::TokenInstantiation {
            name: "TEST".to_string(),
            symbol: "TST".to_string(),
            decimals: 8,
            initial_balances: Vec::new(),
            minting: Some(types::MintingInformation {
                cap: Some(100_000),
                minter: bob.into(),
            }),
        };
        // Instantiate the contract.
        Oas20Token::instantiate(&mut ctx, Request::Instantiate(token_instantiation.clone()))
            .expect("instantiation should work");

        let ti = token_instantiation.clone();
        let rsp = Oas20Token::query(&mut ctx, Request::TokenInformation)
            .expect("token information query should work");
        assert_eq!(
            rsp,
            Response::TokenInformation {
                token_information: types::TokenInformation {
                    name: ti.name,
                    symbol: ti.symbol,
                    decimals: ti.decimals,
                    minting: ti.minting,
                    total_supply: 0,
                }
            },
            "token information query response should be correct"
        );

        let rsp = Oas20Token::query(
            &mut ctx,
            Request::Balance {
                address: alice.into(),
            },
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            Response::Balance { balance: 0 },
            "token balance query response should be correct"
        );

        // Try to transfer some tokens.
        ctx.ec.caller_address = bob.into();
        Oas20Token::call(
            &mut ctx,
            Request::Transfer {
                amount: 5,
                to: alice.into(),
            },
        )
        .expect_err("transfer empty balances should fail");

        // Mint tokens by non-minter should fail.
        ctx.ec.caller_address = alice.into();
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 10,
                to: bob.into(),
            },
        )
        .expect_err("minting by non-minter should fail");

        // Minting zero tokens should fail.
        ctx.ec.caller_address = bob.into();
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 0,
                to: bob.into(),
            },
        )
        .expect_err("minting zero tokens should fail");

        // Minting more than cap should fail.
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 100_000_000,
                to: bob.into(),
            },
        )
        .expect_err("minting over cap should fail");

        // Mint some tokens as minter.
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 10,
                to: bob.into(),
            },
        )
        .expect("minting should work");

        // Minting should overflow.
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: u128::MAX,
                to: bob.into(),
            },
        )
        .expect_err("minting should overflow");

        // Transfering zero amount should fail.
        ctx.ec.caller_address = bob.into();
        Oas20Token::call(
            &mut ctx,
            Request::Transfer {
                amount: 0,
                to: charlie.into(),
            },
        )
        .expect_err("transfer of zero tokens should fail");

        // Transfering more than available tokens should fail.
        Oas20Token::call(
            &mut ctx,
            Request::Transfer {
                amount: 100_000,
                to: charlie.into(),
            },
        )
        .expect_err("transfer of more tokens than available should fail");

        // Transfer some tokens.
        Oas20Token::call(
            &mut ctx,
            Request::Transfer {
                amount: 4,
                to: charlie.into(),
            },
        )
        .expect("transfer of tokens should work");

        // Burning zero tokens should fail.
        Oas20Token::call(&mut ctx, Request::Burn { amount: 0 })
            .expect_err("burning of zero tokens should fail");

        // Burning more than available tokens should fail.
        Oas20Token::call(&mut ctx, Request::Burn { amount: 100_000 })
            .expect_err("burning more than available tokens should fail");

        // Burn some tokens.
        Oas20Token::call(&mut ctx, Request::Burn { amount: 1 })
            .expect("burning of tokens should work");

        // Sending some tokens should work.
        Oas20Token::call(
            &mut ctx,
            Request::Send {
                amount: 1,
                to: 0.into(),
                data: cbor::to_value("test"),
            },
        )
        .expect("transfer of tokens should work");

        // Query balances.
        let rsp = Oas20Token::query(
            &mut ctx,
            Request::Balance {
                address: alice.into(),
            },
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            Response::Balance { balance: 0 },
            "token balance query response should be correct"
        );
        let rsp = Oas20Token::query(
            &mut ctx,
            Request::Balance {
                address: bob.into(),
            },
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            Response::Balance { balance: 4 },
            "token balance query response should be correct"
        );
        let rsp = Oas20Token::query(
            &mut ctx,
            Request::Balance {
                address: charlie.into(),
            },
        )
        .expect("token balance query should work");
        assert_eq!(
            rsp,
            Response::Balance { balance: 4 },
            "token balance query response should be correct"
        );

        // Total supply should be updated.
        let rsp = Oas20Token::query(&mut ctx, Request::TokenInformation)
            .expect("token information query should work");
        assert_eq!(
            rsp,
            Response::TokenInformation {
                token_information: types::TokenInformation {
                    name: token_instantiation.name,
                    symbol: token_instantiation.symbol,
                    decimals: token_instantiation.decimals,
                    minting: token_instantiation.minting,
                    total_supply: 9,
                }
            },
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

        let token_instantiation = types::TokenInstantiation {
            name: "TEST".to_string(),
            symbol: "TST".to_string(),
            decimals: 8,
            initial_balances: vec![
                types::InitialBalance {
                    address: alice,
                    amount: 100,
                },
                types::InitialBalance {
                    address: bob,
                    amount: 10,
                },
                types::InitialBalance {
                    address: charlie,
                    amount: 1,
                },
            ],
            minting: None,
        };

        // Instantiate the contract.
        Oas20Token::instantiate(&mut ctx, Request::Instantiate(token_instantiation.clone()))
            .expect("instantiation should work");

        // Minting should not be allowed.
        ctx.ec.caller_address = alice.into();
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 10,
                to: bob.into(),
            },
        )
        .expect_err("minting should not work");

        // Allowing zero amount should fail.
        Oas20Token::call(
            &mut ctx,
            Request::Allow {
                beneficiary: bob,
                negative: false,
                amount_change: 0,
            },
        )
        .expect_err("allowing of zero amount should fail");

        // Same allower and beneficiary should fail.
        ctx.ec.caller_address = alice.into();
        Oas20Token::call(
            &mut ctx,
            Request::Allow {
                beneficiary: alice,
                negative: false,
                amount_change: 10,
            },
        )
        .expect_err("allowing to self should fail");

        // Allowing should work.
        Oas20Token::call(
            &mut ctx,
            Request::Allow {
                beneficiary: bob,
                negative: false,
                amount_change: 100_000,
            },
        )
        .expect("allowing should work");

        // Reducing allowance should work.
        Oas20Token::call(
            &mut ctx,
            Request::Allow {
                beneficiary: bob,
                negative: true,
                amount_change: 1,
            },
        )
        .expect("allowing should work");

        // Withdrawing zero amount should fail.
        ctx.ec.caller_address = bob.into();
        Oas20Token::call(
            &mut ctx,
            Request::Withdraw {
                from: alice,
                amount: 0,
            },
        )
        .expect_err("withdrawing zero amount should fail");

        Oas20Token::call(
            &mut ctx,
            Request::Withdraw {
                from: bob,
                amount: 0,
            },
        )
        .expect_err("withdrawing from self should fail");

        // Withdrawing more than available balance should fail.
        Oas20Token::call(
            &mut ctx,
            Request::Withdraw {
                from: alice,
                amount: 500,
            },
        )
        .expect_err("withdrawing should fail");

        // Withdrawing should work.
        Oas20Token::call(
            &mut ctx,
            Request::Withdraw {
                from: alice,
                amount: 2,
            },
        )
        .expect("withdrawing should work");

        // Query allowance.
        let rsp = Oas20Token::query(
            &mut ctx,
            Request::Allowance {
                allower: alice,
                beneficiary: bob,
            },
        )
        .expect("token allowance query should work");
        assert_eq!(
            rsp,
            Response::Allowance { allowance: 99997 },
            "token allowance query response should be correct"
        );
    }
}
