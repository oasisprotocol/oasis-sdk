use std::collections::BTreeSet;

use thiserror::Error;

use oasis_runtime_sdk::{
    self as sdk,
    context::{Context, TxContext},
    error, module,
    module::{CallResult, Module as _},
    modules,
    modules::{accounts, core},
    storage::Prefix,
    types::transaction::AuthInfo,
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
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
pub struct Parameters {}

impl module::Parameters for Parameters {
    type Error = ();
}

/// Events emitted by the consensus module (none so far).
#[derive(Debug, cbor::Encode, sdk::Event)]
#[cbor(untagged)]
pub enum Event {}

/// Genesis state for the consensus module.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Genesis {
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
    fn prefetch(
        prefixes: &mut BTreeSet<Prefix>,
        method: &str,
        body: cbor::Value,
        auth_info: &AuthInfo,
    ) -> module::DispatchResult<cbor::Value, Result<(), error::RuntimeError>> {
        match method {
            "benchmarks.accounts.Transfer" => {
                module::DispatchResult::Handled(|| -> Result<(), error::RuntimeError> {
                    let args: types::AccountsTransfer =
                        cbor::from_value(body).map_err(|_| Error::InvalidArgument)?;
                    let from = auth_info.signer_info[0].address_spec.address();

                    // Prefetch accounts 'to'.
                    prefixes.insert(Prefix::from(
                        [
                            modules::accounts::Module::NAME.as_bytes(),
                            accounts::state::ACCOUNTS,
                            args.to.as_ref(),
                        ]
                        .concat(),
                    ));
                    prefixes.insert(Prefix::from(
                        [
                            modules::accounts::Module::NAME.as_bytes(),
                            accounts::state::BALANCES,
                            args.to.as_ref(),
                        ]
                        .concat(),
                    ));
                    // Prefetch accounts 'from'.
                    prefixes.insert(Prefix::from(
                        [
                            modules::accounts::Module::NAME.as_bytes(),
                            accounts::state::ACCOUNTS,
                            from.as_ref(),
                        ]
                        .concat(),
                    ));
                    prefixes.insert(Prefix::from(
                        [
                            modules::accounts::Module::NAME.as_bytes(),
                            accounts::state::BALANCES,
                            from.as_ref(),
                        ]
                        .concat(),
                    ));

                    Ok(())
                }())
            }
            "benchmarks.accounts.Mint" => {
                // Prefetch minting account and balance.
                let from = auth_info.signer_info[0].address_spec.address();
                prefixes.insert(Prefix::from(
                    [
                        modules::accounts::Module::NAME.as_bytes(),
                        accounts::state::ACCOUNTS,
                        from.as_ref(),
                    ]
                    .concat(),
                ));
                prefixes.insert(Prefix::from(
                    [
                        modules::accounts::Module::NAME.as_bytes(),
                        accounts::state::BALANCES,
                        from.as_ref(),
                    ]
                    .concat(),
                ));
                module::DispatchResult::Handled(Ok(()))
            }
            _ => module::DispatchResult::Unhandled(body),
        }
    }

    fn dispatch_call<C: TxContext>(
        ctx: &mut C,
        method: &str,
        body: cbor::Value,
    ) -> module::DispatchResult<cbor::Value, CallResult> {
        match method {
            "benchmarks.accounts.Mint" => module::dispatch_call(ctx, body, Self::tx_accounts_mint),
            "benchmarks.accounts.Transfer" => {
                module::dispatch_call(ctx, body, Self::tx_accounts_transfer)
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
        genesis: Self::Genesis,
    ) -> bool {
        let version = meta.versions.get(Self::NAME).copied().unwrap_or_default();
        if version == 0 {
            // Initialize state from genesis.
            // Set genesis parameters.
            Self::set_params(ctx.runtime_state(), genesis.parameters);
            meta.versions.insert(Self::NAME.to_owned(), Self::VERSION);
            return true;
        }

        // Migrations are not supported.
        false
    }
}

impl<Accounts: modules::accounts::API> module::TransactionHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::BlockHandler for Module<Accounts> {}

impl<Accounts: modules::accounts::API> module::InvariantHandler for Module<Accounts> {}
