use crate::{dispatcher, modules};

use super::MODULE_NAME;

/// Errors emitted by the module.
#[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("core: {0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),

    #[error("accounts: {0}")]
    #[sdk_error(transparent)]
    Accounts(#[from] modules::accounts::Error),

    #[error("{0}")]
    #[sdk_error(transparent, abort)]
    Abort(#[source] dispatcher::Error),
}
