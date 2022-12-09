//! Smart contracts module.
#![deny(rust_2018_idioms)]
#![forbid(unsafe_code)]
#![cfg_attr(all(feature = "benchmarks", test), feature(test))]

#[cfg(test)]
extern crate alloc;

use std::{convert::TryInto, io::Read};

use thiserror::Error;

use crate::store::get_instance_raw_store;
use oasis_contract_sdk_types::storage::StoreKind;
use oasis_runtime_sdk::{
    self as sdk,
    context::{Context, TxContext},
    core::common::crypto::hash::Hash,
    handler, module,
    module::Module as _,
    modules,
    modules::{accounts::API as _, core::API as _},
    runtime::Runtime,
    sdk_derive, storage,
    storage::Store,
    types::transaction::CallFormat,
};

mod abi;
mod code;
mod results;
mod store;
#[cfg(test)]
mod test;
pub mod types;
mod wasm;

/// Unique module name.
const MODULE_NAME: &str = "contracts";

/// Errors emitted by the contracts module.
#[derive(Error, Debug, sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("code too large (size: {0} max: {1})")]
    #[sdk_error(code = 2)]
    CodeTooLarge(u32, u32),

    #[error("code is malformed")]
    #[sdk_error(code = 3)]
    CodeMalformed,

    #[error("specified ABI is not supported")]
    #[sdk_error(code = 4)]
    UnsupportedABI,

    #[error("code is missing required ABI export: {0}")]
    #[sdk_error(code = 5)]
    CodeMissingRequiredExport(String),

    #[error("code declares reserved ABI export: {0}")]
    #[sdk_error(code = 6)]
    CodeDeclaresReservedExport(String),

    #[error("code declares start function")]
    #[sdk_error(code = 7)]
    CodeDeclaresStartFunction,

    #[error("code declares too many memories")]
    #[sdk_error(code = 8)]
    CodeDeclaresTooManyMemories,

    #[error("code {0} not found")]
    #[sdk_error(code = 9)]
    CodeNotFound(u64),

    #[error("instance {0} not found")]
    #[sdk_error(code = 10)]
    InstanceNotFound(u64),

    #[error("module loading failed")]
    #[sdk_error(code = 11)]
    ModuleLoadingFailed,

    #[error("execution failed: {0}")]
    #[sdk_error(code = 12)]
    ExecutionFailed(#[source] anyhow::Error),

    #[error("forbidden by policy")]
    #[sdk_error(code = 13)]
    Forbidden,

    #[error("function not supported")]
    #[sdk_error(code = 14)]
    Unsupported,

    #[error("insufficient balance in caller account")]
    #[sdk_error(code = 15)]
    InsufficientCallerBalance,

    #[error("call depth exceeded (depth: {0} max: {1})")]
    #[sdk_error(code = 16)]
    CallDepthExceeded(u16, u16),

    #[error("result size exceeded (size: {0} max: {1})")]
    #[sdk_error(code = 17)]
    ResultTooLarge(u32, u32),

    #[error("too many subcalls (count: {0} max: {1})")]
    #[sdk_error(code = 18)]
    TooManySubcalls(u16, u16),

    #[error("instance is already using code {0}")]
    #[sdk_error(code = 19)]
    CodeAlreadyUpgraded(u64),

    #[error("abort: {0}")]
    #[sdk_error(code = 20, abort)]
    Abort(#[from] sdk::dispatcher::Error),

    #[error("storage: key too large (size: {0} max: {1})")]
    #[sdk_error(code = 21)]
    StorageKeyTooLarge(u32, u32),

    #[error("storage: value too large (size: {0} max: {1})")]
    #[sdk_error(code = 22)]
    StorageValueTooLarge(u32, u32),

    #[error("crypto: msg too large (size: {0} max: {1})")]
    #[sdk_error(code = 23)]
    CryptoMsgTooLarge(u32, u32),

    #[error("crypto: malformed public key")]
    #[sdk_error(code = 24)]
    CryptoMalformedPublicKey,

    #[error("code declares multiple sub-versions")]
    #[sdk_error(code = 25)]
    CodeDeclaresMultipleSubVersions,

    #[error("crypto: malformed private key")]
    #[sdk_error(code = 26)]
    CryptoMalformedPrivateKey,

    #[error("crypto: malformed encryption key")]
    #[sdk_error(code = 27)]
    CryptoMalformedKey,

    #[error("crypto: malformed nonce")]
    #[sdk_error(code = 28)]
    CryptoMalformedNonce,

    #[error("crypto: key derivation function failure")]
    #[sdk_error(code = 29)]
    CryptoKeyDerivationFunctionFailure,

    #[error("module uses floating point data or operations")]
    #[sdk_error(code = 30)]
    ModuleUsesFloatingPoint,

    #[error("code declares too many functions")]
    #[sdk_error(code = 31)]
    CodeDeclaresTooManyFunctions,

    #[error("code declares too many locals")]
    #[sdk_error(code = 32)]
    CodeDeclaresTooManyLocals,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),

    #[error("contract error: {0}")]
    #[sdk_error(transparent)]
    Contract(#[from] wasm::ContractError),
}

/// Events emitted by the contracts module.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {}

/// Gas costs.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_upload: u64,
    pub tx_upload_per_byte: u64,
    pub tx_instantiate: u64,
    pub tx_call: u64,
    pub tx_upgrade: u64,
    pub tx_change_upgrade_policy: u64,

    // Subcalls.
    pub subcall_dispatch: u64,

    // Storage operations.
    pub wasm_public_storage_get_base: u64,
    pub wasm_public_storage_insert_base: u64,
    pub wasm_public_storage_remove_base: u64,
    pub wasm_public_storage_key_byte: u64,
    pub wasm_public_storage_value_byte: u64,
    pub wasm_confidential_storage_get_base: u64,
    pub wasm_confidential_storage_insert_base: u64,
    pub wasm_confidential_storage_remove_base: u64,
    pub wasm_confidential_storage_key_byte: u64,
    pub wasm_confidential_storage_value_byte: u64,
    pub wasm_env_query_base: u64,

    // Crypto operations.
    pub wasm_crypto_ecdsa_recover: u64,
    pub wasm_crypto_signature_verify_ed25519: u64,
    pub wasm_crypto_signature_verify_secp256k1: u64,
    pub wasm_crypto_signature_verify_sr25519: u64,
    pub wasm_crypto_x25519_derive_symmetric: u64,
    pub wasm_crypto_deoxysii_base: u64,
    pub wasm_crypto_deoxysii_byte: u64,
    pub wasm_crypto_random_bytes_base: u64,
    pub wasm_crypto_random_bytes_byte: u64,
}

