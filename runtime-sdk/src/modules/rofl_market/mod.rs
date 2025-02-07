use crate::{
    context::Context,
    handler, migration,
    module::{self, Module as _, Parameters as _},
    modules::{self, accounts::API as _, core::API as _},
    sdk_derive,
    state::CurrentState,
    types::{address::Address, transaction::Transaction},
    Runtime,
};

mod config;
mod error;
mod event;
pub mod state;
#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "roflmarket";

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

    /// Create a new provider.
    #[handler(call = "roflmarket.ProviderCreate")]
    fn tx_provider_create<C: Context>(ctx: &C, body: types::ProviderCreate) -> Result<(), Error> {
        <C::Runtime as Runtime>::Core::use_tx_gas(Cfg::GAS_COST_CALL_PROVIDER_CREATE)?;

        if CurrentState::with_env(|env| env.is_check_only()) {
            return Ok(());
        }

        // TODO
        Ok(())
    }
}
