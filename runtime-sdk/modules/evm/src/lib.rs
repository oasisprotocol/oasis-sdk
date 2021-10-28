//! EVM module.

pub mod backend;
pub mod derive_caller;
pub mod precompile;
pub mod raw_tx;
pub mod types;

use std::collections::BTreeMap;

use evm::{
    executor::{MemoryStackState, PrecompileFn, StackExecutor, StackSubstateMetadata},
    Config as EVMConfig,
};
use thiserror::Error;

use oasis_runtime_sdk::{
    context::{BatchContext, Context, TxContext},
    error,
    module::{self, CallResult, Module as _},
    modules::{
        self,
        accounts::API as _,
        core::{self, Error as CoreError, API as _},
    },
    storage,
    types::{
        address::{self, Address},
        token, transaction,
        transaction::Transaction,
    },
};

use evm::backend::ApplyBackend;
use types::{H160, H256, U256};

#[cfg(test)]
mod test;

/// Unique module name.
const MODULE_NAME: &str = "evm";

/// State schema constants.
pub mod state {
    use super::{storage, H160};

    /// Prefix for Ethereum account code in our storage (maps H160 -> Vec<u8>).
    pub const CODES: &[u8] = &[0x01];
    /// Prefix for Ethereum account storage in our storage (maps H160||H256 -> H256).
    pub const STORAGES: &[u8] = &[0x02];
    /// Prefix for Ethereum block hashes (only for last BLOCK_HASH_WINDOW_SIZE blocks
    /// excluding current) storage in our storage (maps Round -> H256).
    pub const BLOCK_HASHES: &[u8] = &[0x03];
    /// The number of hash blocks that can be obtained from the current blockchain.
    pub const BLOCK_HASH_WINDOW_SIZE: u64 = 256;

    /// Get a typed store for the given address' storage.
    pub fn storage<'a, S: storage::Store + 'a>(
        state: S,
        address: &'a H160,
    ) -> storage::TypedStore<impl storage::Store + 'a> {
        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &STORAGES);
        storage::TypedStore::new(storage::HashedStore::<_, blake3::Hasher>::new(
            storage::PrefixStore::new(storages, address),
        ))
    }

    /// Get a typed store for codes of all contracts.
    pub fn codes<'a, S: storage::Store + 'a>(
        state: S,
    ) -> storage::TypedStore<impl storage::Store + 'a> {
        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        storage::TypedStore::new(storage::PrefixStore::new(store, &CODES))
    }

    /// Get a typed store for historic block hashes.
    pub fn block_hashes<'a, S: storage::Store + 'a>(
        state: S,
    ) -> storage::TypedStore<impl storage::Store + 'a> {
        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        storage::TypedStore::new(storage::PrefixStore::new(store, &BLOCK_HASHES))
    }
}

/// Module configuration.
pub trait Config: 'static {
    /// Module that is used for accessing accounts.
    type Accounts: modules::accounts::API;

    /// The chain ID to supply when a contract requests it. Ethereum-format transactions must use
    /// this chain ID.
    const CHAIN_ID: u64;

    /// Token denomination used as the native EVM token.
    const TOKEN_DENOMINATION: token::Denomination;

    /// Maps an Ethereum address into an SDK account address.
    fn map_address(address: primitive_types::H160) -> Address {
        Address::new(
            address::ADDRESS_V0_SECP256K1ETH_CONTEXT,
            address::ADDRESS_V0_VERSION,
            address.as_ref(),
        )
    }
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

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

    #[error("gas limit too low: {0} required")]
    #[sdk_error(code = 5)]
    GasLimitTooLow(u64),

    #[error("insufficient balance")]
    #[sdk_error(code = 6)]
    InsufficientBalance,

    #[error("forbidden by policy")]
    #[sdk_error(code = 7)]
    Forbidden,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] CoreError),
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {}

/// Parameters for the EVM module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
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

    /// Peek into EVM storage.
    /// Returns 256-bit value stored at given contract address and index (slot)
    /// in the storage.
    fn get_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error>;

    /// Peek into EVM code storage.
    /// Returns EVM bytecode of contract at given address.
    fn get_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error>;

    /// Get EVM account balance.
    fn get_balance<C: Context>(ctx: &mut C, address: H160) -> Result<u128, Error>;

    /// Simulate an Ethereum CALL.
    fn simulate_call<C: Context>(
        ctx: &mut C,
        gas_price: U256,
        gas_limit: u64,
        caller: H160,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error>;
}

impl<Cfg: Config> API for Module<Cfg> {
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;

