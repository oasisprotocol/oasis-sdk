use std::cmp::max;

use thiserror::Error;

use anyhow::{self, Context as _};

use oasis_runtime_sdk::{
    self as sdk,
    context::Context,
    core::common::crypto::hash::Hash,
    error::RuntimeError,
    keymanager::{get_key_pair_id, KeyPair, KeyPairId},
    module::{CallResult, Module as _},
    modules::{
        core,
        core::{Error as CoreError, API as _},
    },
    runtime::Runtime,
    state::CurrentState,
    storage::{ConfidentialStore, PrefixStore, Store, TypedStore},
    types::{address, transaction},
};

pub mod types;

/// The name of our module.
const MODULE_NAME: &str = "keyvalue";

/// The signature context used in the special greeting encoding scheme signature.
const SPECIAL_GREETING_SIGNATURE_CONTEXT: &[u8] =
    "oasis-runtime-sdk-test/simplekv-special-greeting: v0".as_bytes();

/// Errors emitted by the keyvalue module.
#[derive(Error, Debug, sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] core::Error),

    #[error("{0}")]
    #[sdk_error(transparent, abort)]
    Abort(#[source] sdk::dispatcher::Error),

    #[error("{0}")]
    #[sdk_error(code = 2)]
    Other(#[source] anyhow::Error),
}

/// Events emitted by the keyvalue module.
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {
    #[sdk_event(code = 1)]
    Insert { kv: types::KeyValue },

    #[sdk_event(code = 2)]
    Remove { key: types::Key },
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub insert_absent: u64,
    pub insert_existing: u64,
    pub remove_absent: u64,
    pub remove_existing: u64,
    pub confidential_insert_absent: u64,
    pub confidential_insert_existing: u64,
    pub confidential_remove_absent: u64,
    pub confidential_remove_existing: u64,
}

/// Parameters for the keyvalue module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub gas_costs: GasCosts,
}

impl sdk::module::Parameters for Parameters {
    type Error = ();
}

/// Genesis state for the keyvalue module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
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

