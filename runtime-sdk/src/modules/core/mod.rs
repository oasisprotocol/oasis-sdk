//! Core definitions module.
use thiserror::Error;

use crate::types::transaction;

pub mod types;

/// Unique module name.
pub const MODULE_NAME: &str = "core";

/// Errors emitted by the core module.
#[derive(Error, Debug, oasis_runtime_sdk_macros::Error)]
#[sdk_error(module_name_path = "MODULE_NAME")]
pub enum Error {
    #[error("malformed transaction")]
    #[sdk_error(code = 1)]
    MalformedTransaction,

    #[error("invalid transaction: {0}")]
    #[sdk_error(code = 2)]
    InvalidTransaction(#[from] transaction::Error),

    #[error("invalid method")]
    #[sdk_error(code = 3)]
    InvalidMethod,

    #[error("invalid nonce")]
    #[sdk_error(code = 4)]
    InvalidNonce,

    #[error("insufficient balance to pay fees")]
    #[sdk_error(code = 5)]
    InsufficientFeeBalance,
}

/// State schema constants.
pub mod state {
    /// Runtime metadata.
    pub const METADATA: &[u8] = &[0x01];
}
