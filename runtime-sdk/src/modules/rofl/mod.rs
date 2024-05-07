//! On-chain coordination for ROFL components.
use std::collections::BTreeSet;

use once_cell::sync::Lazy;

use crate::{
    context::Context,
    core::consensus::{
        registry::{Node, RolesMask, VerifiedEndorsedCapabilityTEE},
        state::registry::ImmutableState as RegistryImmutableState,
    },
    crypto::signature::PublicKey,
    handler, migration,
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
    /// Verify whether the origin transaction is signed by an authorized ROFL instance for the given
    /// application.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    fn is_authorized_origin(app: app_id::AppId) -> Result<bool, Error>;

    /// Get an application's configuration.
    fn get_app(id: app_id::AppId) -> Result<types::AppConfig, Error>;

    /// Get all registered instances for an application.
    fn get_instances(id: app_id::AppId) -> Result<Vec<types::Registration>, Error>;
}

/// Module's address that has the application stake pool.
///
/// oasis1qza6sddnalgzexk3ct30gqfvntgth5m4hsyywmff
pub static ADDRESS_APP_STAKE_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "app-stake-pool"));

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

impl<Cfg: Config> API for Module<Cfg> {
    fn is_authorized_origin(app: app_id::AppId) -> Result<bool, Error> {
        let caller_pk = CurrentState::with_env_origin(|env| env.tx_caller_public_key())
            .ok_or(Error::InvalidArgument)?;

        // Resolve RAK as the call may be made by an extra key.
        let rak = match state::get_endorser(&caller_pk) {
            // It may point to a RAK.
            Some(state::KeyEndorsementInfo { rak: Some(rak), .. }) => rak,
            // Or it points to itself.
            Some(_) => caller_pk.try_into().map_err(|_| Error::InvalidArgument)?,
            // Or is unknown.
            None => return Ok(false),
        };

        // Check whether the the endorsement is for the right application.
        Ok(state::get_registration(app, &rak).is_some())
    }

    fn get_app(id: app_id::AppId) -> Result<types::AppConfig, Error> {
        state::get_app(id).ok_or(Error::UnknownApp)
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

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(Default::default());
        }

        let (creator, tx_index) =
            CurrentState::with_env(|env| (env.tx_caller_address(), env.tx_index()));
        let app_id = app_id::AppId::from_creator_round_index(
            creator,
            ctx.runtime_header().round,
            tx_index.try_into().map_err(|_| Error::InvalidArgument)?,
        );

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

        // Register the application.
        let cfg = types::AppConfig {
            id: app_id,
            policy: body.policy,
            admin: Some(creator),
            stake: Cfg::STAKE_APP_CREATE,
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

        let mut cfg = state::get_app(body.id).ok_or(Error::UnknownApp)?;

        // Ensure caller is the admin and is allowed to update the configuration.
        Self::ensure_caller_is_admin(&cfg)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        // Return early if nothing has actually changed.
        if cfg.policy == body.policy && cfg.admin == body.admin {
            return Ok(());
        }

        cfg.policy = body.policy;
        cfg.admin = body.admin;
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
        Self::verify_endorsement(ctx, &cfg.policy, &verified_ect)?;

        // Update registration.
        let registration = types::Registration {
            app: body.app,
            node_id: verified_ect.node_id.unwrap(), // Verified above.
            rak: body.ect.capability_tee.rak,
            rek: body.ect.capability_tee.rek.ok_or(Error::InvalidArgument)?, // REK required.
            expiration: body.expiration,
            extra_keys: body.extra_keys,
        };
        state::update_registration(registration)?;

        Ok(())
    }

    /// Verify whether the given endorsement is allowed by the application policy.
    fn verify_endorsement<C: Context>(
        ctx: &C,
        app_policy: &policy::AppAuthPolicy,
        ect: &VerifiedEndorsedCapabilityTEE,
    ) -> Result<(), Error> {
        use policy::AllowedEndorsement;

        let endorsing_node_id = ect.node_id.ok_or(Error::UnknownNode)?;

        // Attempt to resolve the node that endorsed the enclave. It may be that the node is not
        // even registered in the consensus layer which may be acceptable for some policies.
        //
        // But if the node is registered, it must be registered for this runtime, otherwise it is
        // treated as if it is not registered.
        let node = || -> Result<Option<Node>, Error> {
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
            // Ensure node is registered for this runtime.
            let version = &<C::Runtime as Runtime>::VERSION;
            if node.get_runtime(ctx.runtime_id(), version).is_none() {
                return Ok(None);
            }

            Ok(Some(node))
        }()?;

        for allowed in &app_policy.endorsements {
            match (allowed, &node) {
                (AllowedEndorsement::Any, _) => {
                    // Any node is allowed.
                    return Ok(());
                }
                (AllowedEndorsement::ComputeRole, Some(node)) => {
                    if node.has_roles(RolesMask::ROLE_COMPUTE_WORKER) {
                        return Ok(());
                    }
                }
                (AllowedEndorsement::ObserverRole, Some(node)) => {
                    if node.has_roles(RolesMask::ROLE_OBSERVER) {
                        return Ok(());
                    }
                }
                (AllowedEndorsement::Entity(entity_id), Some(node)) => {
                    if &node.entity_id == entity_id {
                        return Ok(());
                    }
                }
                (AllowedEndorsement::Node(node_id), _) => {
                    if endorsing_node_id == *node_id {
                        return Ok(());
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

        Self::is_authorized_origin(app)
    }

    /// Returns the configuration for the given ROFL application.
    #[handler(query = "rofl.App")]
    fn query_app<C: Context>(_ctx: &C, args: types::AppQuery) -> Result<types::AppConfig, Error> {
        Self::get_app(args.id)
    }

    /// Returns a list of all registered instances for the given ROFL application.
    #[handler(query = "rofl.AppInstances", expensive)]
    fn query_app_instances<C: Context>(
        _ctx: &C,
        args: types::AppQuery,
    ) -> Result<Vec<types::Registration>, Error> {
        Self::get_instances(args.id)
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
            FeePolicy::AppPays => {
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