        if ctx.is_check_only() && !ctx.are_expensive_queries_allowed() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        Self::do_evm(caller, ctx, |exec, gas_limit| {
            let address = exec.create_address(evm::CreateScheme::Legacy {
                caller: caller.into(),
            });
            (
                exec.transact_create(caller.into(), value.into(), init_code, gas_limit, vec![]),
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

        if ctx.is_check_only() && !ctx.are_expensive_queries_allowed() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        Self::do_evm(caller, ctx, |exec, gas_limit| {
            exec.transact_call(
                caller.into(),
                address.into(),
                value.into(),
                data,
                gas_limit,
                vec![],
            )
        })
    }

    fn get_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &state::STORAGES);
        let s = storage::TypedStore::new(storage::HashedStore::<_, blake3::Hasher>::new(
            storage::PrefixStore::new(storages, &address),
        ));

        let result: H256 = s.get(&index).unwrap_or_default();

        Ok(result.as_bytes().to_vec())
    }

    fn get_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &state::CODES));

        Ok(codes.get(&address).unwrap_or_default())
    }

    fn get_balance<C: Context>(ctx: &mut C, address: H160) -> Result<u128, Error> {
        let state = ctx.runtime_state();
        let address = Cfg::map_address(address.into());
        Ok(Cfg::Accounts::get_balance(state, address, Cfg::TOKEN_DENOMINATION).unwrap_or_default())
    }

    fn simulate_call<C: Context>(
        ctx: &mut C,
        gas_price: U256,
        gas_limit: u64,
        caller: H160,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        if !ctx.are_expensive_queries_allowed() {
            return Err(Error::Forbidden);
        }

        ctx.with_simulation(|mut sctx| {
            let call_tx = transaction::Transaction {
                version: 1,
                call: transaction::Call {
                    format: transaction::CallFormat::Plain,
                    method: "evm.Call".to_owned(),
                    body: cbor::to_value(types::Call {
                        address,
                        value,
                        data: data.clone(),
                    }),
                },
                auth_info: transaction::AuthInfo {
                    signer_info: vec![],
                    fee: transaction::Fee {
                        amount: token::BaseUnits::new(
                            gas_price
                                .checked_mul(U256::from(gas_limit))
                                .ok_or(Error::FeeOverflow)?
                                .as_u128(),
                            Cfg::TOKEN_DENOMINATION,
                        ),
                        gas: gas_limit,
                        consensus_messages: 0,
                    },
                },
            };
            sctx.with_tx(0, call_tx, |mut txctx, _call| {
                Self::do_evm(caller, &mut txctx, |exec, gas_limit| {
                    exec.transact_call(
                        caller.into(),
                        address.into(),
                        value.into(),
                        data,
                        gas_limit,
                        vec![],
                    )
                })
            })
        })
    }
}

impl<Cfg: Config> Module<Cfg> {
    const EVM_CONFIG: EVMConfig = EVMConfig::istanbul();

    fn do_evm<C, F, V>(source: H160, ctx: &mut C, f: F) -> Result<V, Error>
    where
        F: FnOnce(
            &mut StackExecutor<
                'static,
                '_,
                MemoryStackState<'_, 'static, backend::Backend<'_, C, Cfg>>,
                BTreeMap<primitive_types::H160, PrecompileFn>,
            >,
            u64,
        ) -> (evm::ExitReason, V),
        C: TxContext,
    {
        let gas_limit: u64 = core::Module::remaining_tx_gas(ctx);
        let gas_price: primitive_types::U256 = ctx.tx_auth_info().fee.gas_price().into();
        let fee_denomination = ctx.tx_auth_info().fee.amount.denomination().clone();

        let vicinity = backend::Vicinity {
            gas_price: gas_price.into(),
            origin: source,
        };

        // The maximum gas fee has already been withdrawn in authenticate_tx().
        let max_gas_fee = gas_price
            .checked_mul(primitive_types::U256::from(gas_limit))
            .ok_or(Error::FeeOverflow)?;

        let mut backend = backend::Backend::<'_, C, Cfg>::new(ctx, vicinity);
        let metadata = StackSubstateMetadata::new(gas_limit, &Self::EVM_CONFIG);
        let stackstate = MemoryStackState::new(metadata, &backend);
        let mut executor = StackExecutor::new_with_precompiles(
            stackstate,
            &Self::EVM_CONFIG,
            &*precompile::PRECOMPILED_CONTRACT,
        );

        // Run EVM.
        let (exit_reason, exit_value) = f(&mut executor, gas_limit);

        if !exit_reason.is_succeed() {
            return Err(Error::EVMError(format!("{:?}", exit_reason)));
        }

        let gas_used = executor.used_gas();

        if gas_used > gas_limit {
            // NOTE: This should never happen as the gas was accounted for in advance.
            core::Module::use_tx_gas(ctx, gas_limit)?;
            return Err(Error::GasLimitTooLow(gas_used));
        }

        // Return the difference between the pre-paid max_gas and actually used gas.
        let fee = executor.fee(gas_price);
        let return_fee = max_gas_fee
            .checked_sub(fee)
            .ok_or(Error::InsufficientBalance)?;

        let (vals, logs) = executor.into_state().deconstruct();
        backend.apply(vals, logs, true);

        core::Module::use_tx_gas(ctx, gas_used)?;

        // Move the difference from the fee accumulator back to the caller.
        let caller_address = Cfg::map_address(source.into());
        Cfg::Accounts::move_from_fee_accumulator(
            ctx,
            caller_address,
            &token::BaseUnits::new(return_fee.as_u128(), fee_denomination),
        )
        .map_err(|_| Error::InsufficientBalance)?;

        Ok(exit_value)
    }

