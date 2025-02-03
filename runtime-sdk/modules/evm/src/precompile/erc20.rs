use std::{marker::PhantomData, str::FromStr};

use ethabi::{ParamType, Token};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileOutput},
    Context, ExitError, ExitSucceed,
};
use oasis_runtime_sdk::{
    modules::accounts::{Error, API as AccountsAPI},
    types::{address, token},
};
use oasis_runtime_sdk_macros::{evm_method, sdk_derive};

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

    fn total_supply() -> Result<u128, Error>;
    fn balance_of(account: address::Address) -> Result<u128, Error>;
    fn transfer(
        sender: address::Address,
        recipient: address::Address,
        amount: &token::BaseUnits,
    ) -> Result<bool, Error>;
    fn approve(
        owner: address::Address,
        spender: address::Address,
        amount: &token::BaseUnits,
    ) -> Result<bool, Error>;
    fn allowance(owner: address::Address, spender: address::Address) -> Result<u128, Error>;
    fn mint(to: address::Address, amount: &token::BaseUnits) -> Result<(), Error>;
    fn burn(from: address::Address, amount: &token::BaseUnits) -> Result<(), Error>;
}

pub trait AccountToken {
    type Accounts: AccountsAPI;

    const GAS_COSTS: TokenOperationCosts;

    const NAME: &str;
    const SYMBOL: &str;
    const DECIMALS: u8;
}

impl<T: AccountToken> Erc20Token for T {
    const GAS_COSTS: TokenOperationCosts = T::GAS_COSTS;

    const NAME: &str = T::NAME;
    const SYMBOL: &str = T::SYMBOL;
    const DECIMALS: u8 = T::DECIMALS;

    fn total_supply() -> Result<u128, Error> {
        let denom = token::Denomination::from_str(Self::SYMBOL).unwrap();
        Ok(T::Accounts::get_total_supplies()?
            .get(&denom)
            .copied()
            .unwrap_or_default())
    }

    fn balance_of(account: address::Address) -> Result<u128, Error> {
        let denom = token::Denomination::from_str(Self::SYMBOL).unwrap();
        T::Accounts::get_balance(account, denom)
    }

    fn transfer(
        sender: address::Address,
        recipient: address::Address,
        amount: &token::BaseUnits,
    ) -> Result<bool, Error> {
        T::Accounts::transfer(sender, recipient, amount).map(|_| true)
    }

    fn approve(
        _owner: address::Address,
        _spender: address::Address,
        _amount: &token::BaseUnits,
    ) -> Result<bool, Error> {
        Ok(false) // XXX
    }

    fn allowance(_owner: address::Address, _spender: address::Address) -> Result<u128, Error> {
        Ok(0) // XXX
    }

    fn mint(to: address::Address, amount: &token::BaseUnits) -> Result<(), Error> {
        T::Accounts::mint(to, amount)
    }

    fn burn(from: address::Address, amount: &token::BaseUnits) -> Result<(), Error> {
        T::Accounts::burn(from, amount)
    }
}

pub struct Erc20Contract<T> {
    _phantom_data: PhantomData<T>,
}

#[sdk_derive(EvmContract)]
impl<T: Erc20Token> Erc20Contract<T> {
    fn sdk_address(addr_token: &Token) -> address::Address {
        address::Address::from_eth(addr_token.clone().into_address().unwrap().as_bytes())
    }

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

    #[evm_method(signature = "name()")]
    fn name(
        _input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        Ok((
            PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::String(T::NAME.to_string())]),
            },
            T::GAS_COSTS.name,
        ))
    }

    #[evm_method(signature = "symbol()")]
    fn symbol(
        _input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        Ok((
            PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::String(T::SYMBOL.to_string())]),
            },
            T::GAS_COSTS.symbol,
        ))
    }

    #[evm_method(signature = "decimals()")]
    fn decimals(
        _input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        Ok((
            PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::Uint(T::DECIMALS.into())]),
            },
            T::GAS_COSTS.decimals,
        ))
    }

    #[evm_method(signature = "totalSupply()")]
    fn total_supply(
        _input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        match T::total_supply() {
            Ok(amount) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                },
                T::GAS_COSTS.total_supply,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "balanceOf(address)")]
    fn balance_of(
        input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address], input)?;
        let address = Self::sdk_address(&params[0]);
        match T::balance_of(address) {
            Ok(amount) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                },
                T::GAS_COSTS.balace_of,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "transfer(address,uint256)")]
    fn transfer(
        input: &[u8],
        _gas_limit: Option<u64>,
        ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address, ParamType::Uint(256)], input)?;
        let recipient = Self::sdk_address(&params[0]);
        let amount = Self::sdk_amount(&params[1])?;
        let sender = address::Address::from_eth(ctx.caller.as_bytes());
        match T::transfer(sender, recipient, &amount) {
            Ok(done) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                },
                T::GAS_COSTS.transfer,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "transferFrom(address,address,uint256)")]
    fn transfer_from(
        input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(
            &[ParamType::Address, ParamType::Address, ParamType::Uint(256)],
            input,
        )?;
        let sender = Self::sdk_address(&params[0]);
        let recipient = Self::sdk_address(&params[1]);
        let amount = Self::sdk_amount(&params[2])?;
        match T::transfer(sender, recipient, &amount) {
            Ok(done) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                },
                T::GAS_COSTS.transfer,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "approve(address,uint256)")]
    fn approve(
        input: &[u8],
        _gas_limit: Option<u64>,
        ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address, ParamType::Uint(256)], input)?;
        let spender = Self::sdk_address(&params[0]);
        let amount = Self::sdk_amount(&params[1])?;
        let owner = address::Address::from_eth(ctx.caller.as_bytes());
        match T::approve(owner, spender, &amount) {
            Ok(done) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                },
                T::GAS_COSTS.approve,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "allowance(address,address)")]
    fn allowance(
        input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address, ParamType::Address], input)?;
        let owner = Self::sdk_address(&params[0]);
        let spender = Self::sdk_address(&params[1]);
        match T::allowance(owner, spender) {
            Ok(amount) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Uint(amount.into())]),
                },
                T::GAS_COSTS.allowance,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "mint(address,uint256)")]
    fn mint(
        input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address, ParamType::Uint(256)], input)?;
        let to = Self::sdk_address(&params[0]);
        let amount = Self::sdk_amount(&params[1])?;
        match T::mint(to, &amount) {
            Ok(_) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                },
                T::GAS_COSTS.mint,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }

    #[evm_method(signature = "burn(address,uint256)")]
    fn burn(
        input: &[u8],
        _gas_limit: Option<u64>,
        _ctx: &Context,
        _is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
        let params = Self::decode_params(&[ParamType::Address, ParamType::Uint(256)], input)?;
        let from = Self::sdk_address(&params[0]);
        let amount = Self::sdk_amount(&params[1])?;
        match T::burn(from, &amount) {
            Ok(_) => Ok((
                PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                },
                T::GAS_COSTS.burn,
            )),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.to_string().into()),
            }),
        }
    }
}
