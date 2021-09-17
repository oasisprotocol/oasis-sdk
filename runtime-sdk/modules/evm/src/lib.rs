//! EVM module.
pub mod evm_backend;
pub mod types;

use std::collections::BTreeMap;

use evm::{
    executor::{MemoryStackState, StackExecutor, StackSubstateMetadata},
    Config as EVMConfig,
};
use once_cell::sync::Lazy;
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

use oasis_runtime_sdk::{
    context::{Context, TxContext},
    crypto::signature::PublicKey,
    error,
    module::{self, CallResult, Module as _},
    modules::{
        self,
        accounts::API as _,
        core::{self, Error as CoreError, API as _},
    },
    storage,
    types::{
        address::Address,
        token,
        transaction::{AddressSpec, AuthInfo, Transaction},
    },
};

use evm::backend::ApplyBackend;
use types::{H160, H256, U256};

/// Unique module name.
const MODULE_NAME: &str = "evm";

/// State schema constants.
pub mod state {
    /// Prefix for Ethereum accounts in our storage (maps H160 -> Account).
    pub const ACCOUNTS: &[u8] = &[0x01];
    /// Prefix for Ethereum account code in our storage (maps H160 -> Vec<u8>).
    pub const CODES: &[u8] = &[0x02];
    /// Prefix for Ethereum account storage in our storage (maps H160||H256 -> H256).
    pub const STORAGES: &[u8] = &[0x03];
    /// Prefix for Ethereum block hashes (only for last BLOCK_HASH_WINDOW_SIZE blocks
    /// excluding current) storage in our storage (maps Round -> H256).
    pub const BLOCK_HASHES: &[u8] = &[0x04];
    /// The number of hash blocks that can be obtained from the current blockchain.
    pub const BLOCK_HASH_WINDOW_SIZE: u64 = 256;
}

/// Module configuration.
pub trait Config: 'static {
    /// Module that is used for accessing accounts.
    type Accounts: modules::accounts::API;
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

/// EVM token pool address.
pub static ADDRESS_EVM_TOKENS: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "evm-tokens"));

/// Errors emitted by the EVM module.
#[derive(Error, Debug, oasis_runtime_sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("EVM error: {0}")]
    #[sdk_error(code = 2)]
    EVMError(String),

    #[error("invalid signer type")]
    #[sdk_error(code = 3)]
    InvalidSignerType,

    #[error("fee overflow")]
    #[sdk_error(code = 4)]
    FeeOverflow,

    #[error("value withdrawal failed")]
    #[sdk_error(code = 5)]
    ValueWithdrawalFailed,

    #[error("gas limit too low: {0} required")]
    #[sdk_error(code = 6)]
    GasLimitTooLow(u64),

    #[error("insufficient balance")]
    #[sdk_error(code = 7)]
    InsufficientBalance,

    #[error("invalid denomination")]
    #[sdk_error(code = 8)]
    InvalidDenomination,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] CoreError),
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_deposit: u64,
    pub tx_withdraw: u64,
}

/// Parameters for the EVM module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    /// Token denomination used for the EVM token.
    pub token_denomination: token::Denomination,
    /// Gas costs.
    pub gas_costs: GasCosts,
}

impl module::Parameters for Parameters {
    type Error = ();

    fn validate_basic(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Genesis state for the EVM module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// Events emitted by the EVM module.
#[derive(Debug, cbor::Encode, oasis_runtime_sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Log {
        address: H160,
        topics: Vec<H256>,
        data: Vec<u8>,
    },
}

impl<Cfg: Config> module::Module for Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

/// Interface that can be called from other modules.
pub trait API {
    /// Perform an Ethereum CREATE transaction.
    /// Returns 160-bit address of created contract.
    fn create<C: TxContext>(ctx: &mut C, value: U256, init_code: Vec<u8>)
        -> Result<Vec<u8>, Error>;