    fn derive_caller<C>(ctx: &mut C) -> Result<H160, Error>
    where
        C: TxContext,
    {
        derive_caller::from_tx_auth_info(ctx.tx_auth_info())
    }

    fn tx_create<C: TxContext>(ctx: &mut C, body: types::Create) -> Result<Vec<u8>, Error> {
        Self::create(ctx, body.value, body.init_code)
    }

    fn tx_call<C: TxContext>(ctx: &mut C, body: types::Call) -> Result<Vec<u8>, Error> {
        Self::call(ctx, body.address, body.value, body.data)
    }

    fn query_storage<C: Context>(ctx: &mut C, body: types::StorageQuery) -> Result<Vec<u8>, Error> {
        Self::get_storage(ctx, body.address, body.index)
    }

    fn query_code<C: Context>(ctx: &mut C, body: types::CodeQuery) -> Result<Vec<u8>, Error> {
        Self::get_code(ctx, body.address)
    }

    fn query_balance<C: Context>(ctx: &mut C, body: types::BalanceQuery) -> Result<u128, Error> {
        Self::get_balance(ctx, body.address)
    }

    fn query_simulate_call<C: Context>(
        ctx: &mut C,
        body: types::SimulateCallQuery,
    ) -> Result<Vec<u8>, Error> {
        Self::simulate_call(
            ctx,
            body.gas_price,
            body.gas_limit,
            body.caller,
            body.address,
            body.value,
            body.data,
        )
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
            _ => module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, Result<cbor::Value, error::RuntimeError>> {
        match method {
            "evm.Storage" => module::dispatch_query(ctx, args, Self::query_storage),
            "evm.Code" => module::dispatch_query(ctx, args, Self::query_code),
            "evm.Balance" => module::dispatch_query(ctx, args, Self::query_balance),
            "evm.SimulateCall" => module::dispatch_query(ctx, args, Self::query_simulate_call),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl<Cfg: Config> Module<Cfg> {
    /// Initialize state from genesis.
    fn init<C: Context>(ctx: &mut C, genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), genesis.parameters);
    }

    /// Migrate state from a previous version.
    fn migrate<C: Context>(_ctx: &mut C, _from: u32) -> bool {
        // No migrations currently supported.
        false
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
            Self::init(ctx, genesis);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Perform migration.
        Self::migrate(ctx, version)
    }
}

impl<Cfg: Config> module::AuthHandler for Module<Cfg> {
    fn decode_tx<C: Context>(
        _ctx: &mut C,
        scheme: &str,
        body: &[u8],
    ) -> Result<Option<Transaction>, CoreError> {
        match scheme {
            "evm.ethereum.v0" => Ok(Some(
                raw_tx::decode(body, Some(Cfg::CHAIN_ID))
                    .map_err(CoreError::MalformedTransaction)?,
            )),
            _ => Ok(None),
        }
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn end_block<C: Context>(ctx: &mut C) {
        // Update the list of historic block hashes.
        let block_number = ctx.runtime_header().round;
        let block_hash = ctx.runtime_header().encoded_hash();
        let mut block_hashes = state::block_hashes(ctx.runtime_state());

        let current_number = block_number;
        block_hashes.insert(&block_number.to_be_bytes(), block_hash);

        if current_number > state::BLOCK_HASH_WINDOW_SIZE {
            let start_number = current_number - state::BLOCK_HASH_WINDOW_SIZE;
            block_hashes.remove(&start_number.to_be_bytes());
        }
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
