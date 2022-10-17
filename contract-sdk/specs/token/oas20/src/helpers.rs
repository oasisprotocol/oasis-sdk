use oasis_contract_sdk::{
    self as sdk,
    env::Env,
    types::{
        message::{CallResult, Message, NotifyReply, Reply},
        InstanceId,
    },
};
use oasis_contract_sdk_storage::{cell::PublicCell, map::PublicMap};
use oasis_contract_sdk_types::address::Address;

use crate::types::{
    Error, Event, InitialBalance, ReceiverRequest, Request, Response, TokenInformation,
    TokenInstantiation,
};

/// Unique identifier for the send subcall.
pub const CALL_ID_SEND: u64 = 1;

/// Handles an OAS20 request call.
pub fn handle_call<C: sdk::Context>(
    ctx: &mut C,
    token_info: PublicCell<TokenInformation>,
    balances: PublicMap<Address, u128>,
    allowances: PublicMap<(Address, Address), u128>,
    request: Request,
) -> Result<Response, Error> {
    match request {
        Request::Transfer { to, amount } => {
            // Transfers the `amount` of funds from caller to `to` address.
            let from = ctx.caller_address().to_owned();
            transfer(ctx, balances, from, to, amount)?;

            ctx.emit_event(Event::Oas20Transferred { from, to, amount });

            Ok(Response::Empty)
        }
        Request::Send { to, amount, data } => {
            let from = ctx.caller_address().to_owned();
            send(
                ctx,
                balances,
                from,
                to,
                amount,
                data,
                CALL_ID_SEND,
                NotifyReply::OnError, // Rollback if subcall fails.
            )?;

            ctx.emit_event(Event::Oas20Sent { from, to, amount });

            Ok(Response::Empty)
        }
        Request::Burn { amount } => {
            let from = ctx.caller_address().to_owned();
            burn(ctx, balances, token_info, from, amount)?;

            ctx.emit_event(Event::Oas20Burned { from, amount });

            Ok(Response::Empty)
        }
        Request::Mint { to, amount } => {
            mint(ctx, balances, token_info, to, amount)?;

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
                allow(ctx, allowances, owner, beneficiary, negative, amount_change)?;

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
            withdraw(ctx, balances, allowances, from, to, amount)?;

            ctx.emit_event(Event::Oas20Withdrew { from, to, amount });

            Ok(Response::Empty)
        }
        _ => Err(Error::BadRequest),
    }
}

/// Handles an OAS20 request query.
pub fn handle_query<C: sdk::Context>(
    ctx: &mut C,
    token_info: PublicCell<TokenInformation>,
    balances: PublicMap<Address, u128>,
    allowances: PublicMap<(Address, Address), u128>,
    request: Request,
) -> Result<Response, Error> {
    match request {
        Request::TokenInformation => {
            // Token info should always be present.
            let token_info = token_info.get(ctx.public_store()).unwrap();

            Ok(Response::TokenInformation {
                token_information: token_info,
            })
        }
        Request::Balance { address } => Ok(Response::Balance {
            balance: balances
                .get(ctx.public_store(), address)
                .unwrap_or_default(),
        }),
        Request::Allowance {
            allower,
            beneficiary,
        } => Ok(Response::Allowance {
            allowance: allowances
                .get(ctx.public_store(), (allower, beneficiary))
                .unwrap_or_default(),
        }),
        _ => Err(Error::BadRequest),
    }
}

/// Handles a reply from OAS20 execution.
pub fn handle_reply<C: sdk::Context>(
    _ctx: &mut C,
    _token_info: PublicCell<TokenInformation>,
    _balances: PublicMap<Address, u128>,
    _allowances: PublicMap<(Address, Address), u128>,
    reply: Reply,
) -> Result<Option<Response>, Error> {
    match reply {
        Reply::Call {
            id: CALL_ID_SEND,
            result: CallResult::Failed { module, code },
            ..
        } => Err(Error::ReceiverCallFailed(module, code)),
        _ => Ok(None),
    }
}

/// Instantiates the contract state.
pub fn instantiate<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    token_info: PublicCell<TokenInformation>,
    instantiation: TokenInstantiation,
) -> Result<TokenInformation, Error> {
    // Setup initial balances and compute the total supply.
    let mut total_supply: u128 = 0;
    for InitialBalance { address, amount } in instantiation.initial_balances {
        total_supply = total_supply
            .checked_add(amount)
            .ok_or(Error::TotalSupplyOverflow)?;
        balances.insert(ctx.public_store(), address, amount);
    }

    let token_information = TokenInformation {
        name: instantiation.name,
        symbol: instantiation.symbol,
        decimals: instantiation.decimals,
        minting: instantiation.minting,
        total_supply,
    };
    token_info.set(ctx.public_store(), token_information.clone());

    Ok(token_information)
}

