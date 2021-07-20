//! Smart contracts module.
#[cfg(test)]
extern crate alloc;

use thiserror::Error;

use oasis_runtime_sdk::{
    self as sdk,
    context::{Context, TxContext},
    core::common::crypto::hash::Hash,
    error::{self, Error as _},
    module,
    module::Module as _,
    modules,
    modules::core::{Module as Core, API as _},
    storage::{self, Store as _},
    types::transaction::CallResult,
};

mod abi;
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

    #[error("code too large")]
    #[sdk_error(code = 2)]
    CodeTooLarge,

    #[error("code is malformed")]
    #[sdk_error(code = 3)]
    CodeMalformed,

    #[error("specified ABI is not supported")]
    #[sdk_error(code = 4)]
    UnsupportedABI,

    #[error("code does not conform to ABI specification")]
    #[sdk_error(code = 5)]
    CodeNonConformant,

    #[error("code not found")]
    #[sdk_error(code = 6)]
    CodeNotFound,

    #[error("instance not found")]
    #[sdk_error(code = 7)]
    InstanceNotFound,

    #[error("module loading failed")]
    #[sdk_error(code = 8)]
    ModuleLoadingFailed,

    #[error("execution failed: {0}")]
    #[sdk_error(code = 9)]
    ExecutionFailed(#[source] anyhow::Error),

    #[error("forbidden by policy")]
    #[sdk_error(code = 10)]
    Forbidden,

    #[error("function not supported")]
    #[sdk_error(code = 11)]
    Unsupported,

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
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_upload: u64,
    pub tx_upload_per_byte: u64,
    pub tx_instantiate: u64,
    pub tx_call: u64,
    pub tx_upgrade: u64,

    pub wasm_op: u64,
    // TODO: Costs of storage operations.
    // TODO: Cost of emitted messages.
    // TODO: Cost of queries.
}

/// Parameters for the contracts module.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub max_code_size: u64,
    pub gas_costs: GasCosts,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            max_code_size: 256 * 1024,     // 256 KiB
            gas_costs: Default::default(), // TODO
        }
    }
}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Genesis state for the contracts module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

/// Interface that can be called from other modules.
pub trait API {
    // TODO: What makes sense?
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

pub struct Module;

impl Module {
    /// Loads code information for the specified code identifier.
    fn load_code_info<C: Context>(
        ctx: &mut C,
        code_id: types::CodeId,
    ) -> Result<types::Code, Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let code_info_store =
            storage::TypedStore::new(storage::PrefixStore::new(&mut store, &state::CODE_INFO));
        let code_info: types::Code = code_info_store
            .get(code_id.to_storage_key())
            .ok_or(Error::CodeNotFound)?;

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

    /// Loads code with the specified code identifier.
    fn load_code<C: Context>(ctx: &mut C, code_id: types::CodeId) -> Result<Vec<u8>, Error> {
        // TODO: Spport local untrusted cache to avoid storage queries.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let code_store = storage::PrefixStore::new(&mut store, &state::CODE);
        let code = code_store
            .get(code_id.to_storage_key())
            .ok_or(Error::CodeNotFound)?;

        Ok(code)
    }

