use std::{marker::PhantomData, str::FromStr};

use ethabi::{ParamType, Token};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitSucceed,
};
use oasis_runtime_sdk::{
    modules::accounts::{Error, API as AccountsAPI},
    state::CurrentState,
    storage,
    types::{address, token},
};
use oasis_runtime_sdk_macros::{evm_contract_address, evm_method, sdk_derive, EvmEvent};
use primitive_types::H160;

use crate::precompile::{contract::EvmEvent as _, PrecompileResult};

const MODULE_NAME: &str = "evm/erc20";
const STATE_ALLOWANCES: &[u8] = &[0x01];

/// The gas costs for the various ERC-20 token functions.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct TokenOperationCosts {
    name: u64,
    symbol: u64,
    decimals: u64,
    total_supply: u64,
    balace_of: u64,
    transfer: u64,
    transfer_from: u64,
    approve: u64,
    allowance: u64,
    mint: u64,
    burn: u64,
}

impl TokenOperationCosts {
    pub const fn new() -> Self {
        Self {
            name: 0,
            symbol: 0,
            decimals: 0,
            total_supply: 0,
            balace_of: 0,
            transfer: 0,
            transfer_from: 0,
            approve: 0,
            allowance: 0,
            mint: 0,
            burn: 0,
        }
    }
}

/// Static ERC-20 token implementations should implement this trait to provide
/// token-specific configuration to the static contract framework.
pub trait Erc20Token {
    const GAS_COSTS: TokenOperationCosts;

    const NAME: &str;
    const SYMBOL: &str;
    const DECIMALS: u8;

    fn address() -> H160;

    fn total_supply() -> Result<u128, Error>;
    fn balance_of(account: &H160) -> Result<u128, Error>;
    fn transfer(sender: &H160, recipient: &H160, amount: &token::BaseUnits) -> Result<bool, Error>;
    fn transfer_from(
        owner: &H160,
        spender: &H160,
        recipient: &H160,
        amount: &token::BaseUnits,
    ) -> Result<bool, Error>;
    fn approve(owner: &H160, spender: &H160, amount: &token::BaseUnits) -> Result<bool, Error>;
    fn allowance(owner: &H160, spender: &H160) -> Result<u128, Error>;
    fn mint(caller: &H160, to: &H160, amount: &token::BaseUnits) -> Result<(), Error>;
    fn burn(caller: &H160, from: &H160, amount: &token::BaseUnits) -> Result<(), Error>;
}

pub trait AccountToken {
    type Accounts: AccountsAPI;

    const ADDRESS: H160;

    const GAS_COSTS: TokenOperationCosts;

    const NAME: &str;
    const SYMBOL: &str;
    const DECIMALS: u8;

    fn is_minting_allowed(caller: &H160, address: &H160) -> Result<bool, Error>;
    fn is_burning_allowed(caller: &H160, address: &H160) -> Result<bool, Error>;
}

fn with_allowance<T, F>(owner: &H160, spender: &H160, f: F) -> Result<u128, Error>
where
    T: AccountToken,
    F: FnOnce(Option<u128>) -> Result<u128, Error>,
{
    let owner = owner.as_bytes();
    let spender = spender.as_bytes();
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let allowances = storage::PrefixStore::new(store, &STATE_ALLOWANCES);
        let allowances = storage::PrefixStore::new(allowances, &T::NAME);
        let mut owner = storage::TypedStore::new(storage::PrefixStore::new(allowances, &owner));
        let old_allowance = owner.get(spender);
        let new_allowance = f(old_allowance)?;
        if new_allowance == 0 {
            owner.remove(spender);
        } else {
            owner.insert(spender, new_allowance);
        }
        Ok(old_allowance.unwrap_or_default())
    })
}

impl<T: AccountToken> Erc20Token for T {
    const GAS_COSTS: TokenOperationCosts = T::GAS_COSTS;

    const NAME: &str = T::NAME;
    const SYMBOL: &str = T::SYMBOL;
    const DECIMALS: u8 = T::DECIMALS;

    fn address() -> H160 {
        Self::ADDRESS
    }

    fn total_supply() -> Result<u128, Error> {
        let denom = token::Denomination::from_str(Self::SYMBOL).unwrap();
        Ok(T::Accounts::get_total_supplies()?
            .get(&denom)
            .copied()
            .unwrap_or_default())
    }

