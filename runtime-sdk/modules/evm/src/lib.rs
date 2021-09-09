//! EVM module.
pub mod evm_backend;
pub mod types;

use evm::{
    executor::{MemoryStackState, StackExecutor, StackSubstateMetadata},
    Config,
};
use thiserror::Error;
use tiny_keccak::{Hasher, Keccak};

use oasis_runtime_sdk::{
    context::{Context, TxContext},
    crypto::signature::PublicKey,
    error,
    module::{self, CallResult, Module as _},
    modules, storage,
    types::transaction::AddressSpec,
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

pub struct Module;

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

    #[error("fee withdrawal failed")]
    #[sdk_error(code = 5)]
    FeeWithdrawalFailed,

    #[error("gas limit too low: {0} required")]
    #[sdk_error(code = 6)]
    GasLimitTooLow(u64),

    #[error("gas price too low")]
    #[sdk_error(code = 7)]
    GasPriceTooLow,
}

/// Parameters for the EVM module.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    /// Minimum acceptable gas price for EVM transactions.
    pub min_gas_price: U256,
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

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

/// Interface that can be called from other modules.
pub trait API {
    /// Perform an Ethereum CREATE transaction.
    /// Returns 160-bit address of created contract.
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
        gas_price: U256,
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error>;

    /// Perform an Ethereum CALL transaction.
    fn call<C: TxContext>(
        ctx: &mut C,
        address: H160,
        value: U256,
        data: Vec<u8>,
        gas_price: U256,
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error>;

    /// Peek into EVM storage.
    /// Returns 256-bit value stored at given contract address and index (slot)
    /// in the storage.
    fn peek_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error>;

    /// Peek into EVM code storage.
    /// Returns EVM bytecode of contract at given address.
    fn peek_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error>;
}

impl API for Module {
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
        gas_price: U256,
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;
        Self::do_evm(caller, value, gas_price, gas_limit, ctx, |exec| {
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
        gas_price: U256,
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;
        Self::do_evm(caller, value, gas_price, gas_limit, ctx, |exec| {
            exec.transact_call(caller.into(), address.into(), value.into(), data, gas_limit)
        })
    }

    fn peek_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &state::STORAGES);
        let s = storage::TypedStore::new(storage::PrefixStore::new(storages, &address));

        let result: H256 = s.get(&index).unwrap_or_default();

        Ok(result.as_bytes().to_vec())
    }

    fn peek_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error> {
        let store = storage::PrefixStore::new(ctx.runtime_state(), &crate::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &state::CODES));

        Ok(codes.get(&address).unwrap_or_default())
    }
}

impl Module {
    const EVM_CONFIG: Config = Config::istanbul();

    fn do_evm<C, F, V>(
        source: H160,
        value: U256,
        gas_price_in: U256,
        gas_limit: u64,
        ctx: &mut C,
        f: F,
    ) -> Result<V, Error>
    where
        F: FnOnce(
            &mut StackExecutor<'static, MemoryStackState<'_, 'static, evm_backend::Backend<'_, C>>>,
        ) -> (evm::ExitReason, V),
        C: Context,
    {
        // Make sure that gas_price >= min_gas_price.
        let params = Self::params(ctx.runtime_state());
        let gas_price: primitive_types::U256 = gas_price_in.into();
        let min_gas_price: primitive_types::U256 = params.min_gas_price.into();
        if gas_price < min_gas_price {
            return Err(Error::GasPriceTooLow);
        }

        let vicinity = evm_backend::Vicinity {
            gas_price: gas_price_in,
            origin: source,
        };

        let mut backend = evm_backend::Backend::<'_, C>::new(vicinity, ctx);
        let metadata = StackSubstateMetadata::new(gas_limit, &Self::EVM_CONFIG);
        let stackstate = MemoryStackState::new(metadata, &backend);
        let mut executor = StackExecutor::new(stackstate, &Self::EVM_CONFIG);

        let (exit_reason, exit_value) = f(&mut executor);

        let gas_used = executor.used_gas();

        if gas_used > gas_limit {
            return Err(Error::GasLimitTooLow(gas_used));
        }

        let fee = executor.fee(gas_price);
        let total_fee = fee.checked_add(value.into()).ok_or(Error::FeeOverflow)?;
        executor
            .state_mut()
            .withdraw(source.into(), total_fee)
            .map_err(|_| Error::FeeWithdrawalFailed)?;

        let (vals, logs) = executor.into_state().deconstruct();
        backend.apply(vals, logs, true);

        if exit_reason.is_succeed() {
            Ok(exit_value)
        } else {
            Err(Error::EVMError(format!("{:?}", exit_reason)))
        }
    }

    fn derive_caller<C>(ctx: &mut C) -> Result<H160, Error>
    where
        C: TxContext,
    {
        match &ctx.tx_auth_info().signer_info[0].address_spec {
            AddressSpec::Signature(PublicKey::Secp256k1(pk)) => {
                // Caller address is derived by doing Keccak-256 on the
                // secp256k1 public key and taking the last 20 bytes
                // of the result.
                let mut k = Keccak::v256();
                let mut out = [0u8; 32];
                k.update(pk.as_bytes());
                k.finalize(&mut out);
                Ok(H160::from_slice(&out[32 - 20..]))
            }
            _ => Err(Error::InvalidSignerType),
        }
    }

    fn tx_create<C: TxContext>(ctx: &mut C, body: types::CreateTx) -> Result<Vec<u8>, Error> {
        Self::create(
            ctx,
            body.value,
            body.init_code,
            body.gas_price,
            body.gas_limit,
        )
    }

    fn tx_call<C: TxContext>(ctx: &mut C, body: types::CallTx) -> Result<Vec<u8>, Error> {
        Self::call(
            ctx,
            body.address,
            body.value,
            body.data,
            body.gas_price,
            body.gas_limit,
        )
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

impl module::MethodHandler for Module {
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
            "evm.PeekStorage" => module::dispatch_query(ctx, args, Self::q_peek_storage),
            "evm.PeekCode" => module::dispatch_query(ctx, args, Self::q_peek_code),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl module::MigrationHandler for Module {
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

impl module::AuthHandler for Module {}

impl module::BlockHandler for Module {
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

impl module::InvariantHandler for Module {}
