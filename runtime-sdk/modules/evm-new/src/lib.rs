//! EVM module.
#![feature(array_chunks)]
#![feature(test)]

pub mod db;
pub mod derive_caller;
pub mod precompile;
pub mod raw_tx;
mod signed_call;
pub mod state;
pub mod types;

use std::sync::Arc;

use base64::prelude::*;
use revm::{
    precompile::PrecompileWithAddress,
    primitives::{Bytes, ExecutionResult, Output, SpecId, TxEnv, TxKind},
    Evm,
};

use oasis_runtime_sdk::{
    callformat,
    context::Context,
    handler, migration,
    module::{self, Module as _},
    modules::{
        accounts::API as _,
        core::{Error as CoreError, API as _},
    },
    runtime::Runtime,
    sdk_derive,
    state::{CurrentState, Mode, Options, TransactionResult, TransactionWithMeta},
    types::{
        address::{self, Address},
        token, transaction,
        transaction::Transaction,
    },
};
use thiserror::Error;
use types::{H160, H256, U256};

/// Unique module name.
/// Note: We use the same name here as the old EVM module, so that we can
/// access the same storage.
const MODULE_NAME: &str = "evm";

#[cfg(any(test, feature = "test"))]
pub mod mock;
#[cfg(test)]
mod test;

/// Module configuration.
pub trait Config: 'static {
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
    fn map_address(address: revm::primitives::Address) -> Address {
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
    fn additional_precompiles() -> Option<Vec<PrecompileWithAddress>> {
        None
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
    fn create<C: Context>(ctx: &C, value: U256, init_code: Vec<u8>) -> Result<Vec<u8>, Error>;

    /// Perform an Ethereum CALL transaction.
    fn call<C: Context>(
        ctx: &C,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error>;

    /// Peek into EVM storage.
    /// Returns 256-bit value stored at given contract address and index (slot)
    /// in the storage.
    fn get_storage<C: Context>(ctx: &C, address: H160, index: H256) -> Result<Vec<u8>, Error>;

    /// Peek into EVM code storage.
    /// Returns EVM bytecode of contract at given address.
    fn get_code<C: Context>(ctx: &C, address: H160) -> Result<Vec<u8>, Error>;

    /// Get EVM account balance.
    fn get_balance<C: Context>(ctx: &C, address: H160) -> Result<u128, Error>;

    /// Simulate an Ethereum CALL.
    ///
    /// If the EVM is confidential, it may accept _signed queries_, which are
    /// formatted as either a [`sdk::types::transaction::Call`] or
    /// [`types::SignedCallDataPack`] encoded and packed into the `data` field
    /// of [`types::SimulateCallQuery`].
    fn simulate_call<C: Context>(ctx: &C, call: types::SimulateCallQuery)
        -> Result<Vec<u8>, Error>;
}

impl<Cfg: Config> API for Module<Cfg> {
    fn create<C: Context>(ctx: &C, value: U256, init_code: Vec<u8>) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller()?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        let (tx_call_format, tx_index, is_simulation) = CurrentState::with_env(|env| {
            (env.tx_call_format(), env.tx_index(), env.is_simulation())
        });

        // Create output (the contract address) does not need to be encrypted because it's
        // trivially computable by anyone who can observe the create tx and receipt status.
        // Therefore, we don't need the `tx_metadata` or to encode the result.
        let (init_code, _tx_metadata) =
            Self::decode_call_data(ctx, init_code, tx_call_format, tx_index, true)?
                .expect("processing always proceeds");

        // If in simulation, this must be EstimateGas query.
        // Use estimate mode if not doing binary search for exact gas costs.
        let estimate_gas =
            is_simulation && <C::Runtime as Runtime>::Core::estimate_gas_search_max_iters(ctx) == 0;

        Self::evm_create(ctx, caller, value, init_code, estimate_gas)
    }

    fn call<C: Context>(
        ctx: &C,
        address: H160,
        value: U256,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        let caller = Self::derive_caller()?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(vec![]);
        }

        let (tx_call_format, tx_index, is_simulation) = CurrentState::with_env(|env| {
            (env.tx_call_format(), env.tx_index(), env.is_simulation())
        });

        let (data, tx_metadata) =
            Self::decode_call_data(ctx, data, tx_call_format, tx_index, true)?
                .expect("processing always proceeds");

        // If in simulation, this must be EstimateGas query.
        // Use estimate mode if not doing binary search for exact gas costs.
        let estimate_gas =
            is_simulation && <C::Runtime as Runtime>::Core::estimate_gas_search_max_iters(ctx) == 0;

        let evm_result = Self::evm_call(ctx, caller, address, value, data, estimate_gas);
        Self::encode_evm_result(ctx, evm_result, tx_metadata)
    }

    fn get_storage<C: Context>(_ctx: &C, address: H160, index: H256) -> Result<Vec<u8>, Error> {
        state::with_public_storage(&address, |store| {
            let result: H256 = store.get(index).unwrap_or_default();
            Ok(result.as_bytes().to_vec())
        })
    }

    fn get_code<C: Context>(_ctx: &C, address: H160) -> Result<Vec<u8>, Error> {
        CurrentState::with_store(|store| {
            let codes = state::codes(store);
            Ok(codes.get(address).unwrap_or_default())
        })
    }

    fn get_balance<C: Context>(_ctx: &C, address: H160) -> Result<u128, Error> {
        let address = Cfg::map_address(address.0.into());
        Ok(
            <C::Runtime as Runtime>::Accounts::get_balance(address, Cfg::TOKEN_DENOMINATION)
                .unwrap_or_default(),
        )
    }

    fn simulate_call<C: Context>(
        ctx: &C,
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

        let (method, body, exec): (_, _, Box<dyn FnOnce() -> Result<_, _>>) = match address {
            Some(address) => {
                // Address is set, this is a simulated `evm.Call`.
                (
                    "evm.Call",
                    cbor::to_value(types::Call {
                        address,
                        value,
                        data: data.clone(),
                    }),
                    Box::new(move || Self::evm_call(ctx, caller, address, value, data, false)),
                )
            }
            None => {
                // Address is not set, this is a simulated `evm.Create`.
                (
                    "evm.Create",
                    cbor::to_value(types::Create {
                        value,
                        init_code: data.clone(),
                    }),
                    Box::new(|| Self::evm_create(ctx, caller, value, data, false)),
                )
            }
        };
        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                format: transaction::CallFormat::Plain,
                method: method.to_owned(),
                body,
                ..Default::default()
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![transaction::SignerInfo {
                    address_spec: transaction::AddressSpec::Internal(
                        transaction::CallerAddress::EthAddress(caller.into()),
                    ),
                    nonce: 0,
                }],
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
                    proxy: None,
                },
                ..Default::default()
            },
        };

        let evm_result = CurrentState::with_transaction_opts(
            Options::new()
                .with_tx(TransactionWithMeta::internal(tx))
                .with_mode(Mode::Simulate),
            || TransactionResult::Rollback(exec()),
        );
        Self::encode_evm_result(ctx, evm_result, tx_metadata)
    }
}

