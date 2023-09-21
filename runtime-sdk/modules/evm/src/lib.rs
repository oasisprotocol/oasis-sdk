//! EVM module.
#![feature(array_chunks)]
#![feature(test)]

extern crate substrate_bn as bn;

pub mod backend;
pub mod derive_caller;
pub mod precompile;
pub mod raw_tx;
mod signed_call;
pub mod state;
pub mod types;

use evm::{
    executor::stack::{StackExecutor, StackSubstateMetadata},
    Config as EVMConfig,
};
use once_cell::sync::OnceCell;
use thiserror::Error;

use oasis_runtime_sdk::{
    callformat,
    context::{BatchContext, Context, TransactionWithMeta, TxContext},
    handler, migration,
    module::{self, Module as _},
    modules::{
        self,
        accounts::API as _,
        core::{Error as CoreError, API as _},
    },
    runtime::Runtime,
    sdk_derive,
    storage::CurrentStore,
    types::{
        address::{self, Address},
        token, transaction,
        transaction::Transaction,
    },
};

use types::{H160, H256, U256};

#[cfg(any(test, feature = "test"))]
pub mod mock;
#[cfg(test)]
mod test;

/// Unique module name.
const MODULE_NAME: &str = "evm";

/// Module configuration.
pub trait Config: 'static {
    /// Module that is used for accessing accounts.
    type Accounts: modules::accounts::API;

    /// AdditionalPrecompileSet is the type used for the additional precompiles.
    /// Use `()` if unused.
    type AdditionalPrecompileSet: evm::executor::stack::PrecompileSet;

    /// The chain ID to supply when a contract requests it. Ethereum-format transactions must use
    /// this chain ID.
    const CHAIN_ID: u64;

    /// Token denomination used as the native EVM token.
    const TOKEN_DENOMINATION: token::Denomination;

    /// Whether to use confidential storage by default, and transaction data encryption.
    const CONFIDENTIAL: bool = false;

    /// Whether to refund unused transaction fee.
    const REFUND_UNUSED_FEE: bool = true;

    /// Maximum result size in bytes.
    const MAX_RESULT_SIZE: usize = 1024;

    /// Maps an Ethereum address into an SDK account address.
    fn map_address(address: primitive_types::H160) -> Address {
        Address::new(
            address::ADDRESS_V0_SECP256K1ETH_CONTEXT,
            address::ADDRESS_V0_VERSION,
            address.as_ref(),
        )
    }

    /// Provides additional precompiles that should be available to the EVM.
    ///
    /// If any of the precompile addresses returned is the same as for one of
    /// the builtin precompiles, then the returned implementation will
    /// overwrite the builtin implementation.
    fn additional_precompiles() -> Option<Self::AdditionalPrecompileSet> {
        None
    }

    /// Returns the config used by the EVM (in the hardfork sense).
    // In some cases, the config may be runtime config dependent (e.g., constant
    // timeness when confidential), so this is made part of the trait.
    fn evm_config(estimation: bool) -> &'static EVMConfig {
        static EVM_CONFIG: OnceCell<EVMConfig> = OnceCell::new();
        static EVM_CONFIG_ESTIMATE: OnceCell<EVMConfig> = OnceCell::new();

        if estimation {
            EVM_CONFIG_ESTIMATE.get_or_init(|| {
                // The estimate mode overestimates transaction costs and returns a gas costs
                // that should be sufficient to execute a transaction, but likely overestimated.
                // The "proper" EVM-way to estimate exact gas is to disable this estimation and
                // do a binary search over all possible gas costs to find the minimum gas cost
                // with which the transaction succeeds. This mode should only be used when the
                // caller wants to avoid the expensive binary search and is ok with a possible
                // overestimation of gas costs.
                EVMConfig {
                    estimate: true,
                    ..Self::evm_config(false).clone()
                }
            })
        } else {
            EVM_CONFIG.get_or_init(EVMConfig::shanghai)
        }
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

    #[error("forbidden by policy: this node only allows simulating calls that use up to {0} gas")]
    #[sdk_error(code = 9)]
    SimulationTooExpensive(u64),

    #[error("invalid signed simulate call query: {0}")]
    #[sdk_error(code = 10)]
    InvalidSignedSimulateCall(&'static str),

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
            InvalidCode(..) => "invalid code",

            OutOfOffset => "out of offset",
            OutOfGas => "out of gas",
            OutOfFund => "out of funds",

            #[allow(clippy::upper_case_acronyms)]
            PCUnderflow => "PC underflow",

            CreateEmpty => "create empty",
            MaxNonce => "nonce overflow",

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

/// Local configuration that can be provided by the node operator.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct LocalConfig {
    /// Maximum gas limit that can be passed to the `evm.SimulateCall` query. Queries
    /// with a higher gas limit will be rejected. A special value of `0` indicates
    /// no limit. Default: 0.
    #[cbor(optional)]
    pub query_simulate_call_max_gas: u64,
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
    ///
    /// If the EVM is confidential, it may accept _signed queries_, which are formatted as
    /// an either a [`sdk::types::transaction::Call`] or [`types::SignedCallDataPack`] encoded
    /// and packed into the `data` field of the [`types::SimulateCallQuery`].
    fn simulate_call<C: Context>(
        ctx: &mut C,
        call: types::SimulateCallQuery,
    ) -> Result<Vec<u8>, Error>;
}