    /// Perform an Ethereum CALL transaction.
    fn call<C: TxContext>(
        ctx: &mut C,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error>;

    /// Deposit tokens from SDK account into EVM account.
    fn deposit<C: TxContext>(
        ctx: &mut C,
        from: Address,
        to: H160,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    /// Withdraw tokens from EVM account into SDK account.
    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: H160,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error>;

    /// Peek into EVM storage.
    /// Returns 256-bit value stored at given contract address and index (slot)
    /// in the storage.
    fn peek_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error>;

    /// Peek into EVM code storage.
    /// Returns EVM bytecode of contract at given address.
    fn peek_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error>;
}

impl<Cfg: Config> API for Module<Cfg> {
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;

        if ctx.is_check_only() {
            return Ok(vec![]);
        }

        Self::do_evm(caller, value, ctx, |exec, gas_limit| {
            let address = exec.create_address(evm::CreateScheme::Legacy {
                caller: caller.into(),
            });
            (
                exec.transact_create(caller.into(), value.into(), init_code, gas_limit),
                address.as_bytes().to_vec(),
            )
        })
    }

    fn call<C: TxContext>(
        ctx: &mut C,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;

        if ctx.is_check_only() {
            return Ok(vec![]);
        }

        Self::do_evm(caller, value, ctx, |exec, gas_limit| {
            exec.transact_call(caller.into(), address.into(), value.into(), data, gas_limit)
        })
    }

    fn deposit<C: TxContext>(
        ctx: &mut C,
        from: Address,
        to: H160,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        // Make sure that the denomination is the same as is set in our params.
        let params = Self::params(ctx.runtime_state());
        if amount.denomination() != &params.token_denomination {
            return Err(Error::InvalidDenomination);
        }

        if ctx.is_check_only() {
            let bal =
                Cfg::Accounts::get_balance(ctx.runtime_state(), from, params.token_denomination)
                    .unwrap();
            if bal < amount.amount() {
                return Err(Error::InsufficientBalance);
            }
            return Ok(());
        }

        // Transfer tokens from SDK account into EVM pool.
        Cfg::Accounts::transfer(ctx, from, *ADDRESS_EVM_TOKENS, &amount)
            .map_err(|_| Error::InsufficientBalance)?;

        // Increase EVM account's balance by the amount of tokens transferred.
        let state = ctx.runtime_state();
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let mut account: evm_backend::Account = accounts.get(&to).unwrap_or_default();

        account.balance += amount.amount().into();
        accounts.insert(&to, account);

        Ok(())
    }

    fn withdraw<C: TxContext>(
        ctx: &mut C,
        from: H160,
        to: Address,
        amount: token::BaseUnits,
    ) -> Result<(), Error> {
        let is_check_only = ctx.is_check_only();

        // Make sure that the denomination is the same as is set in our params.
        let params = Self::params(ctx.runtime_state());
        if amount.denomination() != &params.token_denomination {
            return Err(Error::InvalidDenomination);
        }

        // Check EVM account's balance.
        let state = ctx.runtime_state();
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let mut account: evm_backend::Account = accounts.get(&from).unwrap_or_default();

        if account.balance < amount.amount().into() {
            return Err(Error::InsufficientBalance);
        }

        if is_check_only {
            return Ok(());
        }

        // Decrease EVM account's balance by the amount of tokens requested.
        account.balance -= amount.amount().into();
        accounts.insert(&from, account);

        // Transfer tokens from EVM pool into SDK account.
        Cfg::Accounts::transfer(ctx, *ADDRESS_EVM_TOKENS, to, &amount)
            .map_err(|_| Error::InsufficientBalance)
    }

    fn peek_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &state::STORAGES);
        let s = storage::TypedStore::new(storage::HashedStore::<_, blake3::Hasher>::new(
            storage::PrefixStore::new(storages, &address),
        ));

        let result: H256 = s.get(&index).unwrap_or_default();

        Ok(result.as_bytes().to_vec())
    }

    fn peek_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &state::CODES));

        Ok(codes.get(&address).unwrap_or_default())
    }
}

impl<Cfg: Config> Module<Cfg> {
    const EVM_CONFIG: EVMConfig = EVMConfig::istanbul();

