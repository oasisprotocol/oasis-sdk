//! On-chain coordination for ROFL components.
use std::collections::{BTreeMap, BTreeSet};

use once_cell::sync::Lazy;

use crate::{
    context::Context,
    core::{
        common::crypto::signature::PublicKey as CorePublicKey,
        consensus::{
            registry::{Node, RolesMask, VerifiedEndorsedCapabilityTEE},
            state::registry::ImmutableState as RegistryImmutableState,
        },
    },
    crypto::signature::PublicKey,
    dispatcher, handler, keymanager, migration,
    module::{self, Module as _, Parameters as _},
    modules::{self, accounts::API as _, core::API as _},
    sdk_derive,
    state::CurrentState,
    types::{address::Address, transaction::Transaction},
    Runtime,
};

pub mod app;
pub mod app_id;
mod config;
mod error;
mod event;
pub mod policy;
pub mod state;
#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "rofl";

pub use config::Config;
pub use error::Error;
pub use event::Event;

/// Parameters for the module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {}

/// Errors emitted during parameter validation.
#[derive(thiserror::Error, Debug)]
pub enum ParameterValidationError {}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    fn validate_basic(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Genesis state for the module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,

    /// Application configurations.
    pub apps: Vec<types::AppConfig>,
}

/// Interface that can be called from other modules.
pub trait API {
    /// Get the Runtime Attestation Key of the ROFL app instance in case the origin transaction is
    /// signed by a ROFL instance. Otherwise `None` is returned.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    fn get_origin_rak() -> Option<PublicKey>;

    /// Get the registration descriptor of the ROFL app instance in case the origin transaction is
    /// signed by a ROFL instance of the specified app. Otherwise `None` is returned.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    fn get_origin_registration(app: app_id::AppId) -> Option<types::Registration>;

    /// Verify whether the origin transaction is signed by an authorized ROFL instance for the given
    /// application.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    fn is_authorized_origin(app: app_id::AppId) -> bool;

    /// Get a specific registered instance for an application.
    fn get_registration(app: app_id::AppId, rak: PublicKey) -> Result<types::Registration, Error>;

    /// Get an application's configuration.
    fn get_app(id: app_id::AppId) -> Result<types::AppConfig, Error>;

    /// Get all application configurations.
    fn get_apps() -> Result<Vec<types::AppConfig>, Error>;

    /// Get all registered instances for an application.
    fn get_instances(id: app_id::AppId) -> Result<Vec<types::Registration>, Error>;
}

/// Module's address that has the application stake pool.
///
/// oasis1qza6sddnalgzexk3ct30gqfvntgth5m4hsyywmff
pub static ADDRESS_APP_STAKE_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "app-stake-pool"));

/// Key derivation context.
pub static ROFL_DERIVE_KEY_CONTEXT: &[u8] = b"oasis-runtime-sdk/rofl: derive key v1";
/// Secrets encryption key identifier.
pub static ROFL_KEY_ID_SEK: &[u8] = b"oasis-runtime-sdk/rofl: secrets encryption key v1";

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

impl<Cfg: Config> API for Module<Cfg> {
    fn get_origin_rak() -> Option<PublicKey> {
        let caller_pk = CurrentState::with_env_origin(|env| env.tx_caller_public_key())?;

        // Resolve RAK as the call may be made by an extra key.
        state::get_endorser(&caller_pk).map(|kei| match kei {
            // It may point to a RAK.
            state::KeyEndorsementInfo { rak: Some(rak), .. } => rak.into(),
            // Or it points to itself.
            _ => caller_pk,
        })
    }

    fn get_origin_registration(app: app_id::AppId) -> Option<types::Registration> {
        Self::get_origin_rak()
            .and_then(|rak| state::get_registration(app, &rak.try_into().unwrap()))
    }

    fn is_authorized_origin(app: app_id::AppId) -> bool {
        Self::get_origin_registration(app).is_some()
    }

    fn get_registration(app: app_id::AppId, rak: PublicKey) -> Result<types::Registration, Error> {
        state::get_registration(app, &rak.try_into().map_err(|_| Error::InvalidArgument)?)
            .ok_or(Error::UnknownInstance)
    }

    fn get_app(id: app_id::AppId) -> Result<types::AppConfig, Error> {
        state::get_app(id).ok_or(Error::UnknownApp)
    }

    fn get_apps() -> Result<Vec<types::AppConfig>, Error> {
        Ok(state::get_apps())
    }