impl<Cfg: Config> API for Module<Cfg> {
    fn create<C: TxContext>(
        ctx: &mut C,
        value: U256,
        init_code: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        // Create output (the contract address) does not need to be encrypted because it's
        // trivially computable by anyone who can observe the create tx and receipt status.
        // Therefore, we don't need the `tx_metadata` or to encode the result.
        let (init_code, _tx_metadata) =
            Self::decode_call_data(ctx, init_code, ctx.tx_call_format(), ctx.tx_index(), true)?
                .expect("processing always proceeds");

        Self::do_evm(
            caller,
            ctx,
            |exec, gas_limit| {
                let address = exec.create_address(evm::CreateScheme::Legacy {
                    caller: caller.into(),
                });
                let (exit_reason, exit_value) =
                    exec.transact_create(caller.into(), value.into(), init_code, gas_limit, vec![]);
                if exit_reason.is_succeed() {
                    // If successful return the contract deployed address.
                    (exit_reason, address.as_bytes().to_vec())
                } else {
                    // Otherwise propagate the exit value.
                    (exit_reason, exit_value)
                }
            },
            // If in simulation, this must be EstimateGas query.
            // Use estimate mode if not doing binary search for exact gas costs.
            ctx.is_simulation()
                && <C::Runtime as Runtime>::Core::estimate_gas_search_max_iters(ctx) == 0,
        )
    }

    fn call<C: TxContext>(
        ctx: &mut C,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller(ctx)?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        let (data, tx_metadata) =
            Self::decode_call_data(ctx, data, ctx.tx_call_format(), ctx.tx_index(), true)?
                .expect("processing always proceeds");

        Self::do_evm(
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
            // Use estimate mode if not doing binary search for exact gas costs.
            ctx.is_simulation()
                && <C::Runtime as Runtime>::Core::estimate_gas_search_max_iters(ctx) == 0,
        )?;
        Ok(vec![])
    }

    fn get_storage<C: Context>(_ctx: &mut C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        state::with_public_storage(&address, |store| {
            let result: H256 = store.get(index).unwrap_or_default();
            Ok(result.as_bytes().to_vec())
        })
    }

    fn get_code<C: Context>(_ctx: &mut C, address: H160) -> Result<Vec<u8>, Error> {
        CurrentStore::with(|store| {
            let codes = state::codes(store);
            Ok(codes.get(address).unwrap_or_default())
        })
    }

    fn get_balance<C: Context>(_ctx: &mut C, address: H160) -> Result<u128, Error> {
        let address = Cfg::map_address(address.into());
        Ok(Cfg::Accounts::get_balance(address, Cfg::TOKEN_DENOMINATION).unwrap_or_default())
    }

    fn simulate_call<C: Context>(
        ctx: &mut C,
        call: types::SimulateCallQuery,
    ) -> Result<Vec<u8>, Error> {
        let (
            types::SimulateCallQuery {
                gas_price,
                gas_limit,
                caller,
                address,
                value,
                data,
            },
            tx_metadata,
        ) = Self::decode_simulate_call_query(ctx, call)?;

        let evm_result = ctx.with_simulation(|mut sctx| {
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
                    ..Default::default()
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
                    ..Default::default()
                },
            };
            sctx.with_tx(
                TransactionWithMeta::internal(call_tx),
                |mut txctx, _call| {
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
                },
            )
        });
        Self::encode_evm_result(ctx, evm_result, tx_metadata)
    }
}

