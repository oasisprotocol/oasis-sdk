//! An OAS20 contract.
#![feature(wasm_abi)]

extern crate alloc;

use oasis_contract_sdk::{
    self as sdk,
    env::Env,
    types::message::{Message, NotifyReply},
};
use oasis_contract_sdk_oas20_types as types;
use oasis_contract_sdk_oas20_types::{Error, Event, Request, Response};
use oasis_contract_sdk_storage::{cell::Cell, map::Map};
use oasis_contract_sdk_types::address::Address;

/// The contract type.
pub struct Oas20Token;

/// Storage cell for the token information.
const TOKEN_INFO: Cell<types::TokenInformation> = Cell::new(b"token_info");
const BALANCES: Map<Address, u128> = Map::new(b"balances");
const ALLOWANCES: Map<(Address, Address), u128> = Map::new(b"allowances");

impl Oas20Token {
    /// Transfer the `amount` of funds from `from` to `to` address.
    fn transfer<C: sdk::Context>(
        ctx: &mut C,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<(), Error> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        let mut from_balance = BALANCES.get(ctx.public_store(), from).unwrap_or_default();
        let mut to_balance = BALANCES.get(ctx.public_store(), to).unwrap_or_default();

        from_balance = from_balance
            .checked_sub(amount)
            .ok_or(Error::InsufficientFunds)?;
        to_balance += amount;

        BALANCES.insert(ctx.public_store(), from, from_balance);
        BALANCES.insert(ctx.public_store(), to, to_balance);

        Ok(())
    }

    /// Burns the `amount` of funds from `from`.
    fn burn<C: sdk::Context>(ctx: &mut C, from: Address, amount: u128) -> Result<(), Error> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        // Remove from account balance.
        let mut from_balance = BALANCES.get(ctx.public_store(), from).unwrap_or_default();
        from_balance = from_balance
            .checked_sub(amount)
            .ok_or(Error::InsufficientFunds)?;

        // Decrease the supply.
        // Token info should always be present.
        let mut token_info = TOKEN_INFO.get(ctx.public_store()).unwrap();
        // Shouldn't ever overflow.
        token_info.total_supply = token_info.total_supply.checked_sub(amount).unwrap();

        BALANCES.insert(ctx.public_store(), from, from_balance);
        TOKEN_INFO.set(ctx.public_store(), token_info);

