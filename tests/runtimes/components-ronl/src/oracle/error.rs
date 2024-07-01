use oasis_runtime_sdk::modules;

use super::MODULE_NAME;

/// Errors emitted by the module.
#[derive(thiserror::Error, Debug, oasis_runtime_sdk::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("not authorized")]
    #[sdk_error(code = 2)]
    NotAuthorized,

    #[error("{0}")]
    #[sdk_error(transparent)]
    Rofl(#[from] modules::rofl::Error),

    #[error("{0}")]
    #[sdk_error(transparent)]
    Core(#[from] modules::core::Error),
}