/// Transfer the `amount` of funds from `from` to `to` address.
pub fn transfer<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    from: Address,
    to: Address,
    amount: u128,
) -> Result<(), Error> {
    if amount == 0 {
        return Err(Error::ZeroAmount);
    }

    let mut from_balance = balances.get(ctx.public_store(), from).unwrap_or_default();
    let mut to_balance = balances.get(ctx.public_store(), to).unwrap_or_default();

    from_balance = from_balance
        .checked_sub(amount)
        .ok_or(Error::InsufficientFunds)?;
    to_balance += amount;

    balances.insert(ctx.public_store(), from, from_balance);
    balances.insert(ctx.public_store(), to, to_balance);

    Ok(())
}

/// Burns the `amount` of funds from `from`.
pub fn burn<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    token_info: PublicCell<TokenInformation>,
    from: Address,
    amount: u128,
) -> Result<(), Error> {
    if amount == 0 {
        return Err(Error::ZeroAmount);
    }

    // Remove from account balance.
    let mut from_balance = balances.get(ctx.public_store(), from).unwrap_or_default();
    from_balance = from_balance
        .checked_sub(amount)
        .ok_or(Error::InsufficientFunds)?;

    // Decrease the supply.
    // Token info should always be present.
    let mut info = token_info.get(ctx.public_store()).unwrap();
    // Shouldn't ever overflow.
    info.total_supply = info.total_supply.checked_sub(amount).unwrap();

    balances.insert(ctx.public_store(), from, from_balance);
    token_info.set(ctx.public_store(), info);

    Ok(())
}

/// Mints the `amount` of tokens to `to`.
pub fn mint<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    token_info_cell: PublicCell<TokenInformation>,
    to: Address,
    amount: u128,
) -> Result<(), Error> {
    if amount == 0 {
        return Err(Error::ZeroAmount);
    }
    // Token info should always be present.
    let mut token_info = token_info_cell.get(ctx.public_store()).unwrap();
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
    let mut to_balance = balances.get(ctx.public_store(), to).unwrap_or_default();
    // Cannot overflow due to the total supply overflow check above.
    to_balance = to_balance.checked_add(amount).unwrap();

    // Increase the supply.
    // Overflow already checked above.
    token_info.total_supply = token_info.total_supply.checked_add(amount).unwrap();

    balances.insert(ctx.public_store(), to, to_balance);
    token_info_cell.set(ctx.public_store(), token_info);

    Ok(())
}

/// Transfers the `amount` of funds from caller to `to` contract instance identifier
/// and calls `ReceiveOas20` on the receiving contract.
#[allow(clippy::too_many_arguments)]
pub fn send<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    from: Address,
    to: InstanceId,
    amount: u128,
    data: cbor::Value,
    id: u64,
    notify: NotifyReply,
) -> Result<(), Error> {
    let to_address = ctx.env().address_for_instance(to);
    transfer(ctx, balances, from, to_address, amount)?;

    // There should be high-level helpers for calling methods of other contracts that follow a similar
    // "standard" API - maybe define an API and helper methods in an OAS-0 document.

    // Emit a message through which we instruct the runtime to make a call on the
    // contract's behalf.
    use cbor::cbor_map;
    ctx.emit_message(Message::Call {
        id,
        reply: notify,
        method: "contracts.Call".to_string(),
        body: cbor::cbor_map! {
            "id" => cbor::cbor_int!(to.as_u64() as i64),
            "data" => cbor::cbor_bytes!(cbor::to_vec(
                cbor::to_value(ReceiverRequest::Receive{sender: from, amount, data}),
            )),
            "tokens" => cbor::cbor_array![],
        },
        max_gas: None,
        data: None,
    });

    Ok(())
}

/// Update the `beneficiary` allowance by the `amount`.
pub fn allow<C: sdk::Context>(
    ctx: &mut C,
    allowances: PublicMap<(Address, Address), u128>,
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

    let allowance = allowances
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

    allowances.insert(ctx.public_store(), (allower, beneficiary), new_allowance);

    Ok((new_allowance, change))
}

/// Withdraw the `amount` of funds from `from` to `to`.
pub fn withdraw<C: sdk::Context>(
    ctx: &mut C,
    balances: PublicMap<Address, u128>,
    allowances: PublicMap<(Address, Address), u128>,
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

    let mut allowance = allowances
        .get(ctx.public_store(), (from, to))
        .unwrap_or_default();
    allowance = allowance
        .checked_sub(amount)
        .ok_or(Error::InsufficientAllowance)?;

    transfer(ctx, balances, from, to, amount)?;

    allowances.insert(ctx.public_store(), (from, to), allowance);

    Ok(())
}