    fn get_instances(id: app_id::AppId) -> Result<Vec<types::Registration>, Error> {
        Ok(state::get_registrations_for_app(id))
    }
}

#[sdk_derive(Module)]
impl<Cfg: Config> Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
    type Genesis = Genesis;

    #[migration(init)]
    fn init(genesis: Genesis) {
        genesis
            .parameters
            .validate_basic()
            .expect("invalid genesis parameters");

        // Set genesis parameters.
        Self::set_params(genesis.parameters);

        // Insert all applications.
        for cfg in genesis.apps {
            if state::get_app(cfg.id).is_some() {
                panic!("duplicate application in genesis: {:?}", cfg.id);
            }

            state::set_app(cfg);
        }
    }

    /// Create a new ROFL application.
    #[handler(call = "rofl.Create")]
    fn tx_create<C: Context>(ctx: &C, body: types::Create) -> Result<app_id::AppId, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_CREATE)?;

        if body.metadata.len() > Cfg::MAX_METADATA_PAIRS {
            return Err(Error::InvalidArgument);
        }
        for (key, value) in &body.metadata {
            if key.len() > Cfg::MAX_METADATA_KEY_SIZE {
                return Err(Error::InvalidArgument);
            }
            if value.len() > Cfg::MAX_METADATA_VALUE_SIZE {
                return Err(Error::InvalidArgument);
            }
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(Default::default());
        }

        let (creator, tx_index) =
            CurrentState::with_env(|env| (env.tx_caller_address(), env.tx_index()));
        let app_id = match body.scheme {
            types::IdentifierScheme::CreatorRoundIndex => app_id::AppId::from_creator_round_index(
                creator,
                ctx.runtime_header().round,
                tx_index.try_into().map_err(|_| Error::InvalidArgument)?,
            ),
            types::IdentifierScheme::CreatorNonce => {
                let nonce = <C::Runtime as Runtime>::Accounts::get_nonce(creator)?;

                app_id::AppId::from_creator_nonce(creator, nonce)
            }
        };

        // Sanity check that the application doesn't already exist.
        if state::get_app(app_id).is_some() {
            return Err(Error::AppAlreadyExists);
        }

        // Transfer stake.
        <C::Runtime as Runtime>::Accounts::transfer(
            creator,
            *ADDRESS_APP_STAKE_POOL,
            &Cfg::STAKE_APP_CREATE,
        )?;

        // Generate the secret encryption (public) key.
        let sek = Self::derive_app_key(
            ctx,
            &app_id,
            types::KeyKind::X25519,
            types::KeyScope::Global,
            ROFL_KEY_ID_SEK,
            None,
        )?
        .input_keypair
        .pk;

        // Register the application.
        let cfg = types::AppConfig {
            id: app_id,
            policy: body.policy,
            admin: Some(creator),
            stake: Cfg::STAKE_APP_CREATE,
            metadata: body.metadata,
            sek,
            ..Default::default()
        };
        state::set_app(cfg);

        CurrentState::with(|state| state.emit_event(Event::AppCreated { id: app_id }));

        Ok(app_id)
    }

    /// Ensure caller is the current administrator, return an error otherwise.
    fn ensure_caller_is_admin(cfg: &types::AppConfig) -> Result<(), Error> {
        let caller = CurrentState::with_env(|env| env.tx_caller_address());
        if cfg.admin != Some(caller) {
            return Err(Error::Forbidden);
        }
        Ok(())
    }

    /// Update a ROFL application.
    #[handler(call = "rofl.Update")]
    fn tx_update<C: Context>(ctx: &C, body: types::Update) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_UPDATE)?;

        if body.metadata.len() > Cfg::MAX_METADATA_PAIRS {
            return Err(Error::InvalidArgument);
        }
        for (key, value) in &body.metadata {
            if key.len() > Cfg::MAX_METADATA_KEY_SIZE {
                return Err(Error::InvalidArgument);
            }
            if value.len() > Cfg::MAX_METADATA_VALUE_SIZE {
                return Err(Error::InvalidArgument);
            }
        }
        if body.secrets.len() > Cfg::MAX_METADATA_PAIRS {
            return Err(Error::InvalidArgument);
        }
        for (key, value) in &body.secrets {
            if key.len() > Cfg::MAX_METADATA_KEY_SIZE {
                return Err(Error::InvalidArgument);
            }
            if value.len() > Cfg::MAX_METADATA_VALUE_SIZE {
                return Err(Error::InvalidArgument);
            }
        }

        let mut cfg = state::get_app(body.id).ok_or(Error::UnknownApp)?;

        // Ensure caller is the admin and is allowed to update the configuration.
        Self::ensure_caller_is_admin(&cfg)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        // If there is no SEK defined, regenerate it.
        if cfg.sek == Default::default() {
            cfg.sek = Self::derive_app_key(
                ctx,
                &body.id,
                types::KeyKind::X25519,
                types::KeyScope::Global,
                ROFL_KEY_ID_SEK,
                None,
            )?
            .input_keypair
            .pk;
        }

        cfg.policy = body.policy;
        cfg.admin = body.admin;
        cfg.metadata = body.metadata;
        cfg.secrets = body.secrets;
        state::set_app(cfg);

        CurrentState::with(|state| state.emit_event(Event::AppUpdated { id: body.id }));

        Ok(())
    }

    /// Remove a ROFL application.
    #[handler(call = "rofl.Remove")]
    fn tx_remove<C: Context>(ctx: &C, body: types::Remove) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_REMOVE)?;

        let cfg = state::get_app(body.id).ok_or(Error::UnknownApp)?;

        // Ensure caller is the admin and is allowed to update the configuration.
        Self::ensure_caller_is_admin(&cfg)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        state::remove_app(body.id);

        // Return stake to the administrator account.
        if let Some(admin) = cfg.admin {
            <C::Runtime as Runtime>::Accounts::transfer(
                *ADDRESS_APP_STAKE_POOL,
                admin,
                &cfg.stake,
            )?;
        }

        CurrentState::with(|state| state.emit_event(Event::AppRemoved { id: body.id }));

        Ok(())
    }

    /// Register a new ROFL instance.
    #[handler(call = "rofl.Register")]
    fn tx_register<C: Context>(ctx: &C, body: types::Register) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_REGISTER)?;

        if body.expiration <= ctx.epoch() {
            return Err(Error::RegistrationExpired);
        }

        if body.metadata.len() > Cfg::MAX_METADATA_PAIRS {
            return Err(Error::InvalidArgument);
        }
        for (key, value) in &body.metadata {
            if key.len() > Cfg::MAX_METADATA_KEY_SIZE {
                return Err(Error::InvalidArgument);
            }
            if value.len() > Cfg::MAX_METADATA_VALUE_SIZE {
                return Err(Error::InvalidArgument);
            }
        }

        let cfg = state::get_app(body.app).ok_or(Error::UnknownApp)?;

        if body.expiration - ctx.epoch() > cfg.policy.max_expiration {
            return Err(Error::InvalidArgument);
        }

        // Ensure that the transaction is signed by RAK (and co-signed by extra keys).
        let signer_pks: BTreeSet<PublicKey> = CurrentState::with_env(|env| {
            env.tx_auth_info()
                .signer_info
                .iter()
                .filter_map(|si| si.address_spec.public_key())
                .collect()
        });
        if !signer_pks.contains(&body.ect.capability_tee.rak.into()) {
            return Err(Error::NotSignedByRAK);
        }
        for extra_pk in &body.extra_keys {
            if !signer_pks.contains(extra_pk) {
                return Err(Error::NotSignedByExtraKey);
            }
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        // Verify policy.
        let verified_ect = body
            .ect
            .verify(&cfg.policy.quotes)
            .map_err(|_| Error::InvalidArgument)?;

        // Verify enclave identity.
        if !cfg
            .policy
            .enclaves
            .contains(&verified_ect.verified_attestation.quote.identity)
        {
            return Err(Error::UnknownEnclave);
        }

        // Verify allowed endorsement.
        let node = Self::verify_endorsement(ctx, &cfg.policy, &verified_ect)?;

        // Update registration.
        let registration = types::Registration {
            app: body.app,
            node_id: verified_ect.node_id.unwrap(), // Verified above.
            entity_id: node.map(|n| n.entity_id),
            rak: body.ect.capability_tee.rak,
            rek: body.ect.capability_tee.rek.ok_or(Error::InvalidArgument)?, // REK required.
            expiration: body.expiration,
            extra_keys: body.extra_keys,
            metadata: body.metadata,
        };
        state::update_registration(registration)?;

        CurrentState::with(|state| {
            state.emit_event(Event::InstanceRegistered {
                app_id: body.app,
                rak: body.ect.capability_tee.rak.into(),
            })
        });

        Ok(())
    }

    /// Derive a ROFL application-specific key.
    #[handler(call = "rofl.DeriveKey")]
    fn tx_derive_key<C: Context>(
        ctx: &C,
        body: types::DeriveKey,
    ) -> Result<types::DeriveKeyResponse, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_DERIVE_KEY)?;

        // Ensure call is encrypted to avoid leaking any keys by accident.
        let call_format = CurrentState::with_env(|env| env.tx_call_format());
        if !call_format.is_encrypted() {
            return Err(Error::PlainCallFormatNotAllowed);
        }

        // Currently only generation zero keys are supported.
        if body.generation != 0 {
            return Err(Error::InvalidArgument);
        }

        // Ensure key identifier is not too long.
        if body.key_id.len() > Cfg::DERIVE_KEY_MAX_KEY_ID_LENGTH {
            return Err(Error::InvalidArgument);
        }

        // Disallow invocation from subcalls.
        if CurrentState::with_env(|env| env.is_internal()) {
            return Err(Error::Forbidden);
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(Default::default());
        }

        // Ensure caller is an authorized instance of the given application.
        let reg = Self::get_origin_registration(body.app).ok_or(Error::Forbidden)?;

        // Derive application key.
        let key = Self::derive_app_key(
            ctx,
            &body.app,
            body.kind,
            body.scope,
            &body.key_id,
            Some(reg),
        )?;
        let key = match body.kind {
            types::KeyKind::EntropyV0 => key.state_key.0.into(),
            types::KeyKind::X25519 => key.input_keypair.sk.as_ref().into(),
        };

        Ok(types::DeriveKeyResponse { key })
    }

    fn derive_app_key_id(
        app: &app_id::AppId,
        kind: types::KeyKind,
        scope: types::KeyScope,
        key_id: &[u8],
        reg: Option<types::Registration>,
    ) -> Result<keymanager::KeyPairId, Error> {
        // Build the base key identifier.
        //
        // We use the following tuple elements which are fed into TupleHash to derive the final key
        // identifier, in order:
        //
        // - V1 context domain separator.
        // - App ID.
        // - Encoded kind.
        // - Key ID.
        // - Optional CBOR-serialized extra domain separation.
        //
        let kind_id = &[kind as u8];
        let mut key_id = vec![ROFL_DERIVE_KEY_CONTEXT, app.as_ref(), kind_id, key_id];
        let mut extra_dom: BTreeMap<&str, Vec<u8>> = BTreeMap::new();

        match scope {
            types::KeyScope::Global => {
                // Nothing to do here, global keys don't include an explicit scope for backwards
                // compatibility.
            }
            types::KeyScope::Node => {
                // Fetch node identifier corresponding to the application instance.
                let node_id = reg.ok_or(Error::InvalidArgument)?.node_id;

                extra_dom.insert("scope", [scope as u8].to_vec());
                extra_dom.insert("node_id", node_id.as_ref().to_vec());
            }
            types::KeyScope::Entity => {
                // Fetch entity identifier corresponding to the application instance.
                let entity_id = reg
                    .ok_or(Error::InvalidArgument)?
                    .entity_id
                    .ok_or(Error::InvalidArgument)?;

                extra_dom.insert("scope", [scope as u8].to_vec());
                extra_dom.insert("entity_id", entity_id.as_ref().to_vec());
            }
        };

        // Add optional extra domain separation.
        let extra_dom = if !extra_dom.is_empty() {
            cbor::to_vec(extra_dom)
        } else {
            vec![]
        };
        if !extra_dom.is_empty() {
            key_id.push(&extra_dom)
        }

        // Finalize the key identifier.
        Ok(keymanager::get_key_pair_id(key_id))
    }

    fn derive_app_key<C: Context>(
        ctx: &C,
        app: &app_id::AppId,
        kind: types::KeyKind,
        scope: types::KeyScope,
        key_id: &[u8],
        reg: Option<types::Registration>,
    ) -> Result<keymanager::KeyPair, Error> {
        let key_id = Self::derive_app_key_id(app, kind, scope, key_id, reg)?;

        let km = ctx
            .key_manager()
            .ok_or(Error::Abort(dispatcher::Error::KeyManagerFailure(
                keymanager::KeyManagerError::NotInitialized,
            )))?;
        km.get_or_create_keys(key_id)
            .map_err(|err| Error::Abort(dispatcher::Error::KeyManagerFailure(err)))
    }

    /// Verify whether the given endorsement is allowed by the application policy.
    ///
    /// Returns an optional endorsing node descriptor when available.
    fn verify_endorsement<C: Context>(
        ctx: &C,
        app_policy: &policy::AppAuthPolicy,
        ect: &VerifiedEndorsedCapabilityTEE,
    ) -> Result<Option<Node>, Error> {
        use policy::AllowedEndorsement;

        let endorsing_node_id = ect.node_id.ok_or(Error::UnknownNode)?;

        // Attempt to resolve the node that endorsed the enclave. It may be that the node is not
        // even registered in the consensus layer which may be acceptable for some policies.
        let maybe_node = || -> Result<Option<Node>, Error> {
            let registry = RegistryImmutableState::new(ctx.consensus_state());
            let node = registry
                .node(&endorsing_node_id)
                .map_err(|_| Error::UnknownNode)?;
            let node = if let Some(node) = node {
                node
            } else {
                return Ok(None);
            };
            // Ensure node is not expired.
            if node.expiration < ctx.epoch() {
                return Ok(None);
            }

            Ok(Some(node))
        }()?;

        // Ensure node is registered for this runtime.
        let has_runtime = |node: &Node| -> bool {
            let version = &<C::Runtime as Runtime>::VERSION;
            node.get_runtime(ctx.runtime_id(), version).is_some()
        };

        for allowed in &app_policy.endorsements {
            match (allowed, &maybe_node) {
                (AllowedEndorsement::Any, _) => {
                    // Any node is allowed.
                    return Ok(maybe_node);
                }
                (AllowedEndorsement::ComputeRole, Some(node)) => {
                    if node.has_roles(RolesMask::ROLE_COMPUTE_WORKER) && has_runtime(node) {
                        return Ok(maybe_node);
                    }
                }
                (AllowedEndorsement::ObserverRole, Some(node)) => {
                    if node.has_roles(RolesMask::ROLE_OBSERVER) && has_runtime(node) {
                        return Ok(maybe_node);
                    }
                }
                (AllowedEndorsement::Entity(entity_id), Some(node)) => {
                    // If a specific entity is required, it may be registered for any runtime.
                    if &node.entity_id == entity_id {
                        return Ok(maybe_node);
                    }
                }
                (AllowedEndorsement::Node(node_id), _) => {
                    if endorsing_node_id == *node_id {
                        return Ok(maybe_node);
                    }
                }
                _ => continue,
            }
        }

        // If nothing matched, this node is not allowed to register.
        Err(Error::NodeNotAllowed)
    }

    /// Verify whether the origin transaction is signed by an authorized ROFL instance for the given
    /// application.
    #[handler(call = "rofl.IsAuthorizedOrigin", internal)]
    fn internal_is_authorized_origin<C: Context>(
        _ctx: &C,
        app: app_id::AppId,
    ) -> Result<bool, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_IS_AUTHORIZED_ORIGIN)?;

        Ok(Self::is_authorized_origin(app))
    }

    #[handler(call = "rofl.AuthorizedOriginNode", internal)]
    fn internal_authorized_origin_node<C: Context>(
        _ctx: &C,
        app: app_id::AppId,
    ) -> Result<CorePublicKey, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_AUTHORIZED_ORIGIN_NODE)?;

        let registration = Self::get_origin_registration(app).ok_or(Error::UnknownInstance)?;
        Ok(registration.node_id)
    }

    #[handler(call = "rofl.AuthorizedOriginEntity", internal)]
    fn internal_authorized_origin_entity<C: Context>(
        _ctx: &C,
        app: app_id::AppId,
    ) -> Result<Option<CorePublicKey>, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_AUTHORIZED_ORIGIN_ENTITY)?;

        let registration = Self::get_origin_registration(app).ok_or(Error::UnknownInstance)?;
        Ok(registration.entity_id)
    }

    /// Returns the configuration for the given ROFL application.
    #[handler(query = "rofl.App")]
    fn query_app<C: Context>(_ctx: &C, args: types::AppQuery) -> Result<types::AppConfig, Error> {
        Self::get_app(args.id)
    }

    /// Returns all ROFL app configurations.
    #[handler(query = "rofl.Apps", expensive)]
    fn query_apps<C: Context>(_ctx: &C, _args: ()) -> Result<Vec<types::AppConfig>, Error> {
        Self::get_apps()
    }

    /// Returns a specific registered instance for the given ROFL application.
    #[handler(query = "rofl.AppInstance")]
    fn query_app_instance<C: Context>(
        _ctx: &C,
        args: types::AppInstanceQuery,
    ) -> Result<types::Registration, Error> {
        Self::get_registration(args.app, args.rak)
    }

    /// Returns a list of all registered instances for the given ROFL application.
    #[handler(query = "rofl.AppInstances", expensive)]
    fn query_app_instances<C: Context>(
        _ctx: &C,
        args: types::AppQuery,
    ) -> Result<Vec<types::Registration>, Error> {
        Self::get_instances(args.id)
    }

    /// Returns the minimum stake thresholds for managing ROFL.
    #[handler(query = "rofl.StakeThresholds")]
    fn query_stake_thresholds<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<types::StakeThresholds, Error> {
        Ok(types::StakeThresholds {
            app_create: Cfg::STAKE_APP_CREATE,
        })
    }

    /// Returns the minimum stake thresholds for managing ROFL.
    #[handler(call = "rofl.StakeThresholds", internal)]
    fn internal_query_stake_thresholds<C: Context>(
        ctx: &C,
        _args: (),
    ) -> Result<types::StakeThresholds, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_STAKE_THRESHOLDS)?;
        Ok(types::StakeThresholds {
            app_create: Cfg::STAKE_APP_CREATE,
        })
    }

    fn resolve_payer_from_tx<C: Context>(
        ctx: &C,
        tx: &Transaction,
        app_policy: &policy::AppAuthPolicy,
    ) -> Result<Option<Address>, anyhow::Error> {
        let caller_pk = tx
            .auth_info
            .signer_info
            .first()
            .and_then(|si| si.address_spec.public_key());

        match tx.call.method.as_str() {
            "rofl.Register" => {
                // For registration transactions, extract endorsing node.
                let body: types::Register = cbor::from_value(tx.call.body.clone())?;
                if body.expiration <= ctx.epoch() {
                    return Err(Error::RegistrationExpired.into());
                }

                // Ensure that the transaction is signed by RAK.
                let caller_pk = caller_pk.ok_or(Error::NotSignedByRAK)?;
                if caller_pk != body.ect.capability_tee.rak {
                    return Err(Error::NotSignedByRAK.into());
                }

                body.ect.verify_endorsement()?;

                // Checking other details is not relevant for authorizing fee payments as if the
                // node signed a TEE capability then it is authorizing fees to be spent on its
                // behalf.

                let node_id = body.ect.node_endorsement.public_key;
                let payer = Address::from_consensus_pk(&node_id);

                Ok(Some(payer))
            }
            _ => {
                // For others, check if caller is one of the endorsed keys.
                let caller_pk = match caller_pk {
                    Some(pk) => pk,
                    None => return Ok(None),
                };

                Ok(state::get_endorser(&caller_pk)
                    .map(|ei| Address::from_consensus_pk(&ei.node_id)))
            }
        }
    }
}