    fn balance_of(account: &H160) -> Result<u128, Error> {
        let account = address::Address::from_eth(account.as_bytes());
        let denom = token::Denomination::from_str(Self::SYMBOL).unwrap();
        T::Accounts::get_balance(account, denom)
    }

    fn transfer(sender: &H160, recipient: &H160, amount: &token::BaseUnits) -> Result<bool, Error> {
        let sender = address::Address::from_eth(sender.as_bytes());
        let recipient = address::Address::from_eth(recipient.as_bytes());
        T::Accounts::transfer(sender, recipient, amount).map(|_| true)
    }

    fn transfer_from(
        owner: &H160,
        spender: &H160,
        recipient: &H160,
        amount: &token::BaseUnits,
    ) -> Result<bool, Error> {
        with_allowance::<Self, _>(owner, spender, |allowance| {
            let allowance = allowance.unwrap_or_default();
            if amount.amount() <= allowance {
                Ok(allowance - amount.amount())
            } else {
                Err(Error::InsufficientBalance)
            }
        })?;
        let owner = address::Address::from_eth(owner.as_bytes());
        let recipient = address::Address::from_eth(recipient.as_bytes());
        T::Accounts::transfer(owner, recipient, amount).map(|_| true)
    }

    fn approve(owner: &H160, spender: &H160, amount: &token::BaseUnits) -> Result<bool, Error> {
        with_allowance::<Self, _>(owner, spender, |_allowance| Ok(amount.amount())).map(|_| {
            // Don't care what the old value is, the change was successful.
            true
        })
    }

    fn allowance(owner: &H160, spender: &H160) -> Result<u128, Error> {
        with_allowance::<Self, _>(
            owner,
            spender,
            |allowance| Ok(allowance.unwrap_or_default()),
        )
    }

    fn mint(caller: &H160, to: &H160, amount: &token::BaseUnits) -> Result<(), Error> {
        if Self::is_minting_allowed(caller, to)? {
            let to = address::Address::from_eth(to.as_bytes());
            T::Accounts::mint(to, amount)
        } else {
            Err(Error::Forbidden)
        }
    }

    fn burn(caller: &H160, from: &H160, amount: &token::BaseUnits) -> Result<(), Error> {
        if Self::is_burning_allowed(caller, from)? {
            let from = address::Address::from_eth(from.as_bytes());
            T::Accounts::burn(from, amount)
        } else {
            Err(Error::Forbidden)
        }
    }
}

#[derive(EvmEvent)]
#[evm_event(name = "Transfer")]
struct TransferEvent {
    #[evm_event(arg_type = "address", indexed)]
    from: Token,
    #[evm_event(arg_type = "address", indexed)]
    to: Token,
    #[evm_event(arg_type = "uint256")]
    value: Token,
}

#[derive(EvmEvent)]
#[evm_event(name = "Approval")]
struct ApprovalEvent {
    #[evm_event(arg_type = "address", indexed)]
    owner: Token,
    #[evm_event(arg_type = "address", indexed)]
    spender: Token,
    #[evm_event(arg_type = "uint256")]
    value: Token,
}

#[derive(Default)]
pub struct Erc20Contract<T> {
    _phantom_data: PhantomData<T>,
}

#[sdk_derive(EvmContract)]
impl<T: Erc20Token> Erc20Contract<T> {
    fn sdk_amount(amount_token: &Token) -> Result<token::BaseUnits, PrecompileFailure> {
        let amount = amount_token.clone().into_uint().unwrap();
        if amount >= u128::MAX.into() {
            Err(PrecompileFailure::Error {
                exit_status: ExitError::Other("amount overflow".into()),
            })
        } else {
            Ok(token::BaseUnits::new(
                amount.as_u128(),
                token::Denomination::from_str(T::SYMBOL).unwrap(),
            ))
        }
    }

