use crate::{dispatcher, modules};

use super::MODULE_NAME;

/// Errors emitted by the module.
#[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
pub enum Error {
    #[error("invalid argument")]
    #[sdk_error(code = 1)]
    InvalidArgument,

    #[error("unknown application")]
    #[sdk_error(code = 2)]
    UnknownApp,

    #[error("tx not signed by RAK")]
    #[sdk_error(code = 3)]
    NotSignedByRAK,

    #[error("tx not signed by extra key")]
    #[sdk_error(code = 4)]
    NotSignedByExtraKey,

    #[error("unknown enclave")]
    #[sdk_error(code = 5)]
    UnknownEnclave,

    #[error("unknown node")]
    #[sdk_error(code = 6)]
    UnknownNode,

    #[error("endorsement from given node not allowed")]
    #[sdk_error(code = 7)]
    NodeNotAllowed,

    #[error("registration expired")]
    #[sdk_error(code = 8)]
    RegistrationExpired,

    #[error("extra key update not allowed")]
    #[sdk_error(code = 9)]
    ExtraKeyUpdateNotAllowed,

    #[error("application already exists")]
    #[sdk_error(code = 10)]
    AppAlreadyExists,

    #[error("forbidden")]
    #[sdk_error(code = 11)]
    Forbidden,

    #[error("unknown instance")]
    #[sdk_error(code = 12)]
    UnknownInstance,

    #[error("must use non-plain call format")]
    #[sdk_error(code = 13)]
    PlainCallFormatNotAllowed,

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
