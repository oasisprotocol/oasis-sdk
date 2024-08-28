//! Example ROFL-based oracle module.
use oasis_runtime_sdk::{
    context::Context,
    handler, migration,
    module::{self, Module as _, Parameters as _},
    modules::{
        self,
        core::API as _,
        rofl::{app_id::AppId, API as _},
    },
    sdk_derive,
    state::CurrentState,
    Runtime,
};

mod error;
mod event;
pub mod state;
pub mod types;

pub use error::Error;
pub use event::Event;

/// Unique module name.
const MODULE_NAME: &str = "oracle";

/// Module configuration.
pub trait Config: 'static {
    /// Module implementing the ROFL API.
    type Rofl: modules::rofl::API;

    /// Minimum number of required observations to finalize a round.
    const MIN_OBSERVATIONS: usize = 2;

    /// Gas cost of oracle.Observe call.
    const GAS_COST_CALL_OBSERVE: u64 = 1000;

    /// Identifier of the ROFL application that is allowed to contribute observations.
    fn rofl_app_id() -> AppId;

    /// Observation aggregation function.
    fn aggregate(mut observations: Vec<types::Observation>) -> Option<types::Observation> {
        if observations.is_empty() || observations.len() < Self::MIN_OBSERVATIONS {
            return None;
        }

        // Naive median implementation, should work for this example.
        observations.sort_by_key(|obs| obs.value);
        Some(observations[(observations.len() / 2).saturating_sub(1)].clone())
    }
}

/// Parameters for the module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Parameters {}

/// Errors emitted during rewards parameter validation.
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

    /// Process an observation by an oracle.
    #[handler(call = "oracle.Observe")]
    fn tx_observe<C: Context>(ctx: &C, body: types::Observation) -> Result<(), Error> {
        let params = Self::params();
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_OBSERVE)?;

        // Ensure that the observation was processed by the configured ROFL application.
        if !Cfg::Rofl::is_authorized_origin(Cfg::rofl_app_id()) {
            return Err(Error::NotAuthorized);
        }

        // NOTE: This is a naive oracle implementation for ROFL example purposes. A real oracle
        // must do additional checks and better aggregation before accepting values.

        // Update the round.
        let mut round = state::get_current_round();
        round.observations.push(body);

        // Emit aggregated observation when possible.
        if round.observations.len() >= Cfg::MIN_OBSERVATIONS {
            let agg = Cfg::aggregate(std::mem::take(&mut round.observations));
            state::set_last_observation(agg.clone());

            CurrentState::with(|state| state.emit_event(Event::ValueUpdated(agg)));
        }

        state::set_current_round(round);

        Ok(())
    }
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