impl sdk::module::TransactionHandler for Module {
    fn decode_tx<C: Context>(
        _ctx: &C,
        scheme: &str,
        body: &[u8],
    ) -> Result<Option<transaction::Transaction>, CoreError> {
        match scheme {
            "keyvalue.special-greeting.v0" => {
                let special_greeting: types::SpecialGreeting = cbor::from_slice(body)
                    .with_context(|| "decoding special greeting")
                    .map_err(CoreError::MalformedTransaction)?;
                special_greeting
                    .from
                    .verify(
                        SPECIAL_GREETING_SIGNATURE_CONTEXT,
                        &special_greeting.params_cbor,
                        &special_greeting.signature,
                    )
                    .with_context(|| "verifying special greeting signature")
                    .map_err(CoreError::MalformedTransaction)?;
                let params: types::SpecialGreetingParams =
                    cbor::from_slice(&special_greeting.params_cbor)
                        .with_context(|| "decoding special greeting parameters")
                        .map_err(CoreError::MalformedTransaction)?;
                Ok(Some(transaction::Transaction {
                    version: transaction::LATEST_TRANSACTION_VERSION,
                    call: transaction::Call {
                        format: transaction::CallFormat::Plain,
                        method: "keyvalue.Insert".to_string(),
                        body: cbor::to_value(types::KeyValue {
                            key: "greeting".as_bytes().to_owned(),
                            value: params.greeting.into_bytes(),
                        }),
                        ..Default::default()
                    },
                    auth_info: transaction::AuthInfo {
                        signer_info: vec![transaction::SignerInfo {
                            address_spec: transaction::AddressSpec::Signature(
                                address::SignatureAddressSpec::Ed25519(special_greeting.from),
                            ),
                            nonce: params.nonce,
                        }],
                        fee: transaction::Fee {
                            gas: 500,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                }))
                // After we decode this, the accounts module will check the nonce.
            }
            _ => Ok(None),
        }
    }
}

impl sdk::module::BlockHandler for Module {}
impl sdk::module::InvariantHandler for Module {}

impl sdk::module::MethodHandler for Module {
    fn dispatch_call<C: Context>(
        ctx: &C,
        method: &str,
        body: cbor::Value,
    ) -> sdk::module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "keyvalue.Insert" => sdk::module::dispatch_call(ctx, body, Self::tx_insert),
            "keyvalue.Remove" => sdk::module::dispatch_call(ctx, body, Self::tx_remove),
            "keyvalue.GetCreateKey" => sdk::module::dispatch_call(ctx, body, Self::tx_getcreatekey),
            "keyvalue.ConfidentialGet" => {
                sdk::module::dispatch_call(ctx, body, Self::tx_confidential_get)
            }
            "keyvalue.ConfidentialInsert" => {
                sdk::module::dispatch_call(ctx, body, Self::tx_confidential_insert)
            }
            "keyvalue.ConfidentialRemove" => {
                sdk::module::dispatch_call(ctx, body, Self::tx_confidential_remove)
            }
            _ => sdk::module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_query<C: Context>(
        ctx: &C,
        method: &str,
        args: cbor::Value,
    ) -> sdk::module::DispatchResult<cbor::Value, Result<cbor::Value, RuntimeError>> {
        match method {
            "keyvalue.Get" => sdk::module::dispatch_query(ctx, args, Self::query_get),
            _ => sdk::module::DispatchResult::Unhandled(args),
        }
    }
}

// Actual implementation of this runtime's externally-callable methods.
impl Module {
    /// Insert given keyvalue into storage.
    fn tx_insert<C: Context>(_ctx: &C, body: types::KeyValue) -> Result<(), Error> {
        let params = Self::params();

        if CurrentState::with_env(|env| env.is_simulation()) {
            <C::Runtime as Runtime>::Core::use_tx_gas(max(
                params.gas_costs.insert_absent,
                params.gas_costs.insert_existing,
            ))?;
            return Ok(());
        }

        let cost = CurrentState::with_store(|store| {
            let mut store = sdk::storage::PrefixStore::new(store, &MODULE_NAME);
            let ts = sdk::storage::TypedStore::new(&mut store);
            match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
                None => params.gas_costs.insert_absent,
                Some(_) => params.gas_costs.insert_existing,
            }
        });
        <C::Runtime as Runtime>::Core::use_tx_gas(cost)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let bc = body.clone();
        CurrentState::with(|state| {
            let mut store = sdk::storage::PrefixStore::new(state.store(), &MODULE_NAME);
            let mut ts = sdk::storage::TypedStore::new(&mut store);
            ts.insert(&body.key, body.value);

            state.emit_event(Event::Insert { kv: bc });
        });

        Ok(())
    }

    /// Remove keyvalue from storage using given key.
    fn tx_remove<C: Context>(_ctx: &C, body: types::Key) -> Result<(), Error> {
        let params = Self::params();

        if CurrentState::with_env(|env| env.is_simulation()) {
            <C::Runtime as Runtime>::Core::use_tx_gas(max(
                params.gas_costs.remove_absent,
                params.gas_costs.remove_existing,
            ))?;
            return Ok(());
        }

        let cost = CurrentState::with_store(|store| {
            let mut store = sdk::storage::PrefixStore::new(store, &MODULE_NAME);
            let ts = sdk::storage::TypedStore::new(&mut store);
            match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
                None => params.gas_costs.remove_absent,
                Some(_) => params.gas_costs.remove_existing,
            }
        });
        <C::Runtime as Runtime>::Core::use_tx_gas(cost)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let bc = body.clone();
        CurrentState::with(|state| {
            let mut store = sdk::storage::PrefixStore::new(state.store(), &MODULE_NAME);
            let mut ts = sdk::storage::TypedStore::new(&mut store);
            ts.remove(&body.key);

            state.emit_event(Event::Remove { key: bc });
        });

        Ok(())
    }

    fn tx_getcreatekey<C: Context>(ctx: &C, body: types::Key) -> Result<(), Error> {
        if CurrentState::with_env(|env| !env.is_execute()) {
            return Ok(());
        }

        let key_result = ctx
            .key_manager()
            .unwrap()
            .get_or_create_keys(KeyPairId::from(Hash::digest_bytes(&body.key).as_ref()));
        match key_result {
            Ok(_) => Ok(()),
            Err(err) => Err(Error::Abort(sdk::dispatcher::Error::KeyManagerFailure(err))),
        }
    }

    fn get_key_pair<C: Context>(ctx: &C, id: &[u8]) -> Result<KeyPair, Error> {
        let kid = get_key_pair_id([id]);
        let kmgr = ctx
            .key_manager()
            .ok_or(Error::Abort(sdk::dispatcher::Error::Aborted))?;
        kmgr.get_or_create_keys(kid)
            .map_err(|err| Error::Abort(sdk::dispatcher::Error::KeyManagerFailure(err)))
    }

