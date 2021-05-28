use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_runtime_sdk::{
    self as sdk,
    context::{Context, TxContext},
    core::common::cbor,
    error::{Error as _, RuntimeError},
    module::Module as _,
    modules::{
        core,
        core::{Module as Core, API as _},
    },
    types::transaction::CallResult,
};

pub mod types;

/// The name of our module.
const MODULE_NAME: &str = "keyvalue";

/// Errors emitted by the keyvalue module.
#[derive(Error, Debug, sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] core::Error),
}

/// Events emitted by the keyvalue module.
#[derive(Debug, Serialize, Deserialize, sdk::Event)]
#[serde(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Insert { kv: types::KeyValue },

    #[sdk_event(code = 2)]
    Remove { key: types::Key },
}

/// Gas costs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasCosts {
    #[serde(rename = "insert_absent")]
    pub insert_absent: u64,
    #[serde(rename = "insert_existing")]
    pub insert_existing: u64,
    #[serde(rename = "remove_absent")]
    pub remove_absent: u64,
    #[serde(rename = "remove_existing")]
    pub remove_existing: u64,
}

/// Parameters for the keyvalue module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    #[serde(rename = "gas_costs")]
    pub gas_costs: GasCosts,
}

impl sdk::module::Parameters for Parameters {
    type Error = ();
}

/// Genesis state for the keyvalue module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,
}

/// Simple keyvalue runtime module.
pub struct Module;

impl sdk::module::Module for Module {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

impl sdk::module::AuthHandler for Module {}
impl sdk::module::BlockHandler for Module {}

impl sdk::module::MethodHandler for Module {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> sdk::module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "keyvalue.Insert" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_insert(ctx, args)?))
                }();
                match result {
                    Ok(value) => sdk::module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => sdk::module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "keyvalue.Remove" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_remove(ctx, args)?))
                }();
                match result {
                    Ok(value) => sdk::module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => sdk::module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            _ => sdk::module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &mut C,
        method: &str,
        args: cbor::Value,
    ) -> sdk::module::DispatchResult<cbor::Value, Result<cbor::Value, RuntimeError>> {
        match method {
            "keyvalue.Get" => sdk::module::DispatchResult::Handled((|| {
                let args = cbor::from_value(args).map_err(|_| Error::InvalidArgument)?;
                Ok(cbor::to_value(&Self::query_get(ctx, args)?))
            })()),
            _ => sdk::module::DispatchResult::Unhandled(args),
        }
    }
}

// Actual implementation of this runtime's externally-callable methods.
impl Module {
    /// Insert given keyvalue into storage.
    fn tx_insert<C: TxContext>(ctx: &mut C, body: types::KeyValue) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        let params = Self::params(ctx.runtime_state());

        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let ts = sdk::storage::TypedStore::new(&mut store);
        let cost = match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
            None => params.gas_costs.insert_absent,
            Some(_) => params.gas_costs.insert_existing,
        };
        // We must drop ts and store so that use_gas can borrow ctx.
        Core::use_tx_gas(ctx, cost)?;

        // Recreate store and ts after we get ctx back
        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut ts = sdk::storage::TypedStore::new(&mut store);
        let bc = body.clone();
        ts.insert(&body.key, &body.value);
        ctx.emit_event(Event::Insert { kv: bc });
        Ok(())
    }

    /// Remove keyvalue from storage using given key.
    fn tx_remove<C: TxContext>(ctx: &mut C, body: types::Key) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }

        let params = Self::params(ctx.runtime_state());

        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let ts = sdk::storage::TypedStore::new(&mut store);
        let cost = match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
            None => params.gas_costs.remove_absent,
            Some(_) => params.gas_costs.remove_existing,
        };
        // We must drop ts and store so that use_gas can borrow ctx.
        Core::use_tx_gas(ctx, cost)?;

        // Recreate store and ts after we get ctx back
        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut ts = sdk::storage::TypedStore::new(&mut store);
        let bc = body.clone();
        ts.remove(&body.key);
        ctx.emit_event(Event::Remove { key: bc });
        Ok(())
    }

    /// Fetch keyvalue from storage using given key.
    fn query_get<C: Context>(ctx: &mut C, body: types::Key) -> Result<types::KeyValue, Error> {
        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let ts = sdk::storage::TypedStore::new(&mut store);
        let v: Vec<u8> = ts.get(body.key.clone()).ok_or(Error::InvalidArgument)?;
        Ok(types::KeyValue {
            key: body.key,
            value: v,
        })
    }
}

impl sdk::module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut sdk::modules::core::types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            Self::set_params(ctx.runtime_state(), &genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}
