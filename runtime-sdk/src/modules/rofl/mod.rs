//! On-chain coordination for ROFL components.
use crate::{
    context::Context,
    core::consensus::{
        registry::{RolesMask, VerifiedEndorsedCapabilityTEE},
        state::registry::ImmutableState as RegistryImmutableState,
    },
    handler, migration,
    module::{self, Module as _, Parameters as _},
    modules::{self, core::API as _},
    sdk_derive,
    state::CurrentState,
    types::{address::Address, transaction::Transaction},
    Runtime,
};

pub mod policy;
pub mod state;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "rofl";

/// Module configuration.
pub trait Config: 'static {
    /// Module that is used for accessing accounts.
    type Accounts: modules::accounts::API;

    /// ROFL enclave authorization policy.
    fn auth_policy() -> policy::AuthPolicy {
        Default::default() // Default policy does not allow anything.
    }
}

/// Errors emitted by the module.
#[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("unknown application")]
    #[sdk_error(code = 2)]
    UnknownApp,

    #[error("tx not signed by RAK")]
    #[sdk_error(code = 3)]
    NotSignedByRAK,

    #[error("unknown enclave")]
    #[sdk_error(code = 4)]
    UnknownEnclave,

    #[error("unknown node")]
    #[sdk_error(code = 5)]
    UnknownNode,

    #[error("endorsement from given node not allowed")]
    #[sdk_error(code = 6)]
    NodeNotAllowed,

    #[error("registration expired")]
    #[sdk_error(code = 7)]
    RegistrationExpired,

    #[error("extra key update not allowed")]
    #[sdk_error(code = 8)]
    ExtraKeyUpdateNotAllowed,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
}

/// Gas costs.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct GasCosts {
    pub tx_register: u64,
    pub internal_is_authorized_origin: u64,
}

/// Parameters for the module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {
    pub gas_costs: GasCosts,
}

/// Errors emitted during rewards parameter validation.
#[derive(thiserror::Error, Debug)]
pub enum ParameterValidationError {}

impl module::Parameters for Parameters {
    type Error = ParameterValidationError;

    fn validate_basic(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Genesis state for the rewards module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
    pub parameters: Parameters,
}

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

#[sdk_derive(Module)]
impl<Cfg: Config> Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    type Error = Error;
    type Event = ();
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
    }

    /// Register a new ROFL instance.
    #[handler(call = "rofl.Register")]
    fn tx_register<C: Context>(ctx: &C, body: types::Register) -> Result<(), Error> {
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(params.gas_costs.tx_register)?;

        if body.expiration <= ctx.epoch() {
            return Err(Error::RegistrationExpired);
        }

        let policy = Cfg::auth_policy();
        let app_policy = policy.apps.get(&body.app).ok_or(Error::UnknownApp)?;

        if body.expiration - ctx.epoch() > app_policy.max_expiration {
            return Err(Error::InvalidArgument);
        }

        // Ensure that the transaction is signed by RAK.
        let caller_pk = CurrentState::with_env(|env| env.tx_caller_public_key())
            .ok_or(Error::NotSignedByRAK)?;
        if caller_pk != body.ect.capability_tee.rak {
            return Err(Error::NotSignedByRAK);
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        // Verify policy.
        let verified_ect = body
            .ect
            .verify(&app_policy.quotes)
            .map_err(|_| Error::InvalidArgument)?;

        // Verify enclave identity.
        if !app_policy
            .enclaves
            .contains(&verified_ect.verified_quote.identity)
        {
            return Err(Error::UnknownEnclave);
        }

        // Verify allowed endorsement.
        Self::verify_endorsement(ctx, app_policy, &verified_ect)?;

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

        // In all cases, we need to resolve the node that endorsed the enclave.
        let registry = RegistryImmutableState::new(ctx.consensus_state());
        let node = registry
            .node(&ect.node_id.ok_or(Error::UnknownNode)?)
            .map_err(|_| Error::UnknownNode)?
            .ok_or(Error::UnknownNode)?;
        // Ensure node is not expired.
        if node.expiration < ctx.epoch() {
            return Err(Error::UnknownNode);
        }
        // Ensure node is registered for this runtime.
        let version = &<C::Runtime as Runtime>::VERSION;
        node.get_runtime(ctx.runtime_id(), version)
            .ok_or(Error::NodeNotAllowed)?;

        for allowed in &app_policy.endorsement {
            match allowed {
                AllowedEndorsement::Any => {
                    // As long as the node is registered (checked above), it is allowed.
                    return Ok(());
                }
                AllowedEndorsement::ComputeRole => {
                    if node.has_roles(RolesMask::ROLE_COMPUTE_WORKER) {
                        return Ok(());
                    }
                }
                AllowedEndorsement::ObserverRole => {
                    if node.has_roles(RolesMask::ROLE_OBSERVER) {
                        return Ok(());
                    }
                }
                AllowedEndorsement::Entity(entity_id) => {
                    if &node.entity_id == entity_id {
                        return Ok(());
                    }
                }
            }
        }

        // If nothing matched, this node is not allowed to register.
        Err(Error::NodeNotAllowed)
    }

    /// Verify whether the origin transaction is signed is an authorized ROFL instance for the given
    /// application.
    #[handler(call = "rofl.IsAuthorizedOrigin", internal)]
    fn internal_is_authorized_origin<C: Context>(ctx: &C, app: String) -> Result<bool, Error> {
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(params.gas_costs.internal_is_authorized_origin)?;

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

        // We need to also fetch the registration to ensure it is registered for the right app.
        let registration = match state::get_registration(&rak) {
            Some(registration) => registration,
            None => return Ok(false),
        };

        // Ensure enclave is registered for the correct app.
        if registration.app != app {
            return Ok(false);
        }

        Ok(true)
    }

    fn resolve_payer_from_tx<C: Context>(
        ctx: &C,
        tx: &Transaction,
        app_policy: &policy::AppAuthPolicy,
    ) -> Result<Option<Address>, anyhow::Error> {
        match tx.call.method.as_str() {
            "rofl.Register" => {
                // For registration transactions, extract endorsing node.
                let body: types::Register = cbor::from_value(tx.call.body.clone())?;
                if body.expiration <= ctx.epoch() {
                    return Err(Error::RegistrationExpired.into());
                }

                // Ensure that the transaction is signed by RAK.
                let caller_pk = CurrentState::with_env(|env| env.tx_caller_public_key())
                    .ok_or(Error::NotSignedByRAK)?;
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
                let caller_pk = match CurrentState::with_env(|env| env.tx_caller_public_key()) {
                    Some(pk) => pk,
                    _ => return Ok(None),
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
        let policy = Cfg::auth_policy();
        let app_id = String::from_utf8_lossy(&proxy.id);
        let app_policy = if let Some(app_policy) = policy.apps.get(app_id.as_ref()) {
            app_policy
        } else {
            return Ok(None);
        };

        match app_policy.fees {
            FeePolicy::AppPays => {
                // Application needs to figure out a way to pay, defer to regular handler.
                Ok(None)
            }
            FeePolicy::EndorsingNodePays | FeePolicy::EndorsingNodePaysWithReimbursement => {
                Self::resolve_payer_from_tx(ctx, tx, app_policy)
                    .map_err(modules::core::Error::InvalidArgument)
            }
        }
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {
    fn end_block<C: Context>(ctx: &C) {
        // Only do work in case the epoch has changed since the last processed block.
        if !<C::Runtime as Runtime>::Core::has_epoch_changed() {
            return;
        }

        // Process enclave expirations.
        state::expire_registrations(ctx.epoch());
    }
}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
