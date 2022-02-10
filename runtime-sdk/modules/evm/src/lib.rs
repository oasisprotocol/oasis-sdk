//! EVM module.

pub mod backend;
pub mod derive_caller;
pub mod precompile;
pub mod raw_tx;
pub mod types;

use std::collections::BTreeMap;

use evm::{
    executor::stack::{MemoryStackState, PrecompileFn, StackExecutor, StackSubstateMetadata},
    Config as EVMConfig,
};
use once_cell::sync::Lazy;
use thiserror::Error;

use oasis_runtime_sdk::{
    context::{BatchContext, Context, TxContext},
    handler,
    module::{self, Module as _},
    modules::{
        self,
        accounts::API as _,
        core::{Error as CoreError, API as _},
    },
    runtime::Runtime,
    sdk_derive, storage,
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

    #[error("execution failed: {0}")]
    #[sdk_error(code = 2)]
    ExecutionFailed(String),

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

    #[error("reverted: {0}")]
    #[sdk_error(code = 8)]
    Reverted(String),

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] CoreError),
}

impl From<evm::ExitError> for Error {
    fn from(e: evm::ExitError) -> Error {
        use evm::ExitError::*;
        let msg = match e {
            StackUnderflow => "stack underflow",
            StackOverflow => "stack overflow",
            InvalidJump => "invalid jump",
            InvalidRange => "invalid range",
            DesignatedInvalid => "designated invalid",
            CallTooDeep => "call too deep",
            CreateCollision => "create collision",
            CreateContractLimit => "create contract limit",
            InvalidCode => "invalid code",

            OutOfOffset => "out of offset",
            OutOfGas => "out of gas",
            OutOfFund => "out of fund",

            #[allow(clippy::upper_case_acronyms)]
            PCUnderflow => "PC underflow",

            CreateEmpty => "create empty",

            Other(msg) => return Error::ExecutionFailed(msg.to_string()),
        };
        Error::ExecutionFailed(msg.to_string())
    }
}

impl From<evm::ExitFatal> for Error {
    fn from(e: evm::ExitFatal) -> Error {
        use evm::ExitFatal::*;
        let msg = match e {
            NotSupported => "not supported",
            UnhandledInterrupt => "unhandled interrupt",
            CallErrorAsFatal(err) => return err.into(),
            Other(msg) => return Error::ExecutionFailed(msg.to_string()),
        };
        Error::ExecutionFailed(msg.to_string())
    }
}

/// Process an EVM result to return either a successful result or a (readable) error reason.
fn process_evm_result(exit_reason: evm::ExitReason, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    match exit_reason {
        evm::ExitReason::Succeed(_) => Ok(data),
        evm::ExitReason::Revert(_) => {
            // Decode revert reason, format is as follows:
            //
            // 08c379a0                                                         <- Function selector
            // 0000000000000000000000000000000000000000000000000000000000000020 <- Offset of string return value
            // 0000000000000000000000000000000000000000000000000000000000000047 <- Length of string return value (the revert reason)
            // 6d7946756e6374696f6e206f6e6c79206163636570747320617267756d656e74 <- First 32 bytes of the revert reason
            // 7320776869636820617265206772656174686572207468616e206f7220657175 <- Next 32 bytes of the revert reason
            // 616c20746f203500000000000000000000000000000000000000000000000000 <- Last 7 bytes of the revert reason
            //
            const ERROR_STRING_SELECTOR: &[u8] = &[0x08, 0xc3, 0x79, 0xa0]; // Keccak256("Error(string)")
            const FIELD_OFFSET_START: usize = 4;
            const FIELD_LENGTH_START: usize = FIELD_OFFSET_START + 32;
            const FIELD_REASON_START: usize = FIELD_LENGTH_START + 32;
            const MIN_SIZE: usize = FIELD_REASON_START;
            const MAX_REASON_SIZE: usize = 1024;
            if data.len() < MIN_SIZE || !data.starts_with(ERROR_STRING_SELECTOR) {
                // TODO: Could also return Base64-encoded raw reason?
                return Err(Error::Reverted("unknown".to_string()));
            }
            // Decode and validate length.
            let mut length =
                primitive_types::U256::from(&data[FIELD_LENGTH_START..FIELD_LENGTH_START + 32])
                    .low_u32() as usize;
            if FIELD_REASON_START + length > data.len() {
                // TODO: Could also return Base64-encoded raw reason?
                return Err(Error::Reverted("unknown".to_string()));
            }
            // Make sure that this doesn't ever return huge reason values as this is at least
            // somewhat contract-controlled.
            if length > MAX_REASON_SIZE {
                length = MAX_REASON_SIZE;
            }
            let reason =
                String::from_utf8_lossy(&data[FIELD_REASON_START..FIELD_REASON_START + length]);
            Err(Error::Reverted(reason.to_string()))
        }
        evm::ExitReason::Error(err) => Err(err.into()),
        evm::ExitReason::Fatal(err) => Err(err.into()),
    }
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

        let rsp = Self::do_evm(
            caller,
            ctx,
            |exec, gas_limit| {
                let address = exec.create_address(evm::CreateScheme::Legacy {
                    caller: caller.into(),
                });
                (
                    exec.transact_create(caller.into(), value.into(), init_code, gas_limit, vec![]),
                    address.as_bytes().to_vec(),
                )
            },
            // If in simulation, this must be EstimateGas query.
            ctx.is_simulation(),
        );

        // Always return success in CheckTx, as we might not have up-to-date state.
        if ctx.is_check_only() {
            rsp.or_else(|_| Ok(vec![]))
        } else {
            rsp
        }
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

        let rsp = Self::do_evm(
            caller,
            ctx,
            |exec, gas_limit| {
                exec.transact_call(
                    caller.into(),
                    address.into(),
                    value.into(),
                    data,
                    gas_limit,
                    vec![],
                )
            },
            // If in simulation, this must be EstimateGas query.
            ctx.is_simulation(),
        );

        // Always return success in CheckTx, as we might not have up-to-date state.
        if ctx.is_check_only() {
            rsp.or_else(|_| Ok(vec![]))
        } else {
            rsp
        }
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
                Self::do_evm(
                    caller,
                    &mut txctx,
                    |exec, gas_limit| {
                        exec.transact_call(
                            caller.into(),
                            address.into(),
                            value.into(),
                            data,
                            gas_limit,
                            vec![],
                        )
                    },
                    // Simulate call is never called from EstimateGas.
                    false,
                )
            })
        })
    }
}

