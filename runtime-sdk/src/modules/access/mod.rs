//! Method access control module.
use std::collections::{BTreeMap, BTreeSet};

use once_cell::unsync::Lazy;
use thiserror::Error;

use crate::{
    context::Context,
    migration,
    module::{self, Module as _, Parameters as _},
    modules::{self, core::API as _},
    sdk_derive,
    state::CurrentState,
    storage,
    types::{transaction, address::{Address, SignatureAddressSpec}},
};

//#[cfg(test)]
//mod test;
pub mod types;

/// Unique module name.
const MODULE_NAME: &str = "access";

/// Errors emitted by the access module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("caller is not authorized to call method: {0}")]
    #[sdk_error(code = 1)]
    NotAuthorized(String),
}

pub trait Config: 'static {
    const METHOD_AUTHORIZATIONS: Lazy<types::Authorization> = Lazy::new(types::Authorization::new);
}

pub struct Module<Cfg: Config, Accounts: modules::accounts::API> {
    _cfg: std::marker::PhantomData<Cfg>,
    _accounts: std::marker::PhantomData<Accounts>,
}

#[sdk_derive(Module)]
impl<Cfg: Config, Accounts: modules::accounts::API> Module<Cfg, Accounts> {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = ();
    type Parameters = ();
    type Genesis = ();
}

impl<Cfg: Config, Accounts: modules::accounts::API> module::TransactionHandler for Module<Cfg, Accounts> {
    fn before_handle_call<C: Context>(_ctx: &C, call: &transaction::Call) -> Result<(), modules::core::Error> {
        let tx_caller_address = CurrentState::with_env(|env| env.tx_caller_address());
        if Cfg::METHOD_AUTHORIZATIONS.is_authorized(&call.method, &tx_caller_address) {
            Ok(())
        } else {
            Err(modules::core::Error::MalformedTransaction(Error::NotAuthorized(call.method.clone()).into()))
        }
    }
}

impl<Cfg: Config, Accounts: modules::accounts::API> module::BlockHandler for Module<Cfg, Accounts> {}

impl<Cfg: Config, Accounts: modules::accounts::API> module::InvariantHandler for Module<Cfg, Accounts> {}
