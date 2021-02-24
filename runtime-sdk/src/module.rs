//! Runtime modules.
use std::{collections::BTreeMap, fmt::Debug};

use impl_trait_for_tuples::impl_for_tuples;
use serde::{de::DeserializeOwned, Serialize};

use oasis_core_runtime::common::cbor;

use crate::{
    context::{DispatchContext, TxContext},
    error, event, modules, storage,
    storage::Store,
    types::transaction::{CallResult, Transaction},
};

/// Metadata of a callable method.
pub struct CallableMethodInfo {
    /// Method name.
    pub name: &'static str,

    /// Method handler function.
    pub handler: fn(&CallableMethodInfo, &mut TxContext, cbor::Value) -> CallResult,
}

/// Metadata of a query method.
pub struct QueryMethodInfo {
    /// Method name.
    pub name: &'static str,

    /// Method handler function.
    pub handler: fn(
        &QueryMethodInfo,
        &mut DispatchContext,
        cbor::Value,
    ) -> Result<cbor::Value, error::RuntimeError>,
}

/// Registry of methods exposed by the modules.
pub struct MethodRegistry {
    callable_methods: BTreeMap<&'static str, CallableMethodInfo>,
    query_methods: BTreeMap<&'static str, QueryMethodInfo>,
}

impl MethodRegistry {
    /// Create a new method registry.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            callable_methods: BTreeMap::new(),
            query_methods: BTreeMap::new(),
        }
    }

    /// Register a new callable method.
    ///
    /// # Panics
    ///
    /// This method will panic in case a method with the same name is already registered.
    pub fn register_callable(&mut self, mi: CallableMethodInfo) {
        if self.callable_methods.contains_key(mi.name) {
            panic!("callable method already exists: {}", mi.name);
        }
        self.callable_methods.insert(mi.name, mi);
    }

    /// Looks up a previously registered callable method.
    pub fn lookup_callable(&self, name: &str) -> Option<&CallableMethodInfo> {
        self.callable_methods.get(name)
    }

    /// Register a new query method.
    ///
    /// # Panics
    ///
    /// This method will panic in case a method with the same name is already registered.
    pub fn register_query(&mut self, mi: QueryMethodInfo) {
        if self.query_methods.contains_key(mi.name) {
            panic!("query method already exists: {}", mi.name);
        }
        self.query_methods.insert(mi.name, mi);
    }

    /// Looks up a previously registered callable method.
    pub fn lookup_query(&self, name: &str) -> Option<&QueryMethodInfo> {
        self.query_methods.get(name)
    }
}

/// Method registration handler.
#[impl_for_tuples(30)]
pub trait MethodRegistrationHandler {
    /// Register any methods exported by the module.
    fn register_methods(_methods: &mut MethodRegistry) {
        // Default implementation doesn't do anything.
    }
}

/// Authentication handler.
pub trait AuthHandler {
    /// Authenticate a transaction.
    ///
    /// Note that any signatures have already been verified.
    fn authenticate_tx(
        _ctx: &mut DispatchContext,
        _tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        // Default implementation doesn't do any checks.
        Ok(())
    }
}

#[impl_for_tuples(30)]
impl AuthHandler for Tuple {
    fn authenticate_tx(
        ctx: &mut DispatchContext,
        tx: &Transaction,
    ) -> Result<(), modules::core::Error> {
        for_tuples!( #( Tuple::authenticate_tx(ctx, tx)?; )* );
        Ok(())
    }
}

/// Migration handler.
pub trait MigrationHandler {
    /// Genesis state type.
    type Genesis;

    /// Initialize state from genesis or perform a migration.
    ///
    /// Should return true in case metadata has been changed.
    fn init_or_migrate(
        ctx: &mut DispatchContext,
        meta: &mut modules::core::types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool;
}

#[allow(clippy::type_complexity)]
#[impl_for_tuples(30)]
impl MigrationHandler for Tuple {
    for_tuples!( type Genesis = ( #( Tuple::Genesis ),* ); );

    fn init_or_migrate(
        ctx: &mut DispatchContext,
        meta: &mut modules::core::types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool {
        [for_tuples!( #( Tuple::init_or_migrate(ctx, meta, &genesis.Tuple) ),* )]
            .iter()
            .any(|x| *x)
    }
}

/// Block handler.
#[impl_for_tuples(30)]
pub trait BlockHandler {
    /// Perform any common actions at the start of the block (before any transactions have been
    /// executed).
    fn begin_block(_ctx: &mut DispatchContext) {
        // Default implementation doesn't do anything.
    }

    /// Perform any common actions at the end of the block (after all transactions have been
    /// executed).
    fn end_block(_ctx: &mut DispatchContext) {
        // Default implementation doesn't do anything.
    }
}

/// A runtime module.
pub trait Module {
    /// Module name.
    const NAME: &'static str;

    /// Module version.
    const VERSION: u32 = 1;

    /// Module error type.
    type Error: error::Error + 'static;

    /// Module event type.
    type Event: event::Event + 'static;

    /// Module parameters.
    type Parameters: Parameters + 'static;

    /// Return the module's parameters.
    fn params<S: Store>(store: S) -> Self::Parameters {
        let store = storage::PrefixStore::new(store, &Self::NAME);
        let store = storage::TypedStore::new(store);
        store.get(Self::Parameters::STORE_KEY).unwrap_or_default()
    }

    /// Set the module's parameters.
    fn set_params<S: Store>(store: S, params: &Self::Parameters) {
        let store = storage::PrefixStore::new(store, &Self::NAME);
        let mut store = storage::TypedStore::new(store);
        store.insert(Self::Parameters::STORE_KEY, params);
    }
}

/// Parameters for a runtime module.
pub trait Parameters: Debug + Default + Serialize + DeserializeOwned {
    type Error;

    /// Store key used for storing parameters.
    const STORE_KEY: &'static [u8] = &[0x00];

    /// Perform basic parameter validation.
    fn validate_basic(&self) -> Result<(), Self::Error> {
        // No validation by default.
        Ok(())
    }
}