impl<Cfg: Config> Module<Cfg> {
    fn do_evm<C, F>(source: H160, ctx: &mut C, f: F, estimate_gas: bool) -> Result<Vec<u8>, Error>
    where
        F: FnOnce(
            &mut StackExecutor<
                'static,
                '_,
                backend::OasisStackState<'_, '_, 'static, C, Cfg>,
                precompile::Precompiles<Cfg, backend::OasisBackend<'_, C, Cfg>>,
            >,
            u64,
        ) -> (evm::ExitReason, Vec<u8>),
        C: TxContext,
    {
        let is_query = ctx.is_check_only();
        let cfg = Cfg::evm_config(estimate_gas);
        let gas_limit: u64 = <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx);
        let gas_price: primitive_types::U256 = ctx.tx_auth_info().fee.gas_price().into();

        let vicinity = backend::Vicinity {
            gas_price,
            origin: source.into(),
        };

        let backend = backend::OasisBackend::<'_, C, Cfg>::new(ctx, vicinity);
        let metadata = StackSubstateMetadata::new(gas_limit, cfg);
        let stackstate = backend::OasisStackState::new(metadata, &backend);
        let precompiles = precompile::Precompiles::new(&backend);
        let mut executor = StackExecutor::new_with_precompiles(stackstate, cfg, &precompiles);

        // Run EVM and process the result.
        let (exit_reason, exit_value) = f(&mut executor, gas_limit);
        let gas_used = executor.used_gas();

        // Clamp data based on maximum allowed result size.
        let exit_value = if !is_query && exit_value.len() > Cfg::MAX_RESULT_SIZE {
            exit_value[..Cfg::MAX_RESULT_SIZE].to_vec()
        } else {
            exit_value
        };

        let exit_value = match exit_reason {
            evm::ExitReason::Succeed(_) => exit_value,
            err => {
                let err = match err {
                    evm::ExitReason::Revert(_) => Error::Reverted(base64::encode(exit_value)),
                    evm::ExitReason::Error(err) => err.into(),
                    evm::ExitReason::Fatal(err) => err.into(),
                    _ => unreachable!("already handled above"),
                };

                <C::Runtime as Runtime>::Core::use_tx_gas(ctx, gas_used)?;
                Cfg::Accounts::set_refund_unused_tx_fee(ctx, Cfg::REFUND_UNUSED_FEE);
                return Err(err);
            }
        };