impl<Cfg: Config> module::FeeProxyHandler for Module<Cfg> {
    fn resolve_payer<C: Context>(
        ctx: &C,
        tx: &Transaction,
    ) -> Result<Option<Address>, modules::core::Error> {
        use policy::FeePolicy;

        let proxy = if let Some(ref proxy) = tx.auth_info.fee.proxy {
            proxy
        } else {
            return Ok(None);
        };

        if proxy.module != MODULE_NAME {
            return Ok(None);
        }

        // Look up the per-ROFL app policy.
        let app_id = app_id::AppId::try_from(proxy.id.as_slice())
            .map_err(|err| modules::core::Error::InvalidArgument(err.into()))?;
        let app_policy = state::get_app(app_id).map(|cfg| cfg.policy).ok_or(
            modules::core::Error::InvalidArgument(Error::UnknownApp.into()),
        )?;

        match app_policy.fees {
            FeePolicy::InstancePays => {
                // Application needs to figure out a way to pay, defer to regular handler.
                Ok(None)
            }
            FeePolicy::EndorsingNodePays => Self::resolve_payer_from_tx(ctx, tx, &app_policy)
                .map_err(modules::core::Error::InvalidArgument),
        }
    }
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn end_block<C: Context>(ctx: &C) {
        // Only do work in case the epoch has changed since the last processed block.
        if !<C::Runtime as Runtime>::Core::has_epoch_changed() {
            return;
        }

        // Process enclave expirations.
        // TODO: Consider processing unprocessed in the next block(s) if there are too many.
        state::expire_registrations(ctx.epoch(), 128);
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
