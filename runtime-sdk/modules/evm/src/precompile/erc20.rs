use std::marker::PhantomData;

use ethabi::Token;
use evm::{
    executor::stack::{PrecompileHandle, PrecompileOutput},
    ExitSucceed,
};
use oasis_runtime_sdk::{
    modules::accounts::{Error as AccountsError, API as AccountsAPI},
    types::{address::Address, token},
};
use oasis_runtime_sdk_macros::{evm_contract_address, evm_method, sdk_derive, EvmEvent};
use primitive_types::H160;

use crate::precompile::{
    contract::{EvmError as _, EvmEvent as _},
    PrecompileResult,
};

const MODULE_NAME: &str = "evm/erc20";

/// Errors emitted by the ERC-20 implementation.
#[derive(
    thiserror::Error, Debug, oasis_runtime_sdk_macros::Error, oasis_runtime_sdk_macros::EvmError,
)]
pub enum Error {
    #[error("forbidden by policy")]
    #[sdk_error(code = 1)]
    #[evm_error(signature = "Forbidden()")]
    Forbidden,

    /// EIP-6093 defined error.
    #[error("invalid sender address {0}")]
    #[sdk_error(code = 2)]
    #[evm_error(signature = "ERC20InvalidSender(address)")]
    ERC20InvalidSender(H160),

    /// EIP-6093 defined error.
    #[error("invalid receiver address {0}")]
    #[sdk_error(code = 3)]
    #[evm_error(signature = "ERC20InvalidReceiver(address)")]
    ERC20InvalidReceiver(H160),

    /// EIP-6093 defined error.
    #[error("invalid approver address {0}")]
    #[sdk_error(code = 4)]
    #[evm_error(signature = "ERC20InvalidApprover(address)")]
    ERC20InvalidApprover(H160),

    /// EIP-6093 defined error.
    #[error("invalid spender address {0}")]
    #[sdk_error(code = 5)]
    #[evm_error(signature = "ERC20InvalidSpender(address)")]
    ERC20InvalidSpender(H160),

    /// EIP-6093 defined error.
    #[error("insufficient balance: {0}, {1} < {2}")]
    #[sdk_error(code = 6)]
    #[evm_error(signature = "ERC20InsufficientBalance(address,uint256,uint256)")]
    ERC20InsufficientBalance(H160, u128, u128),

    /// EIP-6093 defined error.
    #[error("insufficient balance: {0}, {1} < {2}")]
    #[sdk_error(code = 7)]
    #[evm_error(signature = "ERC20InsufficientAllowance(address,uint256,uint256)")]
    ERC20InsufficientAllowance(H160, u128, u128),

