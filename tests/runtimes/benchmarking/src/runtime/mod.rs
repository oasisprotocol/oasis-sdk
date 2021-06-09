use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_runtime_sdk::{
    self as sdk,
    context::{Context, TxContext},
    core::common::cbor,
    error::Error as _,
    module,
    module::Module as _,
    modules,
    modules::{accounts, core},
    types::transaction::CallResult,
};

pub mod types;

const MODULE_NAME: &str = "benchmarks";

/// Errors emitted by the benchmarkkeyvalues module.
#[derive(Error, Debug, sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] core::Error),

    #[error("accounts: {0}")]
    #[sdk_error(transparent)]
    Accounts(#[from] accounts::Error),
}

/// Parameters for the consensus module.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Events emitted by the consensus module (none so far).
#[derive(Debug, Serialize, Deserialize, sdk::Event)]
#[serde(untagged)]
pub enum Event {}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    #[serde(rename = "parameters")]
    pub parameters: Parameters,
}

pub struct Module<Accounts: modules::accounts::API> {
    _accounts: std::marker::PhantomData<Accounts>,
}

// Impls.
impl<Accounts: modules::accounts::API> Module<Accounts> {
    fn tx_accounts_mint<C: TxContext>(ctx: &mut C, body: types::AccountsMint) -> Result<(), Error> {
        // XXX: no gas costs atm.

        Accounts::mint(ctx, ctx.tx_caller_address(), &body.amount)?;

        Ok(())
    }
}

impl<Accounts: modules::accounts::API> Module<Accounts> {
    fn tx_accounts_transfer<C: TxContext>(
        ctx: &mut C,
        body: types::AccountsTransfer,
    ) -> Result<(), Error> {
        // XXX: no gas costs atm.

        Accounts::transfer(ctx, ctx.tx_caller_address(), body.to, &body.amount)?;

        Ok(())
    }
}

impl<Accounts: modules::accounts::API> module::Module for Module<Accounts> {
    const NAME: &'static str = MODULE_NAME;
    const VERSION: u32 = 1;
    type Error = Error;
    type Event = Event;
    type Parameters = Parameters;
}

/// Module methods.
impl<Accounts: modules::accounts::API> module::MethodHandler for Module<Accounts> {
    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "benchmarks.accounts.Mint" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_accounts_mint(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            "benchmarks.accounts.Transfer" => {
                let result = || -> Result<cbor::Value, Error> {
                    let args = cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    Ok(cbor::to_value(&Self::tx_accounts_transfer(ctx, args)?))
                }();
                match result {
                    Ok(value) => module::DispatchResult::Handled(CallResult::Ok(value)),
                    Err(err) => module::DispatchResult::Handled(err.to_call_result()),
                }
            }
            _ => module::DispatchResult::Unhandled(body),
        }
    }
}

impl<Accounts: modules::accounts::API> module::MigrationHandler for Module<Accounts> {
    type Genesis = Genesis;

    fn init_or_migrate<C: Context>(
        ctx: &mut C,
        meta: &mut modules::core::types::Metadata,
        genesis: &Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            // Set genesis parameters.
            Self::set_params(ctx.runtime_state(), &genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}

impl<Accounts: modules::accounts::API> module::AuthHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::BlockHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::InvariantHandler for Module<Accounts> {}