impl<Cfg: Config> Module<Cfg> {
    fn evm_execute<C: Context, F: FnOnce(&mut TxEnv)>(
        ctx: &C,
        estimate_gas: bool,
        f: F,
    ) -> Result<Vec<u8>, Error> {
        let is_query = CurrentState::with_env(|env| !env.is_execute());

        // Prepare the environment.
        let db = db::OasisDB::<'_, C, Cfg>::new(ctx);
        let mut evm = Evm::builder()
            .with_db(db)
            .with_spec_id(SpecId::SHANGHAI)
            .modify_cfg_env(|cfg| {
                cfg.disable_eip3607 = true;
            })
            .append_handler_register(|handler| {
                // Load standard precompiles.
                //
                // For Shanghai spec these include the following: ecrecover,
                // sha256, ripemd160, identity, bn128::{add,mul,pair}, blake2,
                // modexp.
                let precompiles = handler.pre_execution.load_precompiles();

                handler.pre_execution.load_precompiles = Arc::new(move || {
                    // Start with standard precompiles.
                    let mut precompiles = precompiles.clone();

                    // Extend with Oasis-specific precompiles.
                    precompiles.extend(precompile::new());

                    // Extend with module-specific precompiles (if any).
                    if let Some(additional_precompiles) = Cfg::additional_precompiles() {
                        precompiles.extend(additional_precompiles);
                    }

                    precompiles
                });
            })
            .modify_tx_env(f)
            .build();

        // Run the transaction.
        let tx_result = if estimate_gas {
            match evm.transact() {
                Ok(result) => result.result,
                Err(err) => return Err(crate::Error::ExecutionFailed(format!("{:?}", err))),
            }
        } else {
            match evm.transact_commit() {
                Ok(result) => result,
                Err(err) => return Err(crate::Error::ExecutionFailed(format!("{:?}", err))),
            }
        };

        match tx_result {
            ExecutionResult::Success {
                reason: _,
                gas_used,
                gas_refunded: _,
                logs,
                output,
            } => {
                let data = match output {
                    Output::Call(out) => out,
                    Output::Create(_, Some(addr)) => addr.into_array().into(),
                    Output::Create(_, None) => Bytes::new(),
                };

                // Clamp data based on maximum allowed result size.
                let data = if !is_query && data.len() > Cfg::MAX_RESULT_SIZE {
                    data[..Cfg::MAX_RESULT_SIZE].to_vec()
                } else {
                    data.to_vec()
                };

                // Use gas and refund unused gas.
                // XXX: check
                <C::Runtime as Runtime>::Core::use_tx_gas(gas_used)?;
                <C::Runtime as Runtime>::Accounts::set_refund_unused_tx_fee(Cfg::REFUND_UNUSED_FEE);

                // Emit logs as events.
                CurrentState::with(|state| {
                    for log in logs {
                        state.emit_event(crate::Event::Log {
                            address: H160::from_slice(&log.address.into_array()),
                            topics: log
                                .topics()
                                .iter()
                                .map(|&topic| H256::from_slice(topic.as_slice()))
                                .collect(),
                            data: log.data.data.to_vec(),
                        });
                    }
                });

                Ok(data)
            }
            ExecutionResult::Revert { gas_used, output } => {
                // Clamp data based on maximum allowed result size.
                // XXX: to_vec maybe not needed (check encoding)
                let data = if !is_query && output.len() > Cfg::MAX_RESULT_SIZE {
                    output[..Cfg::MAX_RESULT_SIZE].to_vec()
                } else {
                    output.to_vec()
                };

                // Use gas and refund unused gas.
                // XXX: check
                <C::Runtime as Runtime>::Core::use_tx_gas(gas_used)?;
                <C::Runtime as Runtime>::Accounts::set_refund_unused_tx_fee(Cfg::REFUND_UNUSED_FEE);

                Err(Error::Reverted(BASE64_STANDARD.encode(data)))
            }
            ExecutionResult::Halt { reason, gas_used } => {
                // Use gas and refund unused gas.
                // XXX: check
                <C::Runtime as Runtime>::Core::use_tx_gas(gas_used)?;
                <C::Runtime as Runtime>::Accounts::set_refund_unused_tx_fee(Cfg::REFUND_UNUSED_FEE);

                Err(crate::Error::ExecutionFailed(format!("{:?}", reason)))
            }
        }
    }

