//! Core definitions module.
use thiserror::Error;

use crate::{
    error::{self, Error as _},
    types::transaction,
};

pub mod types;

/// Unique module name.
pub const MODULE_NAME: &str = "core";

// TODO: Add a custom derive macro for easier error derivation (module/error codes).
/// Errors emitted by the core module.
#[derive(Error, Debug)]
pub enum Error {
    #[error("malformed transaction")]
    MalformedTransaction,
    #[error("invalid transaction: {0}")]
    InvalidTransaction(#[from] transaction::Error),
    #[error("invalid method")]
    InvalidMethod,
    #[error("invalid nonce")]
    InvalidNonce,
    #[error("insufficient balance to pay fees")]
    InsufficientFeeBalance,
}

impl error::Error for Error {
    fn module(&self) -> &str {
        MODULE_NAME
    }

    fn code(&self) -> u32 {
        match self {
            Error::MalformedTransaction => 1,
            Error::InvalidTransaction(..) => 2,
            Error::InvalidMethod => 3,
            Error::InvalidNonce => 4,
            Error::InsufficientFeeBalance => 5,
        }
    }
}

impl From<Error> for error::RuntimeError {
    fn from(err: Error) -> error::RuntimeError {
        error::RuntimeError::new(err.module(), err.code(), &err.msg())
    }
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
}