        // Apply can fail in case of unsupported actions.
        if let Err(err) = executor.into_state().apply() {
            <C::Runtime as Runtime>::Core::use_tx_gas(ctx, gas_used)?;
            return Err(err); // Do not refund unused fee.
        };

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, gas_used)?;
        Cfg::Accounts::set_refund_unused_tx_fee(ctx, Cfg::REFUND_UNUSED_FEE);

        Ok(exit_value)
    }

    fn derive_caller<C>(ctx: &C) -> Result<H160, Error>
    where
        C: TxContext,
    {
        derive_caller::from_tx_auth_info(ctx.tx_auth_info())
    }

    /// Returns the decrypted call data or `None` if this transaction is simulated in
    /// a context that may not include a key manager (i.e. SimulateCall but not EstimateGas).
    fn decode_call_data<C: Context>(
        ctx: &C,
        data: Vec<u8>,
        format: transaction::CallFormat, // The tx call format.
        tx_index: usize,
        assume_km_reachable: bool,
    ) -> Result<Option<(Vec<u8>, callformat::Metadata)>, Error> {
        if !Cfg::CONFIDENTIAL || format != transaction::CallFormat::Plain {
            // Either the runtime is non-confidential and all txs are plaintext, or the tx
            // is sent using a confidential call format and the tx has already been decrypted.
            return Ok(Some((data, callformat::Metadata::Empty)));
        }
        match cbor::from_slice(&data) {
            Ok(call) => Self::decode_call(ctx, call, tx_index, assume_km_reachable),
            Err(_) => Ok(Some((data, callformat::Metadata::Empty))), // It's not encrypted.
        }
    }

    /// Returns the decrypted call data or `None` if this transaction is simulated in
    /// a context that may not include a key manager (i.e. SimulateCall but not EstimateGas).
    fn decode_call<C: Context>(
        ctx: &C,
        call: transaction::Call,
        tx_index: usize,
        assume_km_reachable: bool,
    ) -> Result<Option<(Vec<u8>, callformat::Metadata)>, Error> {
        match callformat::decode_call_ex(ctx, call, tx_index, assume_km_reachable)? {
            Some((
                transaction::Call {
                    body: cbor::Value::ByteString(data),
                    ..
                },
                metadata,
            )) => Ok(Some((data, metadata))),
            Some((_, _)) => {
                Err(CoreError::InvalidCallFormat(anyhow::anyhow!("invalid inner data")).into())
            }
            None => Ok(None),
        }
    }

    fn decode_simulate_call_query<C: Context>(
        ctx: &C,
        call: types::SimulateCallQuery,
    ) -> Result<(types::SimulateCallQuery, callformat::Metadata), Error> {
        if !Cfg::CONFIDENTIAL {
            return Ok((call, callformat::Metadata::Empty));
        }

        if let Ok(types::SignedCallDataPack {
            data,
            leash,
            signature,
        }) = cbor::from_slice(&call.data)
        {
            let (data, tx_metadata) =
                Self::decode_call(ctx, data, 0, true)?.expect("processing always proceeds");
            return Ok((
                signed_call::verify::<_, Cfg>(
                    ctx,
                    types::SimulateCallQuery { data, ..call },
                    leash,
                    signature,
                )?,
                tx_metadata,
            ));
        }

        // The call is not signed, but it must be encoded as an oasis-sdk call.
        let tx_call_format = transaction::CallFormat::Plain; // Queries cannot be encrypted.
        let (data, tx_metadata) = Self::decode_call_data(ctx, call.data, tx_call_format, 0, true)?
            .expect("processing always proceeds");
        Ok((
            types::SimulateCallQuery {
                caller: Default::default(), // The sender cannot be spoofed.
                data,
                ..call
            },
            tx_metadata,
        ))
    }

    fn encode_evm_result<C: Context>(
        ctx: &C,
        evm_result: Result<Vec<u8>, Error>,
        tx_metadata: callformat::Metadata, // Potentially parsed from an inner enveloped tx.
    ) -> Result<Vec<u8>, Error> {
        if matches!(tx_metadata, callformat::Metadata::Empty) {
            // Either the runtime is non-confidential and all responses are plaintext,
            // or the tx was sent using a confidential call format and dispatcher will
            // encrypt the call in the normal way.
            return evm_result;
        }
        // Always propagate errors in plaintext.
        let call_result = module::CallResult::Ok(evm_result?.into());
        Ok(cbor::to_vec(callformat::encode_result_ex(
            ctx,
            call_result,
            tx_metadata,
            true,
        )))
    }
}

#[sdk_derive(Module)]
impl<Cfg: Config> Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 2;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    fn init(genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(genesis.parameters);
    }

    #[migration(from = 1)]
    fn migrate_v1_to_v2() {
        // No state migration is needed for v2.
    }

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

    #[handler(query = "evm.SimulateCall", expensive, allow_private_km)]
    fn query_simulate_call<C: Context>(
        ctx: &mut C,
        body: types::SimulateCallQuery,
    ) -> Result<Vec<u8>, Error> {
        let cfg: LocalConfig = ctx.local_config(MODULE_NAME).unwrap_or_default();
        if cfg.query_simulate_call_max_gas > 0 && body.gas_limit > cfg.query_simulate_call_max_gas {
            return Err(Error::SimulationTooExpensive(
                cfg.query_simulate_call_max_gas,
            ));
        }
        Self::simulate_call(ctx, body)
    }
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {
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
        CurrentStore::with(|store| {
            // Update the list of historic block hashes.
            let block_number = ctx.runtime_header().round;
            let block_hash = ctx.runtime_header().encoded_hash();
            let mut block_hashes = state::block_hashes(store);

            let current_number = block_number;
            block_hashes.insert(block_number.to_be_bytes(), block_hash);

            if current_number > state::BLOCK_HASH_WINDOW_SIZE {
                let start_number = current_number - state::BLOCK_HASH_WINDOW_SIZE;
                block_hashes.remove(start_number.to_be_bytes());
            }
        });
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