    fn decode_params(types: &[ParamType], input: &[u8]) -> Result<Vec<Token>, PrecompileFailure> {
        ethabi::decode(types, input).map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("invalid parameter".into()),
        })
    }

    #[evm_contract_address]
    fn address() -> H160 {
        T::address()
    }

    #[evm_method(signature = "name()")]
    fn name(handle: &mut impl PrecompileHandle, _input_offset: usize) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.name)?;
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[Token::String(T::NAME.to_string())]),
        })
    }

    #[evm_method(signature = "symbol()")]
    fn symbol(handle: &mut impl PrecompileHandle, _input_offset: usize) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.symbol)?;
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[Token::String(T::SYMBOL.to_string())]),
        })
    }

    #[evm_method(signature = "decimals()")]
    fn decimals(handle: &mut impl PrecompileHandle, _input_offset: usize) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.decimals)?;
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[Token::Uint(T::DECIMALS.into())]),
        })
    }

    #[evm_method(signature = "totalSupply()")]
    fn total_supply(handle: &mut impl PrecompileHandle, _input_offset: usize) -> PrecompileResult {
        match T::total_supply() {
            Ok(amount) => {
                handle.record_cost(T::GAS_COSTS.total_supply)?;
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "balanceOf(address)")]
    fn balance_of(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(&[ParamType::Address], &handle.input()[input_offset..])?;
        let address = params[0].clone().into_address().unwrap();
        match T::balance_of(&address) {
            Ok(amount) => {
                handle.record_cost(T::GAS_COSTS.balace_of)?;
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "transfer(address,uint256)")]
    fn transfer(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Uint(256)],
            &handle.input()[input_offset..],
        )?;
        let recipient = params[0].clone().into_address().unwrap();
        let amount = Self::sdk_amount(&params[1])?;
        let sender = handle.context().caller;
        match T::transfer(&sender, &recipient, &amount) {
            Ok(done) => {
                handle.record_cost(T::GAS_COSTS.transfer)?;
                TransferEvent {
                    from: Token::Address(handle.context().caller),
                    to: params[0].clone(),
                    value: params[1].clone(),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "transferFrom(address,address,uint256)")]
    fn transfer_from(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Address, ParamType::Uint(256)],
            &handle.input()[input_offset..],
        )?;
        let caller = handle.context().caller;
        let owner = params[0].clone().into_address().unwrap();
        let recipient = params[1].clone().into_address().unwrap();
        let amount = Self::sdk_amount(&params[2])?;
        match T::transfer_from(&owner, &caller, &recipient, &amount) {
            Ok(done) => {
                handle.record_cost(T::GAS_COSTS.transfer)?;
                TransferEvent {
                    from: params[0].clone(),
                    to: params[1].clone(),
                    value: params[2].clone(),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "approve(address,uint256)")]
    fn approve(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Uint(256)],
            &handle.input()[input_offset..],
        )?;
        let owner = handle.context().caller;
        let spender = params[0].clone().into_address().unwrap();
        let amount = Self::sdk_amount(&params[1])?;
        match T::approve(&owner, &spender, &amount) {
            Ok(done) => {
                handle.record_cost(T::GAS_COSTS.approve)?;
                ApprovalEvent {
                    owner: Token::Address(owner),
                    spender: params[0].clone(),
                    value: params[1].clone(),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "allowance(address,address)")]
    fn allowance(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Address],
            &handle.input()[input_offset..],
        )?;
        let owner = params[0].clone().into_address().unwrap();
        let spender = params[1].clone().into_address().unwrap();
        match T::allowance(&owner, &spender) {
            Ok(amount) => {
                handle.record_cost(T::GAS_COSTS.allowance)?;
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "mint(address,uint256)")]
    fn mint(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Uint(256)],
            &handle.input()[input_offset..],
        )?;
        let caller = handle.context().caller;
        let to = params[0].clone().into_address().unwrap();
        let amount = Self::sdk_amount(&params[1])?;
        match T::mint(&caller, &to, &amount) {
            Ok(_) => {
                handle.record_cost(T::GAS_COSTS.mint)?;
                TransferEvent {
                    from: Token::Address(H160::zero()),
                    to: params[0].clone(),
                    value: params[1].clone(),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "burn(address,uint256)")]
    fn burn(handle: &mut impl PrecompileHandle, input_offset: usize) -> PrecompileResult {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Uint(256)],
            &handle.input()[input_offset..],
        )?;
        let caller = handle.context().caller;
        let from = params[0].clone().into_address().unwrap();
        let amount = Self::sdk_amount(&params[1])?;
        match T::burn(&caller, &from, &amount) {
            Ok(_) => {
                handle.record_cost(T::GAS_COSTS.burn)?;
                TransferEvent {
                    from: params[0].clone(),
                    to: Token::Address(H160::zero()),
                    value: params[1].clone(),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                })
            }
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }
}