    fn do_evm<C, F, V>(source: H160, value: U256, ctx: &mut C, f: F) -> Result<V, Error>
    where
        F: FnOnce(
            &mut StackExecutor<'static, MemoryStackState<'_, 'static, evm_backend::Backend<'_, C>>>,
            u64,
        ) -> (evm::ExitReason, V),
        C: TxContext,
    {
        let params = Self::params(ctx.runtime_state());
        let den = params.token_denomination;

        let gas_limit: u64 = core::Module::remaining_tx_gas(ctx);
        let gas_price: primitive_types::U256 = ctx.tx_auth_info().fee.gas_price().into();

        let vicinity = evm_backend::Vicinity {
            gas_price: gas_price.into(),
            origin: source,
        };

        // The maximum gas fee has already been withdrawn in authenticate_tx().
        let max_gas_fee = gas_price
            .checked_mul(primitive_types::U256::from(gas_limit))
            .ok_or(Error::FeeOverflow)?;

        let mut backend = evm_backend::Backend::<'_, C>::new(vicinity, ctx);
        let metadata = StackSubstateMetadata::new(gas_limit, &Self::EVM_CONFIG);
        let stackstate = MemoryStackState::new(metadata, &backend);
        let mut executor = StackExecutor::new(stackstate, &Self::EVM_CONFIG);

        // Withdraw the value from the account.
        executor
            .state_mut()
            .withdraw(source.into(), value.into())
            .map_err(|_| Error::ValueWithdrawalFailed)?;

        // Run EVM.
        let (exit_reason, exit_value) = f(&mut executor, gas_limit);

        if !exit_reason.is_succeed() {
            return Err(Error::EVMError(format!("{:?}", exit_reason)));
        }

        let gas_used = executor.used_gas();

        if gas_used > gas_limit {
            core::Module::use_tx_gas(ctx, gas_limit).map_err(Error::Core)?;
            return Err(Error::GasLimitTooLow(gas_used));
        }

        let fee = executor.fee(gas_price);

        // Return the difference between the pre-paid max_gas and actually
        // used gas.
        let return_fee = max_gas_fee
            .checked_sub(fee)
            .ok_or(Error::InsufficientBalance)?;
        executor.state_mut().deposit(source.into(), return_fee);

        let (vals, logs) = executor.into_state().deconstruct();
        backend.apply(vals, logs, true);

        core::Module::use_tx_gas(ctx, gas_used).map_err(Error::Core)?;

        // Move the difference from the fee accumulator back into the
        // EVM token pool.
        Cfg::Accounts::move_from_fee_accumulator(
            ctx,
            *ADDRESS_EVM_TOKENS,
            &token::BaseUnits::new(return_fee.as_u128(), den),
        )
        .map_err(|_| Error::InsufficientBalance)?;

        Ok(exit_value)
    }

    fn derive_caller_from_bytes(b: &[u8]) -> H160 {
        // Caller address is derived by doing Keccak-256 on the
        // secp256k1 public key and taking the last 20 bytes
        // of the result.
        let mut k = Keccak::v256();
        let mut out = [0u8; 32];
        k.update(b);
        k.finalize(&mut out);
        H160::from_slice(&out[32 - 20..])
    }

    fn derive_caller_from_tx_auth_info(ai: &AuthInfo) -> Result<H160, Error> {
        match &ai.signer_info[0].address_spec {
            AddressSpec::Signature(PublicKey::Secp256k1(pk)) => {
                Ok(Self::derive_caller_from_bytes(pk.as_bytes()))
            }
            AddressSpec::Signature(PublicKey::Ed25519(_)) => Ok(Self::derive_caller_from_bytes(
                &ai.signer_info[0].address_spec.address().as_ref()[1..],
            )),
            _ => Err(Error::InvalidSignerType),
        }
    }

    fn derive_caller<C>(ctx: &mut C) -> Result<H160, Error>
    where
        C: TxContext,
    {
        Self::derive_caller_from_tx_auth_info(ctx.tx_auth_info())
    }

    fn tx_create<C: TxContext>(ctx: &mut C, body: types::Create) -> Result<Vec<u8>, Error> {
        Self::create(ctx, body.value, body.init_code)
    }

    fn tx_call<C: TxContext>(ctx: &mut C, body: types::Call) -> Result<Vec<u8>, Error> {
        Self::call(ctx, body.address, body.value, body.data)
    }

    fn tx_deposit<C: TxContext>(ctx: &mut C, body: types::Deposit) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        core::Module::use_tx_gas(ctx, params.gas_costs.tx_deposit)?;

        Self::deposit(ctx, body.from, body.to, body.amount)
    }

    fn tx_withdraw<C: TxContext>(ctx: &mut C, body: types::Withdraw) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        core::Module::use_tx_gas(ctx, params.gas_costs.tx_withdraw)?;

        Self::withdraw(ctx, body.from, body.to, body.amount)
    }

    fn q_peek_storage<C: Context>(
        ctx: &mut C,
        body: types::PeekStorageQuery,
    ) -> Result<Vec<u8>, Error> {
        Self::peek_storage(ctx, body.address, body.index)
    }

    fn q_peek_code<C: Context>(ctx: &mut C, body: types::PeekCodeQuery) -> Result<Vec<u8>, Error> {
        Self::peek_code(ctx, body.address)
    }
}

