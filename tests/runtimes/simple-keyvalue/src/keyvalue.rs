use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_runtime_sdk::{
    self as sdk,
    context::{DispatchContext, TxContext},
    core::common::cbor,
    error::{Error as _, RuntimeError},
    module::{CallableMethodInfo, Module as _, QueryMethodInfo},
    types::transaction::CallResult,
};

pub mod types;

/// The name of our module.
const MODULE_NAME: &str = "keyvalue";

/// Errors emitted by the keyvalue module.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,
}

impl sdk::error::Error for Error {
    fn module(&self) -> &str {
        MODULE_NAME
    }

    fn code(&self) -> u32 {
        match self {
            Error::InvalidArgument => 1,
        }
    }
}

impl From<Error> for sdk::error::RuntimeError {
    fn from(err: Error) -> sdk::error::RuntimeError {
        sdk::error::RuntimeError::new(err.module(), err.code(), &err.msg())
    }
}

/// Events emitted by the keyvalue module (none so far).
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    // XXX: What if we don't want to have any events?  The SDK seems to require an implementation anyways :/
    DummyEvent = 1,
}

impl sdk::event::Event for Event {
    fn module(&self) -> &str {
        MODULE_NAME
    }

    fn code(&self) -> u32 {
        match self {
            Event::DummyEvent { .. } => 1,
        }
    }

    fn value(&self) -> cbor::Value {
        cbor::to_value(self)
    }
}

/// Parameters for the keyvalue module (none so far).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters;

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

impl sdk::module::MethodRegistrationHandler for Module {
    /// Register all supported methods.
    fn register_methods(methods: &mut sdk::module::MethodRegistry) {
        methods.register_callable(sdk::module::CallableMethodInfo {
            name: "keyvalue.Insert",
            handler: Self::_callable_insert_handler,
        });
        methods.register_callable(sdk::module::CallableMethodInfo {
            name: "keyvalue.Remove",
            handler: Self::_callable_remove_handler,
        });
        methods.register_query(sdk::module::QueryMethodInfo {
            name: "keyvalue.Get",
            handler: Self::_query_get_handler,
        });
    }
}

// Boilerplate.
impl Module {
    fn _callable_insert_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext,
        body: cbor::Value,
    ) -> CallResult {
        let result = || -> Result<cbor::Value, Error> {
            let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
            Ok(cbor::to_value(&Self::insert(ctx, args)?))
        }();
        match result {
            Ok(value) => CallResult::Ok(value),
            Err(err) => err.to_call_result(),
        }
    }

    fn _callable_remove_handler(
        _mi: &CallableMethodInfo,
        ctx: &mut TxContext,
        body: cbor::Value,
    ) -> CallResult {
        let result = || -> Result<cbor::Value, Error> {
            let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
            Ok(cbor::to_value(&Self::remove(ctx, args)?))
        }();
        match result {
            Ok(value) => CallResult::Ok(value),
            Err(err) => err.to_call_result(),
        }
    }

    fn _query_get_handler(
        _mi: &QueryMethodInfo,
        ctx: &mut DispatchContext,
        body: cbor::Value,
    ) -> Result<cbor::Value, RuntimeError> {
        let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
        Ok(cbor::to_value(&Self::get(ctx, args)?))
    }
}

// Actual implementation of this runtime's externally-callable methods.
impl Module {
    // Insert given keyvalue into storage.
    fn insert(ctx: &mut TxContext, body: types::KeyValue) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }
        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut ts = sdk::storage::TypedStore::new(&mut store);
        ts.insert(body.key, &body.value);
        Ok(())
    }

    // Remove keyvalue from storage using given key.
    fn remove(ctx: &mut TxContext, body: types::Key) -> Result<(), Error> {
        if ctx.is_check_only() {
            return Ok(());
        }
        let mut store = sdk::storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut ts = sdk::storage::TypedStore::new(&mut store);
        ts.remove(body.key);
        Ok(())
    }

    // Fetch keyvalue from storage using given key.
    fn get(ctx: &mut DispatchContext, body: types::Key) -> Result<types::KeyValue, Error> {
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

    fn init_or_migrate(
        ctx: &mut DispatchContext,
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
