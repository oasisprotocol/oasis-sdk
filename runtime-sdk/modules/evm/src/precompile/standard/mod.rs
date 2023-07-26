//! Implements the standard precompiles as defined in the EVM specification.

mod bn128;
mod modexp;
mod simple;

// Re-exports.
pub(super) use bn128::*;
pub(super) use modexp::*;
pub(super) use simple::*;