impl<Cfg: Config> module::MethodHandler for Module<Cfg> {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "evm.Create" => module::dispatch_call(ctx, body, Self::tx_create),
            "evm.Call" => module::dispatch_call(ctx, body, Self::tx_call),
            "evm.Deposit" => module::dispatch_call(ctx, body, Self::tx_deposit),
            "evm.Withdraw" => module::dispatch_call(ctx, body, Self::tx_withdraw),
            _ => module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, Result<cbor::Value, error::RuntimeError>> {
        match method {
            "evm.PeekStorage" => module::dispatch_query(ctx, args, Self::q_peek_storage),
            "evm.PeekCode" => module::dispatch_query(ctx, args, Self::q_peek_code),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl<Cfg: Config> module::MigrationHandler for Module<Cfg> {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
        genesis: Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            Self::set_params(ctx.runtime_state(), genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not used.
        false
    }
}

impl<Cfg: Config> module::AuthHandler for Module<Cfg> {
    fn authenticate_tx<C: Context>(ctx: &mut C, tx: &Transaction) -> Result<(), CoreError> {
        // We're only interested in transactions that can be paid with tokens
        // from the corresponding EVM account.
        match tx.call.method.as_str() {
            "evm.Create" | "evm.Call" | "evm.Withdraw" => {}
            _ => return Err(CoreError::NotAuthenticated),
        }

        let params = Self::params(ctx.runtime_state());
        let den = params.token_denomination;

        let evm_acct_addr = Self::derive_caller_from_tx_auth_info(&tx.auth_info)
            .map_err(|e| CoreError::MalformedTransaction(anyhow::Error::new(e)))?;

        // Check and update nonces on all signer accounts.
        // Note that we can ignore the return value because the payee is already
        // checked in the above call that derives the EVM account address.
        let _payee = Cfg::Accounts::check_and_update_signer_nonces(ctx, tx)?;

        // The fee should be set by the user to cover at least:
        //     gas_price * gas_limit + sdk_fees
        // And the gas should be set to:
        //     gas_limit + sdk_gas_limit
        // The difference between the paid fee and the actually used gas
        // will be returned in do_evm() above after execution is complete
        // and the EVM has calculated the actual amount of gas used.
        let fee = tx.auth_info.fee.amount.amount();

        // Take the tokens from the EVM account and move them into the
        // fee accumulator.
        let state = ctx.runtime_state();
        let store = storage::PrefixStore::new(state, &MODULE_NAME);
        let mut accounts =
            storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));
        let mut account: evm_backend::Account = accounts.get(&evm_acct_addr).unwrap_or_default();

        if account.balance < fee.into() {
            return Err(CoreError::InsufficientFeeBalance);
        }

        account.nonce += 1.into();
        account.balance -= fee.into();
        accounts.insert(&evm_acct_addr, account);

        Cfg::Accounts::move_into_fee_accumulator(
            ctx,
            *ADDRESS_EVM_TOKENS,
            &token::BaseUnits::new(fee, den),
        )
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn end_block<C: Context>(ctx: &mut C) {
        let block_number = ctx.runtime_header().round;
        let block_hash = ctx.runtime_header().encoded_hash();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let hashes = storage::PrefixStore::new(store, &state::BLOCK_HASHES);
        let mut block_hashes = storage::TypedStore::new(hashes);

        let current_number = block_number;
        block_hashes.insert(&block_number.to_be_bytes(), block_hash);

        if current_number > state::BLOCK_HASH_WINDOW_SIZE {
            let start_number = current_number - state::BLOCK_HASH_WINDOW_SIZE;
            block_hashes.remove(&start_number.to_be_bytes());
        }
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {
    /// Check invariants.
    fn check_invariants<C: Context>(ctx: &mut C) -> Result<(), CoreError> {
        // All EVM account balances should sum up to the balance
        // of the EVM token pool.

        let params = Self::params(ctx.runtime_state());

        // Get balance of EVM token pool.
        #[allow(clippy::or_fun_call)]
        let pool_balance = Cfg::Accounts::get_balance(
            ctx.runtime_state(),
            *ADDRESS_EVM_TOKENS,
            params.token_denomination,
        )
        .or(Err(CoreError::InvariantViolation(
            "unable to get EVM token pool balance".to_string(),
        )))?;

        // Get all EVM accounts.
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let astore = storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));

        let accounts: BTreeMap<H160, evm_backend::Account> = astore.iter().collect();

        // Compute the total balance of all EVM accounts.
        let mut evm_balance: U256 = U256::zero();
        for acct in accounts.values() {
            match evm_balance.checked_add(acct.balance) {
                Some(eb) => evm_balance = eb,
                None => {
                    return Err(CoreError::InvariantViolation(
                        "U256 overflow when computing total balance of all EVM accounts"
                            .to_string(),
                    ))
                }
            }
        }

        if evm_balance != pool_balance.into() {
            Err(CoreError::InvariantViolation(format!(
                "token pool balance mismatch: evm_balance={}, pool_balance={}",
                evm_balance, pool_balance,
            )))
        } else {
            Ok(())
        }
    }
}
