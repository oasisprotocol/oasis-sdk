use once_cell::sync::Lazy;

use oasis_runtime_sdk::{
    context::Context,
    core::common::crypto::signature::PublicKey,
    handler, migration,
    module::{self, Module as _, Parameters as _},
    modules::{accounts::API as _, core::API as _, rofl::API as _},
    sdk_derive,
    state::CurrentState,
    types::address::Address,
    Runtime,
};

mod config;
mod error;
mod event;
mod payment;
pub mod state;
#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "roflmarket";

pub use config::Config;
pub use error::Error;
pub use event::Event;

use payment::PaymentMethod;

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
}

/// Module's address that has the provider stake pool.
///
/// oasis1qzta0kk6vy0yrwgllual4ntnjay68lp7vq5fs8jy
pub static ADDRESS_PROVIDER_STAKE_POOL: Lazy<Address> =
    Lazy::new(|| Address::from_module(MODULE_NAME, "provider-stake-pool"));

pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
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
    }

    /// Create a new provider.
    #[handler(call = "roflmarket.ProviderCreate")]
    fn tx_provider_create<C: Context>(ctx: &C, body: types::ProviderCreate) -> Result<(), Error> {
        // Pay gas for provider creation.
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_PROVIDER_CREATE)?;

        // Sanity check number of requested offers.
        let offer_count: u64 = body
            .offers
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        if offer_count > Cfg::MAX_PROVIDER_OFFERS {
            return Err(Error::InvalidArgument);
        }

        // Pay gas for creating each offer.
        <C::Runtime as Runtime>::Core::use_tx_gas(
            offer_count.saturating_mul(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_ADD),
        )?;

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
            return Ok(());
        }

        // Sanity check that the provider doesn't already exist.
        let address = CurrentState::with_env(|env| env.tx_caller_address());
        if state::get_provider(address).is_some() {
            return Err(Error::ProviderAlreadyExists);
        }

        // Transfer stake.
        <C::Runtime as Runtime>::Accounts::transfer(
            address,
            *ADDRESS_PROVIDER_STAKE_POOL,
            &Cfg::STAKE_PROVIDER_CREATE,
        )?;

        // Register the provider.
        let provider = types::Provider {
            address,
            nodes: body.nodes,
            scheduler_app: body.scheduler_app,
            payment_address: body.payment_address,
            metadata: body.metadata,
            stake: Cfg::STAKE_PROVIDER_CREATE,
            offers_next_id: offer_count.into(),
            offers_count: offer_count,
            created_at: ctx.now(),
            updated_at: ctx.now(),
            ..Default::default()
        };
        state::set_provider(provider);

        // Create the offers, assigning sequential identifiers to them.
        for (id, mut offer) in body.offers.into_iter().enumerate() {
            offer.validate()?;

            let id: u64 = id.try_into().map_err(|_| Error::InvalidArgument)?;
            offer.id = id.into();

            state::set_offer(address, offer);
        }

        CurrentState::with(|state| state.emit_event(Event::ProviderCreated { address }));

        Ok(())
    }

    /// Ensure caller is the current provider administrator, return an error otherwise.
    fn ensure_caller_is_provider_admin(provider: &types::Provider) -> Result<(), Error> {
        let caller = CurrentState::with_env(|env| env.tx_caller_address());
        if provider.address != caller {
            return Err(Error::Forbidden);
        }
        Ok(())
    }

    /// Update an existing provider.
    #[handler(call = "roflmarket.ProviderUpdate")]
    fn tx_provider_update<C: Context>(ctx: &C, body: types::ProviderUpdate) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_PROVIDER_UPDATE)?;

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
            return Ok(());
        }

        let mut provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_provider_admin(&provider)?;

        provider.nodes = body.nodes;
        provider.scheduler_app = body.scheduler_app;
        provider.payment_address = body.payment_address;
        provider.metadata = body.metadata;
        provider.updated_at = ctx.now();
        state::set_provider(provider);

        CurrentState::with(|state| {
            state.emit_event(Event::ProviderUpdated {
                address: body.provider,
            })
        });

        Ok(())
    }

    #[handler(call = "roflmarket.ProviderUpdateOffers")]
    fn tx_provider_update_offers<C: Context>(
        ctx: &C,
        body: types::ProviderUpdateOffers,
    ) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_BASE)?;

        let add_count: u64 = body
            .add
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        let update_count: u64 = body
            .update
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        let remove_count: u64 = body
            .remove
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;

        <C::Runtime as Runtime>::Core::use_tx_gas(
            add_count.saturating_mul(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_ADD),
        )?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            update_count.saturating_mul(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_ADD),
        )?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            remove_count.saturating_mul(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_RM),
        )?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let mut provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_provider_admin(&provider)?;

        let new_offer_count = provider
            .offers_count
            .saturating_add(add_count)
            .checked_sub(remove_count)
            .ok_or(Error::InvalidArgument)?;
        if new_offer_count > Cfg::MAX_PROVIDER_OFFERS {
            return Err(Error::InvalidArgument);
        }

        for mut offer in body.add {
            offer.validate()?;
            offer.id = provider.offers_next_id.increment();
            state::set_offer(provider.address, offer);
        }
        for offer in body.update {
            offer.validate()?;
            // Ensure the offer exists before updating it to prevent a case where a new offer would
            // be created with a caller-controlled identifier.
            state::get_offer(provider.address, offer.id).ok_or(Error::OfferNotFound)?;
            state::set_offer(provider.address, offer);
        }
        for offer_id in body.remove {
            // Ensure the offer exists before removing it to prevent an incorrect count.
            state::get_offer(provider.address, offer_id).ok_or(Error::OfferNotFound)?;
            state::remove_offer(provider.address, offer_id);
        }

        // Update provider metadata.
        provider.offers_count = new_offer_count;
        provider.updated_at = ctx.now();
        state::set_provider(provider);

        CurrentState::with(|state| {
            state.emit_event(Event::ProviderUpdated {
                address: body.provider,
            })
        });

        Ok(())
    }

    #[handler(call = "roflmarket.ProviderRemove")]
    fn tx_provider_remove<C: Context>(ctx: &C, body: types::ProviderRemove) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_PROVIDER_REMOVE)?;

        if !ctx.should_execute_contracts() {
            return Ok(());
        }

        let provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_provider_admin(&provider)?;

        // Forbid removal if the provider has any associated instances.
        if provider.instances_count > 0 {
            return Err(Error::ProviderHasInstances);
        }

        // Remove all offers, first charging for gas.
        <C::Runtime as Runtime>::Core::use_tx_gas(
            provider
                .offers_count
                .saturating_mul(Cfg::GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_RM),
        )?;
        for offer in state::get_offers(provider.address) {
            state::remove_offer(provider.address, offer.id);
        }

        // Finally remove the provider.
        state::remove_provider(provider.address);

        CurrentState::with(|state| {
            state.emit_event(Event::ProviderRemoved {
                address: provider.address,
            })
        });

        Ok(())
    }

    #[handler(call = "roflmarket.InstanceCreate")]
    fn tx_instance_create<C: Context>(
        ctx: &C,
        body: types::InstanceCreate,
    ) -> Result<types::InstanceId, Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_CREATE)?;

        if body.term_count == 0 {
            return Err(Error::InvalidArgument);
        }

        if !ctx.should_execute_contracts() {
            return Ok(Default::default());
        }

        let mut provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        let offer = state::get_offer(provider.address, body.offer).ok_or(Error::OfferNotFound)?;

        if offer.capacity == 0 {
            return Err(Error::OutOfCapacity);
        }

        let caller_address = CurrentState::with_env(|env| env.tx_caller_address());
        let instance_id = provider.instances_next_id.increment();
        let mut instance = types::Instance {
            provider: provider.address,
            id: instance_id,
            offer: offer.id,
            status: types::InstanceStatus::Created,
            creator: caller_address,
            admin: body.admin.unwrap_or(caller_address),
            resources: offer.resources,
            deployment: body.deployment,
            created_at: ctx.now(),
            updated_at: ctx.now(),
            paid_from: ctx.now(),
            paid_until: ctx.now(),
            payment: offer.payment.clone(),
            payment_address: payment::generate_address(body.provider, instance_id),
            ..Default::default()
        };

        // Handle payment.
        offer
            .payment
            .pay(ctx, &mut instance, body.term, body.term_count)?;

        state::set_instance(instance);

        // Update provider metadata.
        provider.instances_count = provider
            .instances_count
            .checked_add(1)
            .ok_or(Error::InvalidArgument)?;
        provider.updated_at = ctx.now();
        state::set_provider(provider);

        CurrentState::with(|state| {
            state.emit_event(Event::InstanceCreated {
                provider: body.provider,
                id: instance_id,
            })
        });

        Ok(instance_id)
    }

    #[handler(call = "roflmarket.InstanceTopUp")]
    fn tx_instance_topup<C: Context>(ctx: &C, body: types::InstanceTopUp) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_TOPUP)?;

        if body.term_count == 0 {
            return Err(Error::InvalidArgument);
        }

        if !ctx.should_execute_contracts() {
            return Ok(());
        }

        let mut instance =
            state::get_instance(body.provider, body.id).ok_or(Error::InstanceNotFound)?;

        if instance.status != types::InstanceStatus::Accepted {
            return Err(Error::InvalidInstanceState);
        }

        // Handle payment.
        instance
            .payment
            .clone()
            .pay(ctx, &mut instance, body.term, body.term_count)?;

        instance.updated_at = ctx.now();
        state::set_instance(instance);

        CurrentState::with(|state| {
            state.emit_event(Event::InstanceUpdated {
                provider: body.provider,
                id: body.id,
            })
        });

        Ok(())
    }

    /// Ensure caller is the provider's scheduler app and return the endorsing node's public key.
    fn ensure_caller_is_scheduler_app(provider: &types::Provider) -> Result<PublicKey, Error> {
        // Skip checks in simulation mode for correct gas estimation.
        // This is fine because no confidential data is being protected here.
        if CurrentState::with_env(|env| env.is_simulation()) {
            return Ok(Default::default());
        }

        let node_id = Cfg::Rofl::get_origin_registration(provider.scheduler_app)
            .map(|r| r.node_id)
            .ok_or(Error::Forbidden)?;

        // Ensure app instance is endorsed by one of the provider's nodes.
        if !provider.nodes.contains(&node_id) {
            return Err(Error::Forbidden);
        }

        Ok(node_id)
    }

    #[handler(call = "roflmarket.InstanceAccept")]
    fn tx_instance_accept<C: Context>(ctx: &C, body: types::InstanceAccept) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_ACCEPT_BASE)?;

        let instance_count: u64 = body
            .ids
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            instance_count.saturating_mul(Cfg::GAS_COST_CALL_INSTANCE_ACCEPT_INSTANCE),
        )?;

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
            return Ok(());
        }

        let provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        let node_id = Self::ensure_caller_is_scheduler_app(&provider)?;

        for instance_id in body.ids {
            let mut instance = match state::get_instance(body.provider, instance_id) {
                Some(instance) => instance,
                None => continue, // Skip instances that have been removed.
            };
            // Skip instances that have already been accepted or have been cancelled.
            if instance.status != types::InstanceStatus::Created {
                continue;
            }

            // Update offer capacity iff the offer still exists. Note that offer capacity mangement
            // is best-effort and the provider can always reset to an arbitrarily high value.
            if let Some(mut offer) = state::get_offer(body.provider, instance.offer) {
                offer.capacity = offer.capacity.saturating_sub(1);
                state::set_offer(body.provider, offer);
            }

            instance.status = types::InstanceStatus::Accepted;
            instance.node_id = Some(node_id);
            instance.metadata = body.metadata.clone();
            instance.updated_at = ctx.now();
            state::set_instance(instance);

            CurrentState::with(|state| {
                state.emit_event(Event::InstanceAccepted {
                    provider: provider.address,
                    id: instance_id,
                })
            });
        }

        Ok(())
    }

    #[handler(call = "roflmarket.InstanceUpdate")]
    fn tx_instance_update<C: Context>(ctx: &C, body: types::InstanceUpdate) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_UPDATE_BASE)?;

        let instance_count: u64 = body
            .updates
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            instance_count.saturating_mul(Cfg::GAS_COST_CALL_INSTANCE_UPDATE_INST),
        )?;

        for update in &body.updates {
            if let Some(metadata) = &update.metadata {
                if metadata.len() > Cfg::MAX_METADATA_PAIRS {
                    return Err(Error::InvalidArgument);
                }
                for (key, value) in metadata {
                    if key.len() > Cfg::MAX_METADATA_KEY_SIZE {
                        return Err(Error::InvalidArgument);
                    }
                    if value.len() > Cfg::MAX_METADATA_VALUE_SIZE {
                        return Err(Error::InvalidArgument);
                    }
                }
            }
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_scheduler_app(&provider)?;

        for update in body.updates {
            let mut changed = false;
            let mut instance =
                state::get_instance(body.provider, update.id).ok_or(Error::InstanceNotFound)?;

            // Update various metadata.
            if let Some(node_id) = update.node_id {
                instance.node_id = Some(node_id);
                changed = true;
            }
            if let Some(deployment) = update.deployment {
                instance.deployment = deployment.into();
                changed = true;
            }
            if let Some(metadata) = update.metadata {
                instance.metadata = metadata;
                changed = true;
            }

            // Complete commands.
            if let Some(last_completed_cmd) = update.last_completed_cmd {
                let cmds =
                    state::get_instance_commands(body.provider, update.id, last_completed_cmd);
                instance.cmd_count = instance
                    .cmd_count
                    .saturating_sub(cmds.len().try_into().map_err(|_| Error::InvalidArgument)?);

                for qc in cmds {
                    state::remove_instance_command(body.provider, update.id, qc.id);
                    changed = true;
                }
            }

            if !changed {
                continue;
            }

            instance.updated_at = ctx.now();
            state::set_instance(instance);

            CurrentState::with(|state| {
                state.emit_event(Event::InstanceUpdated {
                    provider: body.provider,
                    id: update.id,
                })
            });
        }

        Ok(())
    }

    /// Ensure caller is the current instance administrator, return an error otherwise.
    fn ensure_caller_is_instance_admin(instance: &types::Instance) -> Result<(), Error> {
        // Skip checks in simulation mode for correct gas estimation.
        // This is fine because no confidential data is being protected here.
        if CurrentState::with_env(|env| env.is_simulation()) {
            return Ok(());
        }

        let caller = CurrentState::with_env(|env| env.tx_caller_address());
        if instance.admin != caller {
            return Err(Error::Forbidden);
        }
        Ok(())
    }

    #[handler(call = "roflmarket.InstanceCancel")]
    fn tx_instance_cancel<C: Context>(ctx: &C, body: types::InstanceCancel) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_CANCEL)?;

        if !ctx.should_execute_contracts() {
            return Ok(());
        }

        let provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        let mut instance =
            state::get_instance(body.provider, body.id).ok_or(Error::InstanceNotFound)?;
        Self::ensure_caller_is_instance_admin(&instance)?;

        match instance.status {
            types::InstanceStatus::Created
                if ctx.now().saturating_sub(instance.created_at)
                    > Cfg::MAX_INSTANCE_ACCEPT_TIME_SECONDS =>
            {
                // Instance has not yet been accepted and the cancellation is outside the acceptance
                // time window. Refund the entire payment.
                instance.payment.refund(ctx, &instance)?;

                // We can also directly remove the instance.
                state::remove_instance(body.provider, body.id);

                CurrentState::with(|state| {
                    state.emit_event(Event::InstanceRemoved {
                        provider: body.provider,
                        id: body.id,
                    })
                });
            }
            types::InstanceStatus::Cancelled => {
                // Instance was already cancelled, do nothing.
            }
            _ => {
                // The instance has either been accepted or cancelled within the acceptance time
                // window. Make the provider claim the entire prepaid amount.
                instance.updated_at = ctx.now();
                instance.status = types::InstanceStatus::Cancelled;
                instance
                    .payment
                    .clone()
                    .claim(ctx, &provider, &mut instance)?;

                state::set_instance(instance);

                CurrentState::with(|state| {
                    state.emit_event(Event::InstanceCancelled {
                        provider: body.provider,
                        id: body.id,
                    })
                });
            }
        }

        Ok(())
    }

    #[handler(call = "roflmarket.InstanceRemove")]
    fn tx_instance_remove<C: Context>(ctx: &C, body: types::InstanceRemove) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_REMOVE)?;

        if !ctx.should_execute_contracts() {
            return Ok(());
        }

        let mut provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_scheduler_app(&provider)?;
        let mut instance =
            state::get_instance(body.provider, body.id).ok_or(Error::InstanceNotFound)?;

        // Update provider metadata.
        provider.instances_count = provider
            .instances_count
            .checked_sub(1)
            .ok_or(Error::InvalidArgument)?;
        provider.updated_at = ctx.now();

        // If the instance is paid for, refund it.
        if instance.paid_until > ctx.now() {
            instance.payment.refund(ctx, &instance)?;
        } else {
            instance.status = types::InstanceStatus::Cancelled;
            instance
                .payment
                .clone()
                .claim(ctx, &provider, &mut instance)?;
        }

        // Update offer capacity iff the offer still exists. Note that offer capacity mangement
        // is best-effort and the provider can always reset to an arbitrarily high value.
        if let Some(mut offer) = state::get_offer(body.provider, instance.offer) {
            offer.capacity = offer.capacity.saturating_add(1);
            state::set_offer(body.provider, offer);
        }

        state::set_provider(provider);
        state::remove_instance(body.provider, body.id);

        // Remove any queued instance commands.
        for cmd in state::get_instance_commands(body.provider, body.id, u64::MAX.into()) {
            state::remove_instance_command(body.provider, body.id, cmd.id);
        }

        CurrentState::with(|state| {
            state.emit_event(Event::InstanceRemoved {
                provider: body.provider,
                id: body.id,
            })
        });

        Ok(())
    }

    #[handler(call = "roflmarket.InstanceExecuteCmds")]
    fn tx_instance_execute_cmds<C: Context>(
        ctx: &C,
        body: types::InstanceExecuteCmds,
    ) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_BASE)?;

        let cmd_count: u64 = body
            .cmds
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            cmd_count.saturating_mul(Cfg::GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_CMD),
        )?;

        for cmd in &body.cmds {
            if cmd.len() > Cfg::MAX_INSTANCE_COMMAND_SIZE {
                return Err(Error::InvalidArgument);
            }
        }

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let mut instance =
            state::get_instance(body.provider, body.id).ok_or(Error::InstanceNotFound)?;
        Self::ensure_caller_is_instance_admin(&instance)?;

        if instance.status != types::InstanceStatus::Accepted {
            return Err(Error::InvalidInstanceState);
        }

        let new_cmd_count = instance
            .cmd_count
            .checked_add(cmd_count)
            .ok_or(Error::InvalidArgument)?;
        if new_cmd_count >= Cfg::MAX_QUEUED_INSTANCE_COMMANDS {
            return Err(Error::TooManyQueuedCommands);
        }

        for cmd in body.cmds {
            let qc = types::QueuedCommand {
                id: instance.cmd_next_id.increment(),
                cmd,
            };

            state::set_instance_command(body.provider, body.id, qc);
        }

        instance.cmd_count = new_cmd_count;
        instance.updated_at = ctx.now();
        state::set_instance(instance);

        CurrentState::with(|state| {
            state.emit_event(Event::InstanceUpdated {
                provider: body.provider,
                id: body.id,
            })
        });

        Ok(())
    }

    #[handler(call = "roflmarket.InstanceClaimPayment")]
    fn tx_instance_claim_payment<C: Context>(
        ctx: &C,
        body: types::InstanceClaimPayment,
    ) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_INSTANCE_CLAIM_PAYMENT_BASE)?;

        let inst_count: u64 = body
            .instances
            .len()
            .try_into()
            .map_err(|_| Error::InvalidArgument)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            inst_count.saturating_mul(Cfg::GAS_COST_CALL_INSTANCE_CLAIM_PAYMENT_INST),
        )?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        let provider = state::get_provider(body.provider).ok_or(Error::ProviderNotFound)?;
        Self::ensure_caller_is_scheduler_app(&provider)?;

        for id in body.instances {
            let mut instance =
                state::get_instance(body.provider, id).ok_or(Error::InstanceNotFound)?;

            instance
                .payment
                .clone()
                .claim(ctx, &provider, &mut instance)?;
            instance.updated_at = ctx.now();
            state::set_instance(instance);

            CurrentState::with(|state| {
                state.emit_event(Event::InstanceUpdated {
                    provider: body.provider,
                    id,
                })
            });
        }

        Ok(())
    }

    /// Query the minimum stake thresholds.
    #[handler(query = "roflmarket.StakeThresholds")]
    fn query_stake_thresholds<C: Context>(
        _ctx: &C,
        _args: (),
    ) -> Result<types::StakeThresholds, Error> {
        Ok(types::StakeThresholds {
            provider_create: Cfg::STAKE_PROVIDER_CREATE,
        })
    }

    /// Query the provider descriptor.
    #[handler(query = "roflmarket.Provider")]
    fn query_provider<C: Context>(
        _ctx: &C,
        args: types::ProviderQuery,
    ) -> Result<types::Provider, Error> {
        state::get_provider(args.provider).ok_or(Error::ProviderNotFound)
    }

    /// Query all provider descriptors.
    #[handler(query = "roflmarket.Providers", expensive)]
    fn query_providers<C: Context>(_ctx: &C, _args: ()) -> Result<Vec<types::Provider>, Error> {
        Ok(state::get_providers())
    }

    /// Query the offer descriptor.
    #[handler(query = "roflmarket.Offer")]
    fn query_offer<C: Context>(_ctx: &C, args: types::OfferQuery) -> Result<types::Offer, Error> {
        state::get_offer(args.provider, args.id).ok_or(Error::OfferNotFound)
    }

    /// Query all offer descriptors of a given provider.
    #[handler(query = "roflmarket.Offers")]
    fn query_offers<C: Context>(
        _ctx: &C,
        args: types::ProviderQuery,
    ) -> Result<Vec<types::Offer>, Error> {
        Ok(state::get_offers(args.provider))
    }

    /// Query the instance descriptor.
    #[handler(query = "roflmarket.Instance")]
    fn query_instance<C: Context>(
        _ctx: &C,
        args: types::InstanceQuery,
    ) -> Result<types::Instance, Error> {
        state::get_instance(args.provider, args.id).ok_or(Error::InstanceNotFound)
    }

    /// Query all instance descriptors of a given provider.
    #[handler(query = "roflmarket.Instances", expensive)]
    fn query_instances<C: Context>(
        _ctx: &C,
        args: types::ProviderQuery,
    ) -> Result<Vec<types::Instance>, Error> {
        Ok(state::get_instances(args.provider))
    }

    /// Query the queued instance commands.
    #[handler(query = "roflmarket.InstanceCommands")]
    fn query_instance_commands<C: Context>(
        _ctx: &C,
        args: types::InstanceQuery,
    ) -> Result<Vec<types::QueuedCommand>, Error> {
        Ok(state::get_instance_commands(
            args.provider,
            args.id,
            u64::MAX.into(),
        ))
    }
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