        Ok(())
    }

    /// Update the `beneficiary` allownace by the `amount`.
    fn allow<C: sdk::Context>(
        ctx: &mut C,
        allower: Address,
        beneficiary: Address,
        negative: bool,
        amount: u128,
    ) -> Result<(u128, u128), Error> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        if allower == beneficiary {
            return Err(Error::SameAllowerAndBeneficiary);
        }

        let allowance = ALLOWANCES
            .get(ctx.public_store(), (allower, beneficiary))
            .unwrap_or_default();

        let (new_allowance, change) = match negative {
            true => {
                let new = allowance.saturating_sub(amount);
                (new, allowance - new)
            }
            false => {
                let new = allowance.saturating_add(amount);
                (new, new - allowance)
            }
        };

        ALLOWANCES.insert(ctx.public_store(), (allower, beneficiary), new_allowance);

        Ok((new_allowance, change))
    }

    /// Withdraw the `amunt` of funds from `from` to `to`.
    fn withdraw<C: sdk::Context>(
        ctx: &mut C,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<(), Error> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        if from == to {
            return Err(Error::SameAllowerAndBeneficiary);
        }

        let mut allowance = ALLOWANCES
            .get(ctx.public_store(), (from, to))
            .unwrap_or_default();
        allowance = allowance
            .checked_sub(amount)
            .ok_or(Error::InsufficientAllowance)?;

        Self::transfer(ctx, from, to, amount)?;

        ALLOWANCES.insert(ctx.public_store(), (from, to), allowance);

        Ok(())
    }

    /// Mints the `amount` of tokens to `to`.
    fn mint<C: sdk::Context>(ctx: &mut C, to: Address, amount: u128) -> Result<(), Error> {
        if amount == 0 {
            return Err(Error::ZeroAmount);
        }

        // Token info should always be present.
        let mut token_info = TOKEN_INFO.get(ctx.public_store()).unwrap();
        // Ensure token supports minting and new supply cap is bellow mint cap.
        match token_info.minting.as_ref() {
            Some(info) => {
                let cap = info.cap.unwrap_or(u128::MAX);
                match token_info.total_supply.checked_add(amount) {
                    Some(new_cap) => {
                        if new_cap > cap {
                            return Err(Error::MintOverCap);
                        }
                    }
                    None => return Err(Error::TotalSupplyOverflow),
                }
                if &info.minter != ctx.caller_address() {
                    return Err(Error::MintingForbidden);
                }
            }
            None => return Err(Error::MintingForbidden),
        }

        // Add to account balance.
        let mut to_balance = BALANCES.get(ctx.public_store(), to).unwrap_or_default();
        // Cannot overflow due to the total supply overflow check above.
        to_balance = to_balance.checked_add(amount).unwrap();

        // Increase the supply.
        // Overflow already checked above.
        token_info.total_supply = token_info.total_supply.checked_add(amount).unwrap();

        BALANCES.insert(ctx.public_store(), to, to_balance);
        TOKEN_INFO.set(ctx.public_store(), token_info);

        Ok(())
    }
}

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
                // Setup initial balances and compute the total supply.
                let mut total_supply: u128 = 0;
                for types::InitialBalance { address, amount } in
                    token_instantiation.initial_balances
                {
                    total_supply = total_supply
                        .checked_add(amount)
                        .ok_or(Error::TotalSupplyOverflow)?;
                    BALANCES.insert(ctx.public_store(), address, amount);
                }

                let token_information = types::TokenInformation {
                    name: token_instantiation.name,
                    symbol: token_instantiation.symbol,
                    decimals: token_instantiation.decimals,
                    minting: token_instantiation.minting,
                    total_supply,
                };
                TOKEN_INFO.set(ctx.public_store(), token_information.clone());

                ctx.emit_event(Event::Oas20Instantiated { token_information });

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Call call. It is supposed to handle the request
        // and return a response.
        match request {
            Request::Transfer { to, amount } => {
                // Transfers the `amount` of funds from caller to `to` address.
                let from = ctx.caller_address().to_owned();
                Self::transfer(ctx, from, to, amount)?;

                ctx.emit_event(Event::Oas20Transferred { from, to, amount });

                Ok(Response::Empty)
            }
            Request::Send { to, amount, data } => {
                // Transfers the `amount` of funds from caller to `to` contract instande identifier
                // and calls `ReceiveOas20` on the receiving contract.

                // Transfers the `amount` of funds from caller to `to` address.
                let from = ctx.caller_address().to_owned();
                let to_address = ctx.env().address_for_instance(to);
                Self::transfer(ctx, from, to_address, amount)?;

                // There should be high-level helpers for calling methods of other contracts that follow a similar
                // "standard" API - maybe define an API and helper methods in an OAS-0 document.

                // Emit a message through which we instruct the runtime to make a call on the
                // contract's behalf
                use cbor::cbor_map;
                ctx.emit_message(Message::Call {
                    id: 0,
                    reply: NotifyReply::Never,
                    method: "contracts.Call".to_string(),
                    body: cbor::cbor_map! {
                        "id" => cbor::cbor_int!(to.as_u64() as i64),
                        "data" => cbor::cbor_bytes!(cbor::to_vec(
                            cbor::to_value(types::ReceiverRequest::Receive{sender: from, amount, data}),
                        )),
                        "tokens" => cbor::cbor_array![],
                    },
                    max_gas: None,
                });

                ctx.emit_event(Event::Oas20Sent { from, to, amount });

                Ok(Response::Empty)
            }
            Request::Burn { amount } => {
                let from = ctx.caller_address().to_owned();
                Self::burn(ctx, from, amount)?;

                ctx.emit_event(Event::Oas20Burned { from, amount });

                Ok(Response::Empty)
            }
            Request::Mint { to, amount } => {
                Self::mint(ctx, to, amount)?;

                ctx.emit_event(Event::Oas20Minted { to, amount });

                Ok(Response::Empty)
            }
            Request::Allow {
                beneficiary,
                negative,
                amount_change,
            } => {
                let owner = ctx.caller_address().to_owned();
                let (new_allowance, amount_change) =
                    Self::allow(ctx, owner, beneficiary, negative, amount_change)?;

                ctx.emit_event(Event::Oas20AllowanceChanged {
                    owner,
                    beneficiary,
                    allowance: new_allowance,
                    negative,
                    amount_change,
                });

                Ok(Response::Empty)
            }
            Request::Withdraw { from, amount } => {
                let to = ctx.caller_address().to_owned();
                Self::withdraw(ctx, from, to, amount)?;

                ctx.emit_event(Event::Oas20Withdraw { from, to, amount });

                Ok(Response::Empty)
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Query query. It is supposed to handle the
        // request and return a response.
        match request {
            Request::TokenInformation => {
                // Token info should always be present.
                let token_info = TOKEN_INFO.get(ctx.public_store()).unwrap();

                Ok(Response::TokenInformation {
                    token_information: token_info,
                })
            }
            Request::Balance { address } => Ok(Response::Balance {
                balance: BALANCES
                    .get(ctx.public_store(), address)
                    .unwrap_or_default(),
            }),
            Request::Allowance {
                allower,
                beneficiary,
            } => Ok(Response::Allowance {
                allowance: ALLOWANCES
                    .get(ctx.public_store(), (allower, beneficiary))
                    .unwrap_or_default(),
            }),
            _ => Err(Error::BadRequest),
        }
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
        .expect_err("minting should zero tokens should fail");

        // Minting more than cap should fail.
        Oas20Token::call(
            &mut ctx,
            Request::Mint {
                amount: 100_000_000,
                to: bob.into(),
            },
        )
        .expect_err("minting should zero tokens should fail");

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
                amount_change: 10,
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
            Response::Allowance { allowance: 7 },
            "token allowance query response should be correct"
        );
    }
}
