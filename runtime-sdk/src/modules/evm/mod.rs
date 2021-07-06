//! EVM module.
use thiserror::Error;

use evm::{
    executor::{MemoryStackState, StackExecutor, StackSubstateMetadata},
    Config,
};
use tiny_keccak::{Hasher, Keccak};

use crate::{
    context::{Context, TxContext},
    crypto::signature::PublicKey,
    error::{self, Error as _},
    module, storage,
    types::transaction::{AddressSpec, CallResult},
};

use types::{H160, H256, U256};

pub mod evm_backend;
pub mod types;

use evm::backend::ApplyBackend;

/// Unique module name.
const MODULE_NAME: &str = "evm";

pub struct Module;

/// Errors emitted by the EVM module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
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
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = ();
    type Parameters = ();
}

/// Interface that can be called from other modules.
pub trait API {
    /// Perform an Ethereum CREATE transaction.
    /// Returns 160-bit address of created contract.
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error>;

    /// Perform an Ethereum CALL transaction.
    fn call<C: TxContext>(
        ctx: &mut C,
        address: H160,
        value: U256,
        data: Vec<u8>,
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
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;
        Self::do_evm(caller, value, gas_limit, ctx, |exec| {
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
        gas_limit: u64,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;
        Self::do_evm(caller, value, gas_limit, ctx, |exec| {
            exec.transact_call(caller.into(), address.into(), value.into(), data, gas_limit)
        })
    }

    fn peek_storage<C: Context>(ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        let store =
            storage::PrefixStore::new(ctx.runtime_state(), &crate::modules::evm::MODULE_NAME);
        let storages =
            storage::PrefixStore::new(store, &crate::modules::evm::evm_backend::STORAGES);
        let s = storage::TypedStore::new(storage::PrefixStore::new(storages, &address));

        let result: H256 = s.get(&index).unwrap_or_default();

        Ok(result.as_bytes().to_vec())
    }

    fn peek_code<C: Context>(ctx: &mut C, address: H160) -> Result<Vec<u8>, Error> {
        let store =
            storage::PrefixStore::new(ctx.runtime_state(), &crate::modules::evm::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(
            store,
            &crate::modules::evm::evm_backend::CODES,
        ));

        Ok(codes.get(&address).unwrap_or_default())
    }
}

impl Module {
    const EVM_CONFIG: Config = Config::istanbul();
    const GAS_PRICE: primitive_types::U256 = primitive_types::U256::zero(); // TODO

    fn do_evm<C, F, V>(
        source: H160,
        value: U256,
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
        let vicinity = evm_backend::Vicinity {
            gas_price: Self::GAS_PRICE.into(),
            origin: source,
        };

        let mut backend = evm_backend::Backend::<'_, C>::new(vicinity, ctx);
        let metadata = StackSubstateMetadata::new(u64::max_value(), &Self::EVM_CONFIG);
        let stackstate = MemoryStackState::new(metadata, &backend);
        let mut executor = StackExecutor::new(stackstate, &Self::EVM_CONFIG);

        let (exit_reason, exit_value) = f(&mut executor);

        let gas_used = executor.used_gas();

        if gas_used > gas_limit {
            return Err(Error::GasLimitTooLow(gas_used));
        }

        let fee = executor.fee(Self::GAS_PRICE);
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
            U256::from_big_endian(&body.value),
            body.init_code,
            body.gas_limit,
        )
    }

    fn tx_call<C: TxContext>(ctx: &mut C, body: types::CallTx) -> Result<Vec<u8>, Error> {
        Self::call(
            ctx,
            H160::from_slice(&body.address),
            U256::from_big_endian(&body.value),
            body.data,
            body.gas_limit,
        )
    }

    fn q_peek_storage<C: Context>(
        ctx: &mut C,
        body: types::PeekStorageQuery,
    ) -> Result<Vec<u8>, Error> {
        Self::peek_storage(
            ctx,
            H160::from_slice(&body.address),
            H256::from_slice(&body.index),
        )
    }

    fn q_peek_code<C: Context>(ctx: &mut C, body: types::PeekCodeQuery) -> Result<Vec<u8>, Error> {
        Self::peek_code(ctx, H160::from_slice(&body.address))
    }
}

impl module::MethodHandler for Module {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "evm.Create" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_create(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "evm.Call" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_call(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            _ => module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, Result<cbor::Value, error::RuntimeError>> {
        match method {
            "evm.PeekStorage" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::q_peek_storage(ctx, args)?))
            })()),
            "evm.PeekCode" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::q_peek_code(ctx, args)?))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl module::MigrationHandler for Module {
    type Genesis = ();
}

impl module::AuthHandler for Module {}

impl module::BlockHandler for Module {}

impl module::InvariantHandler for Module {}