    #[error("accounts: {0}")]
    #[sdk_error(transparent)]
    #[evm_error(signature = "Accounts(string)")]
    Accounts(#[from] oasis_runtime_sdk::modules::accounts::Error),
}

impl Error {
    fn annotate_accounts_error<T>(
        err: AccountsError,
        account: Address,
        address: H160,
        amount: &token::BaseUnits,
    ) -> Self
    where
        T: AccountToken,
    {
        match err {
            AccountsError::InsufficientBalance => {
                match T::Accounts::get_balance(account, amount.denomination().clone()) {
                    Ok(balance) => {
                        Self::ERC20InsufficientBalance(address, balance, amount.amount())
                    }
                    Err(err) => err.into(),
                }
            }
            _ => err.into(),
        }
    }
}

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
    pub const fn default() -> Self {
        Self {
            name: 100,
            symbol: 100,
            decimals: 100,
            total_supply: 500,
            balace_of: 500,
            transfer: 1_000,
            transfer_from: 1_000,
            approve: 1_500,
            allowance: 500,
            mint: 1_000,
            burn: 1_000,
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
    fn transfer(sender: &H160, recipient: &H160, amount: u128) -> Result<bool, Error>;
    fn transfer_from(
        owner: &H160,
        spender: &H160,
        recipient: &H160,
        amount: u128,
    ) -> Result<bool, Error>;
    fn approve(owner: &H160, spender: &H160, amount: u128) -> Result<bool, Error>;
    fn allowance(owner: &H160, spender: &H160) -> Result<u128, Error>;
    fn mint(caller: &H160, to: &H160, amount: u128) -> Result<(), Error>;
    fn burn(caller: &H160, from: &H160, amount: u128) -> Result<(), Error>;
}

/// Helper trait for simplifying implementations for tokens backed by the SDK's
/// Accounts module.
///
/// The trait presents a much narrower interface than the full [`Erc20Token`]
/// trait and there is a blanket [`Erc20Token`] impl for any struct that
/// implements `AccountToken`.
pub trait AccountToken {
    type Accounts: AccountsAPI;

    const ADDRESS: H160;

    const GAS_COSTS: TokenOperationCosts;

    const NAME: &str;
    const SYMBOL: &str;
    const DECIMALS: u8;

    fn denomination() -> token::Denomination;

    fn is_minting_allowed(caller: &H160, address: &H160) -> Result<bool, Error>;
    fn is_burning_allowed(caller: &H160, address: &H160) -> Result<bool, Error>;
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
        Ok(T::Accounts::get_total_supply(T::denomination())?)
    }

    fn balance_of(account: &H160) -> Result<u128, Error> {
        let account = Address::from_eth(account.as_bytes());
        Ok(T::Accounts::get_balance(account, T::denomination())?)
    }

    fn transfer(sender: &H160, recipient: &H160, amount: u128) -> Result<bool, Error> {
        if sender.is_zero() {
            return Err(Error::ERC20InvalidSender(*sender));
        }
        if recipient.is_zero() {
            return Err(Error::ERC20InvalidReceiver(*recipient));
        }
        let amount = token::BaseUnits::new(amount, T::denomination());
        let sender_address = Address::from_eth(sender.as_bytes());
        let recipient_address = Address::from_eth(recipient.as_bytes());
        T::Accounts::transfer(sender_address, recipient_address, &amount)
            .map(|_| true)
            .map_err(|e| Error::annotate_accounts_error::<T>(e, sender_address, *sender, &amount))
    }

    fn transfer_from(
        owner: &H160,
        spender: &H160,
        recipient: &H160,
        amount: u128,
    ) -> Result<bool, Error> {
        if owner.is_zero() {
            return Err(Error::ERC20InvalidApprover(*owner));
        }
        if spender.is_zero() {
            return Err(Error::ERC20InvalidSpender(*spender));
        }
        if recipient.is_zero() {
            return Err(Error::ERC20InvalidReceiver(*recipient));
        }
        let amount = token::BaseUnits::new(amount, T::denomination());
        let owner_acc = Address::from_eth(owner.as_bytes());
        let spender_acc = Address::from_eth(spender.as_bytes());
        let recipient_acc = Address::from_eth(recipient.as_bytes());
        let mut allowance = T::Accounts::get_allowance(owner_acc, spender_acc, T::denomination())?;
        allowance = allowance.checked_sub(amount.amount()).ok_or_else(|| {
            Error::ERC20InsufficientAllowance(*spender, allowance, amount.amount())
        })?;
        T::Accounts::set_allowance(
            owner_acc,
            spender_acc,
            &token::BaseUnits::new(allowance, T::denomination()),
        );
        T::Accounts::transfer(owner_acc, recipient_acc, &amount)
            .map(|_| true)
            .map_err(|e| Error::annotate_accounts_error::<T>(e, owner_acc, *owner, &amount))
    }

    fn approve(owner: &H160, spender: &H160, amount: u128) -> Result<bool, Error> {
        if owner.is_zero() {
            return Err(Error::ERC20InvalidApprover(*owner));
        }
        if spender.is_zero() {
            return Err(Error::ERC20InvalidSpender(*spender));
        }
        let owner = Address::from_eth(owner.as_bytes());
        let spender = Address::from_eth(spender.as_bytes());
        T::Accounts::set_allowance(
            owner,
            spender,
            &token::BaseUnits::new(amount, T::denomination()),
        );
        Ok(true)
    }

    fn allowance(owner: &H160, spender: &H160) -> Result<u128, Error> {
        let owner = Address::from_eth(owner.as_bytes());
        let spender = Address::from_eth(spender.as_bytes());
        Ok(T::Accounts::get_allowance(
            owner,
            spender,
            T::denomination(),
        )?)
    }

    fn mint(caller: &H160, to: &H160, amount: u128) -> Result<(), Error> {
        if to.is_zero() {
            return Err(Error::ERC20InvalidReceiver(*to));
        }
        let amount = token::BaseUnits::new(amount, T::denomination());
        if Self::is_minting_allowed(caller, to)? {
            let to = Address::from_eth(to.as_bytes());
            Ok(T::Accounts::mint(to, &amount)?)
        } else {
            Err(Error::Forbidden)
        }
    }

    fn burn(caller: &H160, from: &H160, amount: u128) -> Result<(), Error> {
        if from.is_zero() {
            return Err(Error::ERC20InvalidReceiver(*from));
        }
        let amount = token::BaseUnits::new(amount, T::denomination());
        if Self::is_burning_allowed(caller, from)? {
            let from = Address::from_eth(from.as_bytes());
            Ok(T::Accounts::burn(from, &amount)?)
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
        handle.record_cost(T::GAS_COSTS.total_supply)?;
        match T::total_supply() {
            Ok(amount) => Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::Uint(amount.into())]),
            }),
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "balanceOf(address)", convert)]
    fn balance_of(handle: &mut impl PrecompileHandle, address: H160) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.balace_of)?;
        match T::balance_of(&address) {
            Ok(amount) => Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::Uint(amount.into())]),
            }),
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "transfer(address,uint256)", convert)]
    fn transfer(
        handle: &mut impl PrecompileHandle,
        recipient: H160,
        amount: u128,
    ) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.transfer)?;
        let sender = handle.context().caller;
        match T::transfer(&sender, &recipient, amount) {
            Ok(done) => {
                TransferEvent {
                    from: Token::Address(handle.context().caller),
                    to: Token::Address(recipient),
                    value: Token::Uint(amount.into()),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "transferFrom(address,address,uint256)", convert)]
    fn transfer_from(
        handle: &mut impl PrecompileHandle,
        owner: H160,
        recipient: H160,
        amount: u128,
    ) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.transfer_from)?;
        let caller = handle.context().caller;
        match T::transfer_from(&owner, &caller, &recipient, amount) {
            Ok(done) => {
                TransferEvent {
                    from: Token::Address(owner),
                    to: Token::Address(recipient),
                    value: Token::Uint(amount.into()),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "approve(address,uint256)", convert)]
    fn approve(
        handle: &mut impl PrecompileHandle,
        spender: H160,
        amount: u128,
    ) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.approve)?;
        let owner = handle.context().caller;
        match T::approve(&owner, &spender, amount) {
            Ok(done) => {
                ApprovalEvent {
                    owner: Token::Address(owner),
                    spender: Token::Address(spender),
                    value: Token::Uint(amount.into()),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[Token::Bool(done)]),
                })
            }
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "allowance(address,address)", convert)]
    fn allowance(
        handle: &mut impl PrecompileHandle,
        owner: H160,
        spender: H160,
    ) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.allowance)?;
        match T::allowance(&owner, &spender) {
            Ok(amount) => Ok(PrecompileOutput {
                exit_status: ExitSucceed::Returned,
                output: ethabi::encode(&[Token::Uint(amount.into())]),
            }),
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "mint(address,uint256)", convert)]
    fn mint(handle: &mut impl PrecompileHandle, to: H160, amount: u128) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.mint)?;
        let caller = handle.context().caller;
        match T::mint(&caller, &to, amount) {
            Ok(_) => {
                TransferEvent {
                    from: Token::Address(H160::zero()),
                    to: Token::Address(to),
                    value: Token::Uint(amount.into()),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                })
            }
            Err(e) => Err(e.encode()),
        }
    }

    #[evm_method(signature = "burn(address,uint256)", convert)]
    fn burn(handle: &mut impl PrecompileHandle, from: H160, amount: u128) -> PrecompileResult {
        handle.record_cost(T::GAS_COSTS.burn)?;
        let caller = handle.context().caller;
        match T::burn(&caller, &from, amount) {
            Ok(_) => {
                TransferEvent {
                    from: Token::Address(from),
                    to: Token::Address(H160::zero()),
                    value: Token::Uint(amount.into()),
                }
                .emit::<Self>(handle)
                .unwrap();
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: vec![],
                })
            }
            Err(e) => Err(e.encode()),
        }
    }
}