    /// Stores code with the specified code identifier.
    fn store_code<C: Context>(
        ctx: &mut C,
        code_id: types::CodeId,
        code: &[u8],
    ) -> Result<(), Error> {
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut code_store = storage::PrefixStore::new(&mut store, &state::CODE);
        code_store.insert(code_id.to_storage_key(), &code);

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
            .ok_or(Error::InstanceNotFound)?;

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

impl Module {
    fn tx_upload<C: TxContext>(
        ctx: &mut C,
        body: types::Upload,
    ) -> Result<types::UploadResult, Error> {
        let params = Self::params(ctx.runtime_state());

        Core::use_tx_gas(ctx, params.gas_costs.tx_upload)?;
        Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(body.code.len() as u64),
        )?;

        // Validate code size.
        if body.code.len() as u64 > params.max_code_size {
            return Err(Error::CodeTooLarge);
        }

        // Validate and compile the code.
        let code = wasm::validate_and_compile(ctx, &body.code, body.abi)?;
        let hash = Hash::digest_bytes(&code);

        // Assign next identifier.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut tstore = storage::TypedStore::new(&mut store);
        let id: types::CodeId = tstore.get(state::NEXT_CODE_IDENTIFIER).unwrap_or_default();
        tstore.insert(state::NEXT_CODE_IDENTIFIER, id.increment());

        // Store information about uploaded code.
        Self::store_code_info(
            ctx,
            types::Code {
                id,
                hash,
                abi: body.abi,
                instantiate_policy: body.instantiate_policy,
            },
        )?;
        Self::store_code(ctx, id, &code)?;

        Ok(types::UploadResult { id })
    }

    fn tx_instantiate<C: TxContext>(
        ctx: &mut C,
        body: types::Instantiate,
    ) -> Result<types::InstantiateResult, Error> {
        let params = Self::params(ctx.runtime_state());
        let creator = ctx.tx_caller_address();

        Core::use_tx_gas(ctx, params.gas_costs.tx_instantiate)?;

        // Load code information, enforce instantiation policy and load the code.
        let code_info = Self::load_code_info(ctx, body.code_id)?;
        code_info.instantiate_policy.enforce(ctx)?;
        let code = Self::load_code(ctx, body.code_id)?;

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
            calls_policy: body.calls_policy,
            upgrades_policy: body.upgrades_policy,
        };
        Self::store_instance_info(ctx, instance_info.clone())?;

        // Run instantiation function.
        // TODO: Gas limit.
        wasm::instantiate(ctx, &body, &code_info, &instance_info, &code)?;

        Ok(types::InstantiateResult { id })
    }

    fn tx_call<C: TxContext>(ctx: &mut C, body: types::Call) -> Result<types::CallResult, Error> {
        let params = Self::params(ctx.runtime_state());

        Core::use_tx_gas(ctx, params.gas_costs.tx_call)?;

        // Load instance information, enforce call policy and load the code.
        let instance_info = Self::load_instance_info(ctx, body.id)?;
        instance_info.calls_policy.enforce(ctx)?;
        let code_info = Self::load_code_info(ctx, instance_info.code_id)?;
        let code = Self::load_code(ctx, instance_info.code_id)?;

        // Run call function.
        // TODO: Gas limit.
        let result = wasm::call(ctx, &body, &code_info, &instance_info, &code)?;
        // TODO: Events, messages.

        Ok(types::CallResult(result.data))
    }

    fn tx_upgrade<C: TxContext>(ctx: &mut C, _body: types::Upgrade) -> Result<(), Error> {
        let params = Self::params(ctx.runtime_state());

        Core::use_tx_gas(ctx, params.gas_costs.tx_upgrade)?;

        Err(Error::Unsupported)
    }

    fn query_code<C: Context>(_ctx: &mut C, _args: types::CodeQuery) -> Result<types::Code, Error> {
        Err(Error::Unsupported)
    }

    fn query_instance<C: Context>(
        _ctx: &mut C,
        _args: types::InstanceQuery,
    ) -> Result<types::Instance, Error> {
        Err(Error::Unsupported)
    }

    fn query_instance_storage<C: Context>(
        _ctx: &mut C,
        _args: types::InstanceStorageQuery,
    ) -> Result<types::InstanceStorageQueryResult, Error> {
        Err(Error::Unsupported)
    }

    fn query_public_key<C: Context>(
        _ctx: &mut C,
        _args: types::PublicKeyQuery,
    ) -> Result<types::PublicKeyQueryResult, Error> {
        Err(Error::Unsupported)
    }

    fn query_custom<C: Context>(
        _ctx: &mut C,
        _args: types::CustomQuery,
    ) -> Result<types::CustomQueryResult, Error> {
        Err(Error::Unsupported)
    }
}

impl module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl module::MethodHandler for Module {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "contracts.Upload" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_upload(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "contracts.Instantiate" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_instantiate(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "contracts.Call" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_call(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "contracts.Upgrade" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(Self::tx_upgrade(ctx, args)?))
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
            "contracts.Code" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::query_code(ctx, args)?))
            })()),
            "contracts.Instance" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::query_instance(ctx, args)?))
            })()),
            "contracts.InstanceStorage" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::query_instance_storage(ctx, args)?))
            })()),
            "contracts.PublicKey" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::query_public_key(ctx, args)?))
            })()),
            "contracts.Custom" => module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(Self::query_custom(ctx, args)?))
            })()),
            _ => module::DispatchResult::Unhandled(args),
        }
    }
}

impl Module {
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
            Self::init(ctx, genesis);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Perform migration.
        Self::migrate(ctx, version)
    }
}

impl module::AuthHandler for Module {}
impl module::BlockHandler for Module {}
impl module::InvariantHandler for Module {}