impl Default for GasCosts {
    fn default() -> Self {
        // The below assume a batch gas limit of 1_000_000_000 ~ 1s.
        GasCosts {
            tx_upload: 30_000_000,
            tx_upload_per_byte: 400,
            tx_instantiate: 100_000,
            tx_call: 50_000,
            tx_upgrade: 50_000,
            tx_change_upgrade_policy: 30_000,

            subcall_dispatch: 1_000,

            wasm_public_storage_get_base: 5_000,
            wasm_public_storage_insert_base: 8_400,
            wasm_public_storage_remove_base: 6_400,
            wasm_public_storage_key_byte: 3_000,
            wasm_public_storage_value_byte: 300,
            wasm_confidential_storage_get_base: 10_000,
            wasm_confidential_storage_insert_base: 16_800,
            wasm_confidential_storage_remove_base: 12_800,
            wasm_confidential_storage_key_byte: 3_500,
            wasm_confidential_storage_value_byte: 400,
            wasm_env_query_base: 100,

            wasm_crypto_ecdsa_recover: 500_000,
            wasm_crypto_signature_verify_ed25519: 500_000,
            wasm_crypto_signature_verify_secp256k1: 500_000,
            wasm_crypto_signature_verify_sr25519: 500_000,
            wasm_crypto_x25519_derive_symmetric: 250_000,
            wasm_crypto_deoxysii_base: 1_000,
            wasm_crypto_deoxysii_byte: 3,
            wasm_crypto_random_bytes_base: 1_000,
            wasm_crypto_random_bytes_byte: 3,
        }
    }
}