    fn evm_create<C: Context>(
        ctx: &C,
        caller: H160,
        value: U256,
        init_code: Vec<u8>,
        estimate_gas: bool,
    ) -> Result<Vec<u8>, Error> {
        Self::evm_execute(ctx, estimate_gas, |tx| {
            tx.gas_limit = <C::Runtime as Runtime>::Core::remaining_tx_gas();
            tx.gas_price = CurrentState::with_env(|env| env.tx_auth_info().fee.gas_price())
                .try_into()
                .unwrap(); // XXX: err checking

            tx.caller = caller.0.into();
            tx.transact_to = TxKind::Create;
            tx.value = revm::primitives::U256::from_be_bytes(value.into());
            tx.data = init_code.into();
        })
    }

    fn evm_call<C: Context>(
        ctx: &C,
        caller: H160,
        address: H160,
        value: U256,
        data: Vec<u8>,
        estimate_gas: bool,
    ) -> Result<Vec<u8>, Error> {
        Self::evm_execute(ctx, estimate_gas, |tx| {
            tx.gas_limit = <C::Runtime as Runtime>::Core::remaining_tx_gas();
            tx.gas_price = CurrentState::with_env(|env| env.tx_auth_info().fee.gas_price())
                .try_into()
                .unwrap(); // XXX: err checking

            tx.caller = caller.0.into();
            tx.transact_to = TxKind::Call(address.0.into());
            tx.value = revm::primitives::U256::from_be_bytes(value.into());
            tx.data = data.into();
        })
    }

    fn derive_caller() -> Result<H160, Error> {
        CurrentState::with_env(|env| derive_caller::from_tx_auth_info(env.tx_auth_info()))
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
    const VERSION: u32 = 3;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    fn init(genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(genesis.parameters);
    }

    #[migration(from = 2)]
    fn migrate_v2_to_v3() {
        // TODO?
    }

    #[handler(call = "evm.Create")]
    fn tx_create<C: Context>(ctx: &C, body: types::Create) -> Result<Vec<u8>, Error> {
        Self::create(ctx, body.value, body.init_code)
    }

    #[handler(call = "evm.Call")]
    fn tx_call<C: Context>(ctx: &C, body: types::Call) -> Result<Vec<u8>, Error> {
        Self::call(ctx, body.address, body.value, body.data)
    }

    #[handler(query = "evm.Storage")]
    fn query_storage<C: Context>(ctx: &C, body: types::StorageQuery) -> Result<Vec<u8>, Error> {
        Self::get_storage(ctx, body.address, body.index)
    }

    #[handler(query = "evm.Code")]
    fn query_code<C: Context>(ctx: &C, body: types::CodeQuery) -> Result<Vec<u8>, Error> {
        Self::get_code(ctx, body.address)
    }

    #[handler(query = "evm.Balance")]
    fn query_balance<C: Context>(ctx: &C, body: types::BalanceQuery) -> Result<u128, Error> {
        Self::get_balance(ctx, body.address)
    }

    #[handler(query = "evm.SimulateCall", expensive, allow_private_km)]
    fn query_simulate_call<C: Context>(
        ctx: &C,
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
        _ctx: &C,
        scheme: &str,
        body: &[u8],
    ) -> Result<Option<Transaction>, CoreError> {
        match scheme {
            "evm.ethereum.v0" => {
                let min_gas_price =
                    <C::Runtime as Runtime>::Core::min_gas_price(&Cfg::TOKEN_DENOMINATION)
                        .unwrap_or_default();

                Ok(Some(
                    raw_tx::decode(
                        body,
                        Some(Cfg::CHAIN_ID),
                        min_gas_price,
                        &Cfg::TOKEN_DENOMINATION,
                    )
                    .map_err(CoreError::MalformedTransaction)?,
                ))
            }
            _ => Ok(None),
        }
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn end_block<C: Context>(ctx: &C) {
        CurrentState::with_store(|store| {
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