    fn with_confidential_store<F, R>(key_pair: KeyPair, f: F) -> R
    where
        F: FnOnce(&mut TypedStore<ConfidentialStore<PrefixStore<&mut dyn Store, &&str>>>) -> R,
    {
        CurrentState::with_store(|store| {
            let inner_store = PrefixStore::new(store, &MODULE_NAME);
            let confidential_store = ConfidentialStore::new_with_key(
                inner_store,
                key_pair.state_key.0,
                &[b"simple-keyvalue"],
            );
            let mut store = TypedStore::new(confidential_store);

            f(&mut store)
        })
    }

    /// Fetch keyvalue from confidential storage using given key.
    fn tx_confidential_get<C: Context>(
        ctx: &C,
        body: types::ConfidentialKey,
    ) -> Result<types::KeyValue, Error> {
        if CurrentState::with_env(|env| !env.is_execute()) {
            return Ok(types::KeyValue {
                key: Vec::new(),
                value: Vec::new(),
            });
        }

        let key_pair = Self::get_key_pair(ctx, body.key_id.as_ref())?;
        let v: Vec<u8> = Self::with_confidential_store(key_pair, |ts| {
            ts.get(body.key.clone()).ok_or(Error::InvalidArgument)
        })?;

        Ok(types::KeyValue {
            key: body.key,
            value: v,
        })
    }

    /// Insert given keyvalue into confidential storage.
    fn tx_confidential_insert<C: Context>(
        ctx: &C,
        body: types::ConfidentialKeyValue,
    ) -> Result<(), Error> {
        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let params = Self::params();

        if CurrentState::with_env(|env| env.is_simulation()) {
            <C::Runtime as Runtime>::Core::use_tx_gas(max(
                params.gas_costs.confidential_insert_absent,
                params.gas_costs.confidential_insert_existing,
            ))?;
            return Ok(());
        }

        let key_pair = Self::get_key_pair(ctx, body.key_id.as_ref())?;
        let cost = Self::with_confidential_store(key_pair.clone(), |ts| {
            match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
                None => params.gas_costs.confidential_insert_absent,
                Some(_) => params.gas_costs.confidential_insert_existing,
            }
        });
        <C::Runtime as Runtime>::Core::use_tx_gas(cost)?;

        // Recreate store and ts after we get ctx back
        Self::with_confidential_store(key_pair, |ts| {
            ts.insert(&body.key, body.value.clone());
        });

        CurrentState::with(|state| {
            state.emit_event(Event::Insert {
                kv: types::KeyValue {
                    key: body.key,
                    value: body.value,
                },
            });
        });

        Ok(())
    }

    /// Remove keyvalue from confidential storage using given key.
    fn tx_confidential_remove<C: Context>(
        ctx: &C,
        body: types::ConfidentialKey,
    ) -> Result<(), Error> {
        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let params = Self::params();

        if CurrentState::with_env(|env| env.is_simulation()) {
            <C::Runtime as Runtime>::Core::use_tx_gas(max(
                params.gas_costs.confidential_remove_absent,
                params.gas_costs.confidential_remove_existing,
            ))?;
            return Ok(());
        }

        let key_pair = Self::get_key_pair(ctx, body.key_id.as_ref())?;
        let cost = Self::with_confidential_store(key_pair.clone(), |ts| {
            match ts.get::<_, Vec<u8>>(body.key.as_slice()) {
                None => params.gas_costs.confidential_remove_absent,
                Some(_) => params.gas_costs.confidential_remove_existing,
            }
        });
        <C::Runtime as Runtime>::Core::use_tx_gas(cost)?;

        // Recreate store and ts after we get ctx back
        Self::with_confidential_store(key_pair, |ts| {
            ts.remove(&body.key);
        });

        CurrentState::with(|state| {
            state.emit_event(Event::Remove {
                key: types::Key {
                    key: body.key.clone(),
                },
            });
        });

        Ok(())
    }

    /// Fetch keyvalue from storage using given key.
    fn query_get<C: Context>(_ctx: &C, body: types::Key) -> Result<types::KeyValue, Error> {
        let v: Vec<u8> = CurrentState::with_store(|store| {
            let mut store = sdk::storage::PrefixStore::new(store, &MODULE_NAME);
            let ts = sdk::storage::TypedStore::new(&mut store);
            ts.get(body.key.clone()).ok_or(Error::InvalidArgument)
        })?;

        Ok(types::KeyValue {
            key: body.key,
            value: v,
        })
    }
}

impl sdk::module::MigrationHandler for Module {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        _ctx: &C,
        meta: &mut sdk::modules::core::types::Metadata,
        genesis: Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            Self::set_params(genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}