/// Parameters for the contracts module.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub max_code_size: u32,
    pub max_stack_size: u32,
    pub max_memory_pages: u32,

    pub max_wasm_functions: u32,
    pub max_wasm_locals: u32,

    pub max_subcall_depth: u16,
    pub max_subcall_count: u16,

    pub max_result_size_bytes: u32,
    pub max_query_size_bytes: u32,
    pub max_storage_key_size_bytes: u32,
    pub max_storage_value_size_bytes: u32,
    pub max_crypto_signature_verify_message_size_bytes: u32,

    pub gas_costs: GasCosts,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            max_code_size: 1024 * 1024, // 1 MiB
            max_stack_size: 60 * 1024,  // 60 KiB
            max_memory_pages: 160,      // 10 MiB

            max_wasm_functions: 10_000,
            max_wasm_locals: 256_000,

            max_subcall_depth: 8,
            max_subcall_count: 16,

            max_result_size_bytes: 1024, // 1 KiB
            max_query_size_bytes: 1024,  // 1 KiB
            max_storage_key_size_bytes: 64,
            max_storage_value_size_bytes: 16 * 1024, // 16 KiB
            max_crypto_signature_verify_message_size_bytes: 16 * 1024, // 16KiB

            gas_costs: Default::default(),
        }
    }
}

impl module::Parameters for Parameters {
    type Error = std::convert::Infallible;
}

/// Genesis state for the contracts module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// Local configuration that can be provided by the node operator.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct LocalConfig {
    /// Gas limit for custom queries that invoke smart contracts.
    #[cbor(optional)]
    pub query_custom_max_gas: u64,

    /// Maximum number of items per page in InstanceRawStorage query result.
    #[cbor(optional)]
    pub max_instance_raw_storage_query_items: u64,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            query_custom_max_gas: 10_000_000,
            max_instance_raw_storage_query_items: 100,
        }
    }
}

/// State schema constants.
pub mod state {
    /// Next code identifier (u64).
    pub const NEXT_CODE_IDENTIFIER: &[u8] = &[0x01];
    /// Next instance identifier (u64).
    pub const NEXT_INSTANCE_IDENTIFIER: &[u8] = &[0x02];
    /// Information about uploaded code.
    pub const CODE_INFO: &[u8] = &[0x03];
    /// Information about the deployed contract instance.
    pub const INSTANCE_INFO: &[u8] = &[0x04];
    /// Per-instance key/value store.
    pub const INSTANCE_STATE: &[u8] = &[0x05];

    /// Uploaded code.
    pub const CODE: &[u8] = &[0xFF];
}

/// Module configuration.
pub trait Config: 'static {
    /// Module that is used for accessing accounts.
    type Accounts: modules::accounts::API;
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

impl<Cfg: Config> Module<Cfg> {
    /// Loads code information for the specified code identifier.
    fn load_code_info<C: Context>(
        ctx: &mut C,
        code_id: types::CodeId,
    ) -> Result<types::Code, Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let code_info_store =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::CODE_INFO));
        let code_info = code_info_store
            .get(code_id.to_storage_key())
            .ok_or_else(|| Error::CodeNotFound(code_id.as_u64()))?;

        Ok(code_info)
    }

    /// Stores specified code information.
    fn store_code_info<C: Context>(ctx: &mut C, code_info: types::Code) -> Result<(), Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut code_info_store =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::CODE_INFO));
        code_info_store.insert(code_info.id.to_storage_key(), code_info);

        Ok(())
    }

    /// Loads specified instance information.
    fn load_instance_info<C: Context>(
        ctx: &mut C,
        instance_id: types::InstanceId,
    ) -> Result<types::Instance, Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let instance_info_store =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::INSTANCE_INFO));
        let instance_info = instance_info_store
            .get(instance_id.to_storage_key())
            .ok_or_else(|| Error::InstanceNotFound(instance_id.as_u64()))?;

        Ok(instance_info)
    }

    /// Stores specified instance information.
    fn store_instance_info<C: Context>(
        ctx: &mut C,
        instance_info: types::Instance,
    ) -> Result<(), Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut instance_info_store =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::INSTANCE_INFO));
        instance_info_store.insert(instance_info.id.to_storage_key(), instance_info);

        Ok(())
    }
}

