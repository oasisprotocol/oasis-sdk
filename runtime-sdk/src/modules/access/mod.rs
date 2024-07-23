//! Method access control module.
use once_cell::unsync::Lazy;
use thiserror::Error;

use crate::{
    context::Context,
    module::{self, Module as _},
    modules, sdk_derive,
    state::CurrentState,
    types::transaction,
};

#[cfg(test)]
mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "access";

/// Errors emitted by the access module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("caller is not authorized to call method")]
    #[sdk_error(code = 1)]
    NotAuthorized,
}

/// Module configuration.
#[allow(clippy::declare_interior_mutable_const)]
pub trait Config: 'static {
    /// To filter methods by caller address, add them to this mapping.
    ///
    /// If the mapping is empty, no method is filtered.
    const METHOD_AUTHORIZATIONS: Lazy<types::Authorization> = Lazy::new(types::Authorization::new);
}

/// The method access control module.
pub struct Module<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

#[sdk_derive(Module)]
impl<Cfg: Config> Module<Cfg> {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = ();
    type Parameters = ();
    type Genesis = ();
}

impl<Cfg: Config> module::TransactionHandler for Module<Cfg> {
    fn before_authorized_call_dispatch<C: Context>(
        _ctx: &C,
        call: &transaction::Call,
    ) -> Result<(), modules::core::Error> {
        let tx_caller_address = CurrentState::with_env(|env| env.tx_caller_address());
        #[allow(clippy::borrow_interior_mutable_const)]
        if Cfg::METHOD_AUTHORIZATIONS.is_authorized(&call.method, &tx_caller_address) {
            Ok(())
        } else {
            Err(modules::core::Error::InvalidArgument(
                Error::NotAuthorized.into(),
            ))
        }
    }
}

impl<Cfg: Config> module::BlockHandler for Module<Cfg> {}

impl<Cfg: Config> module::InvariantHandler for Module<Cfg> {}
