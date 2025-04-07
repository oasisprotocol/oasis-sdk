use oasis_runtime_sdk::{dispatcher, modules};

use super::MODULE_NAME;

/// Errors emitted by the module.
#[derive(thiserror::Error, Debug, oasis_runtime_sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("provider already exists")]
    #[sdk_error(code = 2)]
    ProviderAlreadyExists,

    #[error("provider not found")]
    #[sdk_error(code = 3)]
    ProviderNotFound,

    #[error("forbidden")]
    #[sdk_error(code = 4)]
    Forbidden,

    #[error("provider has instances")]
    #[sdk_error(code = 5)]
    ProviderHasInstances,

    #[error("no more capacity")]
    #[sdk_error(code = 6)]
    OutOfCapacity,

    #[error("offer not found")]
    #[sdk_error(code = 7)]
    OfferNotFound,

    #[error("instance not found")]
    #[sdk_error(code = 8)]
    InstanceNotFound,

    #[error("too many queued commands")]
    #[sdk_error(code = 9)]
    TooManyQueuedCommands,

    #[error("payment failed: {0}")]
    #[sdk_error(code = 10)]
    PaymentFailed(String),

    #[error("bad resource descriptor: {0}")]
    #[sdk_error(code = 11)]
    BadResourceDescriptor(String),

    #[error("invalid instance state")]
    #[sdk_error(code = 12)]
    InvalidInstanceState,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),

    #[error("accounts: {0}")]
    #[sdk_error(transparent)]
    Accounts(#[from] modules::accounts::Error),

    #[error("evm: {0}")]
    #[sdk_error(transparent)]
    Evm(#[from] oasis_runtime_sdk_evm::Error),

    #[error("{0}")]
    #[sdk_error(transparent, abort)]
    Abort(#[source] dispatcher::Error),
}