#[sdk_derive(MethodHandler)]
impl<Cfg: Config> Module<Cfg> {
    #[handler(call = "contracts.Upload")]
    pub fn tx_upload<C: TxContext>(
        ctx: &mut C,
        body: types::Upload,
    ) -> Result<types::UploadResult, Error> {
        let params = Self::params(ctx.runtime_state());
        let uploader = ctx.tx_caller_address();

        // Validate code size.
        let code_size: u32 = body
            .code
            .len()
            .try_into()
            .map_err(|_| Error::CodeTooLarge(u32::MAX, params.max_code_size))?;
        if code_size > params.max_code_size {
            return Err(Error::CodeTooLarge(code_size, params.max_code_size));
        }

        // Account for base gas.
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_upload)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(body.code.len() as u64),
        )?;

        // Decompress code.
        let mut code = Vec::with_capacity(body.code.len());
        let decoder = snap::read::FrameDecoder::new(body.code.as_slice());
        decoder
            .take(params.max_code_size.into())
            .read_to_end(&mut code)
            .map_err(|_| Error::CodeMalformed)?;

        // Account for extra gas needed after decompression.
        let plain_code_size: u32 = code.len().try_into().unwrap();
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(plain_code_size.saturating_sub(code_size) as u64),
        )?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(types::UploadResult::default());
        }

        // Validate and transform the code.
        let (code, abi_info) = wasm::validate_and_transform::<Cfg, C>(&code, body.abi, &params)?;
        let hash = Hash::digest_bytes(&code);

        // Validate code size again and account for any instrumentation. This is here to avoid any
        // incentives in generating code that gets maximally inflated after instrumentation.
        let inst_code_size: u32 = code
            .len()
            .try_into()
            .map_err(|_| Error::CodeTooLarge(u32::MAX, params.max_code_size))?;
        if inst_code_size > params.max_code_size {
            return Err(Error::CodeTooLarge(inst_code_size, params.max_code_size));
        }
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(inst_code_size.saturating_sub(plain_code_size) as u64),
        )?;

        // Assign next identifier.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut tstore = storage::TypedStore::new(&mut store);
        let id: types::CodeId = tstore.get(state::NEXT_CODE_IDENTIFIER).unwrap_or_default();
        tstore.insert(state::NEXT_CODE_IDENTIFIER, id.increment());

        // Store information about uploaded code.
        let code_info = types::Code {
            id,
            hash,
            abi: body.abi,
            abi_sv: abi_info.abi_sv,
            uploader,
            instantiate_policy: body.instantiate_policy,
        };
        Self::store_code(ctx, &code_info, &code)?;
        Self::store_code_info(ctx, code_info)?;

        Ok(types::UploadResult { id })
    }

    #[handler(call = "contracts.Instantiate")]
    pub fn tx_instantiate<C: TxContext>(
        ctx: &mut C,
        body: types::Instantiate,
    ) -> Result<types::InstantiateResult, Error> {
        let params = Self::params(ctx.runtime_state());
        let creator = ctx.tx_caller_address();

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_instantiate)?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(types::InstantiateResult::default());
        }

        // Load code information, enforce instantiation policy and load the code.
        let code_info = Self::load_code_info(ctx, body.code_id)?;
        code_info.instantiate_policy.enforce(ctx)?;
        let code = Self::load_code(ctx, &code_info)?;

        // Assign next identifier.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut tstore = storage::TypedStore::new(&mut store);
        let id: types::InstanceId = tstore
            .get(state::NEXT_INSTANCE_IDENTIFIER)
            .unwrap_or_default();
        tstore.insert(state::NEXT_INSTANCE_IDENTIFIER, id.increment());

        // Store instance information.
        let instance_info = types::Instance {
            id,
            code_id: body.code_id,
            creator,
            upgrades_policy: body.upgrades_policy,
        };
        Self::store_instance_info(ctx, instance_info.clone())?;

        // Transfer any attached tokens.
        for tokens in &body.tokens {
            Cfg::Accounts::transfer(ctx, creator, instance_info.address(), tokens)
                .map_err(|_| Error::InsufficientCallerBalance)?
        }
        // Run instantiation function.
        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx),
            ctx.tx_caller_address(),
            ctx.is_read_only(),
            ctx.tx_call_format(),
            ctx,
        );
        let result = wasm::instantiate::<Cfg, C>(&mut exec_ctx, &contract, &body);

        let result = results::process_execution_result(ctx, result)?;
        results::process_execution_success::<Cfg, C>(ctx, &params, &contract, result)?;
        Ok(types::InstantiateResult { id })
    }

    #[handler(call = "contracts.Call", allow_interactive)]
    pub fn tx_call<C: TxContext>(
        ctx: &mut C,
        body: types::Call,
    ) -> Result<types::CallResult, Error> {
        let params = Self::params(ctx.runtime_state());
        let caller = ctx.tx_caller_address();

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_call)?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(types::CallResult::default());
        }

        // Load instance information and code.
        let instance_info = Self::load_instance_info(ctx, body.id)?;
        let code_info = Self::load_code_info(ctx, instance_info.code_id)?;
        let code = Self::load_code(ctx, &code_info)?;

        // Transfer any attached tokens.
        for tokens in &body.tokens {
            Cfg::Accounts::transfer(ctx, caller, instance_info.address(), tokens)
                .map_err(|_| Error::InsufficientCallerBalance)?
        }
        // Run call function.
        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx),
            ctx.tx_caller_address(),
            ctx.is_read_only(),
            ctx.tx_call_format(),
            ctx,
        );
        let result = wasm::call::<Cfg, C>(&mut exec_ctx, &contract, &body);

        let result = results::process_execution_result(ctx, result)?;
        let data = results::process_execution_success::<Cfg, C>(ctx, &params, &contract, result)?;
        Ok(types::CallResult(data))
    }

    #[handler(call = "contracts.ChangeUpgradePolicy")]
    pub fn tx_change_upgrade_policy<C: TxContext>(
        ctx: &mut C,
        body: types::ChangeUpgradePolicy,
    ) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_change_upgrade_policy)?;

        if ctx.is_check_only() {
            return Ok(());
        }

        // Load instance information.
        let mut instance_info = Self::load_instance_info(ctx, body.id)?;
        instance_info.upgrades_policy.enforce(ctx)?;

        // Change upgrade policy.
        instance_info.upgrades_policy = body.upgrades_policy;
        Self::store_instance_info(ctx, instance_info.clone())?;

        Ok(())
    }

    #[handler(call = "contracts.Upgrade")]
    pub fn tx_upgrade<C: TxContext>(ctx: &mut C, body: types::Upgrade) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());
        let caller = ctx.tx_caller_address();

        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_upgrade)?;

        if !ctx.should_execute_contracts() {
            // Only fast checks are allowed.
            return Ok(());
        }

        // Load instance information and code.
        let mut instance_info = Self::load_instance_info(ctx, body.id)?;
        instance_info.upgrades_policy.enforce(ctx)?;
        if instance_info.code_id == body.code_id {
            return Err(Error::CodeAlreadyUpgraded(body.code_id.as_u64()));
        }
        let code_info = Self::load_code_info(ctx, instance_info.code_id)?;
        let code = Self::load_code(ctx, &code_info)?;

        // Transfer any attached tokens.
        for tokens in &body.tokens {
            Cfg::Accounts::transfer(ctx, caller, instance_info.address(), tokens)
                .map_err(|_| Error::InsufficientCallerBalance)?
        }
        // Run pre-upgrade function on the previous contract.
        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx),
            ctx.tx_caller_address(),
            ctx.is_read_only(),
            ctx.tx_call_format(),
            ctx,
        );
        // Pre-upgrade invocation must succeed for the upgrade to proceed.
        let result = wasm::pre_upgrade::<Cfg, C>(&mut exec_ctx, &contract, &body);

        results::process_execution_result(ctx, result)?;

        // Update the contract code.
        instance_info.code_id = body.code_id;
        let code_info = Self::load_code_info(ctx, instance_info.code_id)?;
        let code = Self::load_code(ctx, &code_info)?;
        Self::store_instance_info(ctx, instance_info.clone())?;

        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            <C::Runtime as Runtime>::Core::remaining_tx_gas(ctx),
            ctx.tx_caller_address(),
            ctx.is_read_only(),
            ctx.tx_call_format(),
            ctx,
        );

        // Run post-upgrade function on the new contract.
        let result = wasm::post_upgrade::<Cfg, C>(&mut exec_ctx, &contract, &body);
        results::process_execution_result(ctx, result)?;
        Ok(())
    }

    #[handler(query = "contracts.Code")]
    pub fn query_code<C: Context>(
        ctx: &mut C,
        args: types::CodeQuery,
    ) -> Result<types::Code, Error> {
        Self::load_code_info(ctx, args.id)
    }

    #[handler(query = "contracts.CodeStorage")]
    pub fn query_code_storage<C: Context>(
        ctx: &mut C,
        args: types::CodeStorageQuery,
    ) -> Result<types::CodeStorageQueryResult, Error> {
        let code_info = Self::load_code_info(ctx, args.id)?;
        let code = Self::load_code(ctx, &code_info)?;

        Ok(types::CodeStorageQueryResult { code })
    }

    #[handler(query = "contracts.Instance")]
    pub fn query_instance<C: Context>(
        ctx: &mut C,
        args: types::InstanceQuery,
    ) -> Result<types::Instance, Error> {
        Self::load_instance_info(ctx, args.id)
    }

    #[handler(query = "contracts.InstanceStorage")]
    pub fn query_instance_storage<C: Context>(
        ctx: &mut C,
        args: types::InstanceStorageQuery,
    ) -> Result<types::InstanceStorageQueryResult, Error> {
        let instance_info = Self::load_instance_info(ctx, args.id)?;
        // NOTE: We can only access the public store here.
        let store = store::for_instance(ctx, &instance_info, StoreKind::Public)?;

        Ok(types::InstanceStorageQueryResult {
            value: store.get(&args.key),
        })
    }

    #[handler(query = "contracts.InstanceRawStorage", expensive)]
    pub fn query_instance_raw_storage<C: Context>(
        ctx: &mut C,
        args: types::InstanceRawStorageQuery,
    ) -> Result<types::InstanceRawStorageQueryResult, Error> {
        let cfg: LocalConfig = ctx.local_config(MODULE_NAME).unwrap_or_default();
        let limit: usize = args
            .limit
            .unwrap_or(u64::MAX)
            .min(cfg.max_instance_raw_storage_query_items)
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        let offset: usize = args
            .offset
            .unwrap_or(0)
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;

        let instance_info = Self::load_instance_info(ctx, args.id)?;
        // Convert contracts API StoreKind to internal storage StoreKind.
        let sk: StoreKind = (args.store_kind as u32)
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        let store = get_instance_raw_store(ctx, &instance_info, sk);

        let items: Vec<(Vec<u8>, Vec<u8>)> = store
            .iter()
            // Shave off first 32 bytes of the key to get the contract instance-level key name.
            .filter(|(k, _)| k.len() >= 32)
            .map(|(k, v)| (k[32..].to_vec(), v.to_vec()))
            .skip(offset)
            .take(limit)
            .collect();

        Ok(types::InstanceRawStorageQueryResult { items })
    }

    #[handler(query = "contracts.PublicKey")]
    pub fn query_public_key<C: Context>(
        _ctx: &mut C,
        _args: types::PublicKeyQuery,
    ) -> Result<types::PublicKeyQueryResult, Error> {
        Err(Error::Unsupported)
    }

    #[handler(query = "contracts.Custom", expensive)]
    pub fn query_custom<C: Context>(
        ctx: &mut C,
        args: types::CustomQuery,
    ) -> Result<types::CustomQueryResult, Error> {
        let params = Self::params(ctx.runtime_state());

        // Load instance information and code.
        let instance_info = Self::load_instance_info(ctx, args.id)?;
        let code_info = Self::load_code_info(ctx, instance_info.code_id)?;
        let code = Self::load_code(ctx, &code_info)?;

        // Load local configuration.
        let cfg: LocalConfig = ctx.local_config(MODULE_NAME).unwrap_or_default();

        // Run query function.
        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            cfg.query_custom_max_gas,
            Default::default(), // No caller for queries.
            true,
            CallFormat::Plain,
            ctx,
        );
        let result = wasm::query::<Cfg, C>(&mut exec_ctx, &contract, &args).inner?; // No need to handle gas.

        Ok(types::CustomQueryResult(result.data))
    }
}

impl<Cfg: Config> module::Module for Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl<Cfg: Config> Module<Cfg> {
    /// Initialize state from genesis.
    pub fn init<C: Context>(ctx: &mut C, genesis: Genesis) {
        // Set genesis parameters.
        Self::set_params(ctx.runtime_state(), genesis.parameters);
    }

    /// Migrate state from a previous version.
    pub fn migrate<C: Context>(_ctx: &mut C, _from: u32) -> bool {
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

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {}
impl<Cfg: Config> module::BlockHandler for Module<Cfg> {}
impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