// Config used by the EVM.
static EVM_CONFIG: EVMConfig = EVMConfig::london();
// Config used by the EVM for estimation.
static EVM_CONFIG_ESTIMATE: Lazy<EVMConfig> = Lazy::new(|| {
    let mut cfg = EVM_CONFIG.clone();
    // Without `estimate=true` the EVM underestimates the gas needed for transactions using CREATE calls: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-150.md
    // However by having `estimate=true` the said transaction costs are GREATLY overestimated in the estimateGas call: ~by around 1/64 of the estimate call gas_limit.
    // https://github.com/rust-blockchain/evm/issues/8
    cfg.estimate = true;
    cfg
});

impl<Cfg: Config> Module<Cfg> {
    fn do_evm<C, F>(source: H160, ctx: &mut C, f: F, estimate_gas: bool) -> Result<Vec<u8>, Error>
    where
        F: FnOnce(
            &mut StackExecutor<
                'static,
                '_,
                MemoryStackState<'_, 'static, backend::Backend<'_, C, Cfg>>,
                BTreeMap<primitive_types::H160, PrecompileFn>,
            >,
            u64,
        ) -> (evm::ExitReason, Vec<u8>),
        C: TxContext,
    {
        let cfg = if estimate_gas {
            &*EVM_CONFIG_ESTIMATE
        } else {
            &EVM_CONFIG
        };

        let gas_limit: u64 = <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx);
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
        let metadata = StackSubstateMetadata::new(gas_limit, cfg);
        let stackstate = MemoryStackState::new(metadata, &backend);
        let mut executor = StackExecutor::new_with_precompiles(
            stackstate,
            cfg,
            &*precompile::PRECOMPILED_CONTRACT,
        );

        // Run EVM and process the result.
        let (exit_reason, exit_value) = f(&mut executor, gas_limit);
        let gas_used = executor.used_gas();
        let fee = executor.fee(gas_price);

        let exit_value = match process_evm_result(exit_reason, exit_value) {
            Ok(exit_value) => exit_value,
            Err(err) => {
                <C::Runtime as Runtime>::Core::use_tx_gas(ctx, gas_used)?;
                return Err(err);
            }
        };

        // Return the difference between the pre-paid max_gas and actually used gas.
        let return_fee = max_gas_fee
            .checked_sub(fee)
            .ok_or(Error::InsufficientBalance)?;

        let (vals, logs) = executor.into_state().deconstruct();
        backend.apply(vals, logs, true);

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, gas_used)?;

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
}

#[sdk_derive(MethodHandler)]
impl<Cfg: Config> Module<Cfg> {
    #[handler(call = "evm.Create")]
    fn tx_create<C: TxContext>(ctx: &mut C, body: types::Create) -> Result<Vec<u8>, Error> {
        Self::create(ctx, body.value, body.init_code)
    }

    #[handler(call = "evm.Call")]
    fn tx_call<C: TxContext>(ctx: &mut C, body: types::Call) -> Result<Vec<u8>, Error> {
        Self::call(ctx, body.address, body.value, body.data)
    }

    #[handler(query = "evm.Storage")]
    fn query_storage<C: Context>(ctx: &mut C, body: types::StorageQuery) -> Result<Vec<u8>, Error> {
        Self::get_storage(ctx, body.address, body.index)
    }

    #[handler(query = "evm.Code")]
    fn query_code<C: Context>(ctx: &mut C, body: types::CodeQuery) -> Result<Vec<u8>, Error> {
        Self::get_code(ctx, body.address)
    }

    #[handler(query = "evm.Balance")]
    fn query_balance<C: Context>(ctx: &mut C, body: types::BalanceQuery) -> Result<u128, Error> {
        Self::get_balance(ctx, body.address)
    }

    #[handler(query = "evm.SimulateCall")]
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
